use anyhow::{Context, anyhow};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(windows)]
use tokio::net::TcpStream;
#[cfg(unix)]
use tokio::net::UnixStream;

#[cfg(windows)]
use crate::daemon::DAEMON_TCP_PORT;
#[cfg(unix)]
use crate::daemon::daemon_socket_path;
use crate::daemon::{Daemon, DaemonRequest, DaemonResponse};
use crate::error::{PwError, Result};
use crate::output::{OutputFormat, ResultBuilder, print_result};

/// Get the daemon PID file path for the current user.
/// Uses the same directory as the daemon socket.
#[cfg(unix)]
fn daemon_pid_path() -> std::path::PathBuf {
    let socket_path = daemon_socket_path();
    socket_path.with_file_name("pw-daemon.pid")
}

pub async fn start(foreground: bool, format: OutputFormat) -> Result<()> {
    if foreground {
        let result = ResultBuilder::new("daemon start")
            .data(json!({
                "started": true,
                "foreground": true
            }))
            .build();
        print_result(&result, format);

        let daemon = Daemon::start().await?;
        daemon.run().await?;
        return Ok(());
    }

    #[cfg(windows)]
    {
        return Err(PwError::Context(
            "Background daemon mode is not available on Windows; use --foreground".to_string(),
        ));
    }

    #[cfg(unix)]
    {
        // Spawn a new process for the daemon rather than forking
        // This avoids issues with tokio runtime after fork and keeps stdio working
        let exe = std::env::current_exe()
            .map_err(|e| PwError::Anyhow(anyhow!("Failed to get executable path: {e}")))?;

        let child = std::process::Command::new(&exe)
            .arg("daemon")
            .arg("start")
            .arg("--foreground")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| PwError::Anyhow(anyhow!("Failed to spawn daemon: {e}")))?;

        // Write PID file
        let pid_path = daemon_pid_path();
        // Ensure parent directory exists
        if let Some(parent) = pid_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&pid_path, child.id().to_string())?;

        // Wait a bit for daemon to start
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Check if it's running
        let status = send_request(DaemonRequest::Ping).await?;
        let running = matches!(status, Some(DaemonResponse::Pong));

        let result = ResultBuilder::new("daemon start")
            .data(json!({
                "started": running,
                "foreground": false,
                "pid_file": pid_path.display().to_string(),
                "pid": child.id()
            }))
            .build();
        print_result(&result, format);

        if !running {
            return Err(PwError::Anyhow(anyhow!("Daemon failed to start")));
        }

        Ok(())
    }
}

pub async fn stop(format: OutputFormat) -> Result<()> {
    let response = send_request(DaemonRequest::Shutdown).await?;
    match response {
        None => {
            let result = ResultBuilder::new("daemon stop")
                .data(json!({
                    "stopped": false,
                    "message": "daemon not running"
                }))
                .build();
            print_result(&result, format);
            Ok(())
        }
        Some(DaemonResponse::Ok) => {
            let result = ResultBuilder::new("daemon stop")
                .data(json!({ "stopped": true }))
                .build();
            print_result(&result, format);
            Ok(())
        }
        Some(DaemonResponse::Error { code, message }) => {
            Err(PwError::Anyhow(anyhow!("daemon error {code}: {message}")))
        }
        Some(other) => Err(PwError::Anyhow(anyhow!(
            "unexpected daemon response: {other:?}"
        ))),
    }
}

pub async fn status(format: OutputFormat) -> Result<()> {
    let response = send_request(DaemonRequest::Ping).await?;
    let Some(response) = response else {
        let result = ResultBuilder::new("daemon status")
            .data(json!({
                "running": false,
                "message": "daemon not running"
            }))
            .build();
        print_result(&result, format);
        return Ok(());
    };

    match response {
        DaemonResponse::Pong => {
            let list = match send_request(DaemonRequest::ListBrowsers).await? {
                Some(DaemonResponse::Browsers { list }) => list,
                _ => Vec::new(),
            };
            let result = ResultBuilder::new("daemon status")
                .data(json!({
                    "running": true,
                    "browsers": list
                }))
                .build();
            print_result(&result, format);
            Ok(())
        }
        DaemonResponse::Error { code, message } => {
            Err(PwError::Anyhow(anyhow!("daemon error {code}: {message}")))
        }
        other => Err(PwError::Anyhow(anyhow!(
            "unexpected daemon response: {other:?}"
        ))),
    }
}

async fn send_request(request: DaemonRequest) -> Result<Option<DaemonResponse>> {
    let stream = match connect_daemon().await {
        Ok(stream) => stream,
        Err(err) if is_not_running(&err) => return Ok(None),
        Err(err) => return Err(PwError::Io(err)),
    };

    let response = send_request_stream(stream, request).await?;
    Ok(Some(response))
}

#[cfg(unix)]
async fn connect_daemon() -> std::io::Result<UnixStream> {
    UnixStream::connect(daemon_socket_path()).await
}

#[cfg(windows)]
async fn connect_daemon() -> std::io::Result<TcpStream> {
    TcpStream::connect(("127.0.0.1", DAEMON_TCP_PORT)).await
}

fn is_not_running(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
    )
}

async fn send_request_stream<S>(mut stream: S, request: DaemonRequest) -> Result<DaemonResponse>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let payload = serde_json::to_string(&request).context("Failed to serialize daemon request")?;
    stream
        .write_all(format!("{}\n", payload).as_bytes())
        .await
        .context("Failed writing daemon request")?;
    stream
        .flush()
        .await
        .context("Failed flushing daemon request")?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .await
        .context("Failed reading daemon response")?;
    let response = serde_json::from_str(&line).context("Failed parsing daemon response")?;
    Ok(response)
}
