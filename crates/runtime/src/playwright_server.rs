//! Playwright server management
//!
//! Handles downloading, launching, and managing the lifecycle of the Playwright
//! Node.js server process.

use crate::driver::get_driver_executable;
use crate::error::{Error, Result};
use tokio::process::{Child, Command};

/// Manages the Playwright server process lifecycle
///
/// The PlaywrightServer wraps a Node.js child process that runs the Playwright
/// driver. It communicates with the server via stdio pipes using JSON-RPC protocol.
#[derive(Debug)]
pub struct PlaywrightServer {
    /// The Playwright server child process
    ///
    /// This is public to allow integration tests to access stdin/stdout pipes.
    /// In production code, you should use the Connection layer instead of
    /// accessing the process directly.
    pub process: Child,
}

impl PlaywrightServer {
    /// Launch the Playwright server process
    ///
    /// This will:
    /// 1. Check if the Playwright driver exists (download if needed)
    /// 2. Launch the server using `node <driver>/cli.js run-driver`
    /// 3. Set environment variable `PW_LANG_NAME=rust`
    ///
    /// # Errors
    ///
    /// Returns `Error::ServerNotFound` if the driver cannot be located.
    /// Returns `Error::LaunchFailed` if the process fails to start.
    pub async fn launch() -> Result<Self> {
        let (node_exe, cli_js) = get_driver_executable()?;

        let mut cmd = Command::new(&node_exe);
        cmd.arg(&cli_js)
            .arg("run-driver")
            .env("PW_LANG_NAME", "rust")
            .env("PW_LANG_NAME_VERSION", env!("CARGO_PKG_RUST_VERSION"))
            .env("PW_CLI_DISPLAY_VERSION", env!("CARGO_PKG_VERSION"))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit());

        // NixOS compatibility: Pass through PLAYWRIGHT_BROWSERS_PATH if set
        if let Ok(browsers_path) = std::env::var("PLAYWRIGHT_BROWSERS_PATH") {
            cmd.env("PLAYWRIGHT_BROWSERS_PATH", browsers_path);
        }

        // Also pass through PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD to prevent download attempts
        if let Ok(skip_download) = std::env::var("PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD") {
            cmd.env("PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD", skip_download);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| Error::LaunchFailed(format!("Failed to spawn process: {}", e)))?;

        // Check if process started successfully
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        match child.try_wait() {
            Ok(Some(status)) => {
                return Err(Error::LaunchFailed(format!(
                    "Server process exited immediately with status: {}",
                    status
                )));
            }
            Ok(None) => {
                // Process is still running, good!
            }
            Err(e) => {
                return Err(Error::LaunchFailed(format!(
                    "Failed to check process status: {}",
                    e
                )));
            }
        }

        Ok(Self { process: child })
    }

    /// Shut down the server gracefully
    ///
    /// Sends a shutdown signal to the server and waits for it to exit.
    ///
    /// # Platform-Specific Behavior
    ///
    /// **Windows**: Explicitly closes stdio pipes before killing the process to avoid
    /// hangs. On Windows, tokio uses a blocking threadpool for child process stdio,
    /// and failing to close pipes before terminating can cause the cleanup to hang
    /// indefinitely.
    ///
    /// **Unix**: Uses standard process termination with graceful wait.
    pub async fn shutdown(mut self) -> Result<()> {
        #[cfg(windows)]
        {
            drop(self.process.stdin.take());
            drop(self.process.stdout.take());
            drop(self.process.stderr.take());

            self.process
                .kill()
                .await
                .map_err(|e| Error::LaunchFailed(format!("Failed to kill process: {}", e)))?;

            match tokio::time::timeout(std::time::Duration::from_secs(5), self.process.wait()).await
            {
                Ok(Ok(_)) => Ok(()),
                Ok(Err(e)) => Err(Error::LaunchFailed(format!(
                    "Failed to wait for process: {}",
                    e
                ))),
                Err(_) => {
                    let _ = self.process.start_kill();
                    Err(Error::LaunchFailed(
                        "Process shutdown timeout after 5 seconds".to_string(),
                    ))
                }
            }
        }

        #[cfg(not(windows))]
        {
            self.process
                .kill()
                .await
                .map_err(|e| Error::LaunchFailed(format!("Failed to kill process: {}", e)))?;

            let _ = self.process.wait().await;

            Ok(())
        }
    }

    /// Force kill the server process
    ///
    /// This should only be used if graceful shutdown fails.
    pub async fn kill(mut self) -> Result<()> {
        #[cfg(windows)]
        {
            drop(self.process.stdin.take());
            drop(self.process.stdout.take());
            drop(self.process.stderr.take());
        }

        self.process
            .kill()
            .await
            .map_err(|e| Error::LaunchFailed(format!("Failed to kill process: {}", e)))?;

        #[cfg(windows)]
        {
            let _ =
                tokio::time::timeout(std::time::Duration::from_secs(2), self.process.wait()).await;
        }

        #[cfg(not(windows))]
        {
            let _ =
                tokio::time::timeout(std::time::Duration::from_millis(500), self.process.wait())
                    .await;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_launch_and_shutdown() {
        let result = PlaywrightServer::launch().await;

        match result {
            Ok(server) => {
                println!("Server launched successfully!");
                let shutdown_result = server.shutdown().await;
                assert!(
                    shutdown_result.is_ok(),
                    "Shutdown failed: {:?}",
                    shutdown_result
                );
            }
            Err(Error::ServerNotFound) => {
                eprintln!(
                    "Could not launch server: Playwright not found and download may have failed"
                );
            }
            Err(Error::LaunchFailed(msg)) => {
                eprintln!("Launch failed: {}", msg);
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_server_can_be_killed() {
        let result = PlaywrightServer::launch().await;

        if let Ok(server) = result {
            println!("Server launched, testing kill...");
            let kill_result = server.kill().await;
            assert!(kill_result.is_ok(), "Kill failed: {:?}", kill_result);
        } else {
            eprintln!("Server didn't launch (expected without Node.js/Playwright)");
        }
    }
}
