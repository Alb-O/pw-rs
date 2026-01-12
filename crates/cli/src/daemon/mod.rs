mod protocol;
mod server;

use anyhow::{Context, Result, anyhow};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(windows)]
use tokio::net::TcpStream;
#[cfg(unix)]
use tokio::net::UnixStream;
use tracing::debug;

use crate::types::BrowserKind;

pub use protocol::{BrowserInfo, DaemonRequest, DaemonResponse};
pub use server::Daemon;

pub const DAEMON_TCP_PORT: u16 = 19222;

/// Returns the daemon socket path for the current user.
///
/// Uses `$XDG_RUNTIME_DIR/pw-daemon.sock` if available (already user-permissioned),
/// otherwise falls back to `/tmp/pw-daemon-{uid}.sock`.
#[cfg(unix)]
pub fn daemon_socket_path() -> std::path::PathBuf {
    use std::path::PathBuf;

    if let Ok(xdg_runtime) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(xdg_runtime).join("pw-daemon.sock");
    }

    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/pw-daemon-{uid}.sock"))
}

#[derive(Clone, Copy, Debug)]
pub struct DaemonClient;

pub async fn try_connect() -> Option<DaemonClient> {
    match connect_daemon().await {
        Ok(stream) => {
            drop(stream);
            Some(DaemonClient)
        }
        Err(err) if is_not_running(&err) => None,
        Err(err) => {
            debug!(target = "pw.daemon", error = %err, "daemon connection failed");
            None
        }
    }
}

/// Request a browser from the daemon, with optional reuse_key for session reuse.
///
/// If `reuse_key` is provided and a browser with that key exists, it will be reused.
/// Otherwise a new browser is spawned and associated with the key.
pub async fn request_browser(
    _client: &DaemonClient,
    kind: BrowserKind,
    headless: bool,
    reuse_key: Option<&str>,
) -> Result<String> {
    let response = send_request(DaemonRequest::AcquireBrowser {
        browser: kind,
        headless,
        reuse_key: reuse_key.map(|s| s.to_string()),
    })
    .await?;

    match response {
        DaemonResponse::Browser { cdp_endpoint, .. } => Ok(cdp_endpoint),
        DaemonResponse::Error { code, message } => Err(anyhow!("daemon error {code}: {message}")),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

async fn send_request(request: DaemonRequest) -> Result<DaemonResponse> {
    let stream = connect_daemon()
        .await
        .context("Failed to connect to daemon")?;
    send_request_stream(stream, request).await
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
