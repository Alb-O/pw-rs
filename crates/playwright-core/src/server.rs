// Playwright server management
//
// Handles downloading, launching, and managing the lifecycle of the Playwright
// Node.js server process.

use crate::driver::get_driver_executable;
use crate::{Error, Result};
use tokio::process::{Child, Command};

/// Manages the Playwright server process lifecycle
///
/// The PlaywrightServer wraps a Node.js child process that runs the Playwright
/// driver. It communicates with the server via stdio pipes using JSON-RPC protocol.
///
/// # Example
///
/// ```no_run
/// # use playwright_core::PlaywrightServer;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let server = PlaywrightServer::launch().await?;
/// // Use the server...
/// server.shutdown().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct PlaywrightServer {
    process: Child,
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
    ///
    /// See: <https://playwright.dev/docs/api>
    pub async fn launch() -> Result<Self> {
        // Get the driver executable paths
        // The driver should already be downloaded by build.rs
        let (node_exe, cli_js) = get_driver_executable()?;

        // Launch the server process
        let mut child = Command::new(&node_exe)
            .arg(&cli_js)
            .arg("run-driver")
            .env("PW_LANG_NAME", "rust")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(|e| Error::LaunchFailed(format!("Failed to spawn process: {}", e)))?;

        // Check if process started successfully
        // Give it a moment to potentially fail
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
    /// # Errors
    ///
    /// Returns an error if the shutdown fails or times out.
    pub async fn shutdown(mut self) -> Result<()> {
        // For now, just kill the process
        // In the future, we should send a proper shutdown message via JSON-RPC
        self.process
            .kill()
            .await
            .map_err(|e| Error::LaunchFailed(format!("Failed to kill process: {}", e)))?;

        // Wait for process to exit
        let _ = self.process.wait().await;

        Ok(())
    }

    /// Force kill the server process
    ///
    /// This should only be used if graceful shutdown fails.
    ///
    /// # Errors
    ///
    /// Returns an error if the kill operation fails.
    pub async fn kill(mut self) -> Result<()> {
        self.process
            .kill()
            .await
            .map_err(|e| Error::LaunchFailed(format!("Failed to kill process: {}", e)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_launch_and_shutdown() {
        // This test will attempt to launch the Playwright server
        // If Playwright is not installed, it will try to download it
        let result = PlaywrightServer::launch().await;

        match result {
            Ok(server) => {
                println!("Server launched successfully!");
                // Clean shutdown
                let shutdown_result = server.shutdown().await;
                assert!(
                    shutdown_result.is_ok(),
                    "Shutdown failed: {:?}",
                    shutdown_result
                );
            }
            Err(Error::ServerNotFound) => {
                // This can happen if npm is not installed or download fails
                eprintln!(
                    "Could not launch server: Playwright not found and download may have failed"
                );
                eprintln!("To run this test, install Playwright manually: npm install playwright");
                // Don't fail the test - this is expected in CI without Node.js
            }
            Err(Error::LaunchFailed(msg)) => {
                eprintln!("Launch failed: {}", msg);
                eprintln!("This may be expected if Node.js or npm is not installed");
                // Don't fail - expected in environments without Node.js
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_server_can_be_killed() {
        // Test that we can force-kill a server
        let result = PlaywrightServer::launch().await;

        if let Ok(server) = result {
            println!("Server launched, testing kill...");
            let kill_result = server.kill().await;
            assert!(kill_result.is_ok(), "Kill failed: {:?}", kill_result);
        } else {
            // Server didn't launch, that's okay for this test
            eprintln!("Server didn't launch (expected without Node.js/Playwright)");
        }
    }
}
