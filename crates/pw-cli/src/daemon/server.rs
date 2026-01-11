use std::collections::HashMap;
use std::net::TcpListener as StdTcpListener;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(windows)]
use tokio::net::TcpListener;
#[cfg(unix)]
use tokio::net::UnixListener;
use tokio::sync::{Mutex, watch};
use tracing::{debug, info, warn};

#[cfg(windows)]
use super::DAEMON_TCP_PORT;
#[cfg(unix)]
use super::daemon_socket_path;
use super::protocol::{BrowserInfo, DaemonRequest, DaemonResponse};
use crate::types::BrowserKind;
use pw::{LaunchOptions, Playwright};

const PORT_RANGE_START: u16 = 9222;
const PORT_RANGE_END: u16 = 10221;

struct BrowserInstance {
    info: BrowserInfo,
    browser: pw::protocol::Browser,
}

struct DaemonState {
    playwright: Playwright,
    /// Browsers indexed by port.
    browsers: HashMap<u16, BrowserInstance>,
    /// Maps reuse_key -> port for browser reuse lookup.
    reuse_index: HashMap<String, u16>,
}

pub struct Daemon {
    state: Arc<Mutex<DaemonState>>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
    #[cfg(unix)]
    listener: UnixListener,
    #[cfg(windows)]
    listener: TcpListener,
}

impl Daemon {
    pub async fn start() -> Result<Self> {
        let playwright = Playwright::launch()
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
        let state = DaemonState {
            playwright,
            browsers: HashMap::new(),
            reuse_index: HashMap::new(),
        };
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        #[cfg(unix)]
        {
            let socket_path = daemon_socket_path();
            if socket_path.exists() {
                std::fs::remove_file(&socket_path).with_context(|| {
                    format!(
                        "Failed to remove existing socket: {}",
                        socket_path.display()
                    )
                })?;
            }
            // Ensure parent directory exists (for XDG_RUNTIME_DIR fallback)
            if let Some(parent) = socket_path.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent).with_context(|| {
                        format!("Failed to create socket directory: {}", parent.display())
                    })?;
                }
            }
            let listener = UnixListener::bind(&socket_path).with_context(|| {
                format!("Failed to bind daemon socket: {}", socket_path.display())
            })?;
            info!(
                target = "pw.daemon",
                socket = %socket_path.display(),
                "daemon listening"
            );
            Ok(Self {
                state: Arc::new(Mutex::new(state)),
                shutdown_tx,
                shutdown_rx,
                listener,
            })
        }

        #[cfg(windows)]
        {
            let addr = format!("127.0.0.1:{}", DAEMON_TCP_PORT);
            let listener = TcpListener::bind(&addr)
                .await
                .with_context(|| format!("Failed to bind daemon TCP socket: {addr}"))?;
            info!(target = "pw.daemon", addr, "daemon listening");
            Ok(Self {
                state: Arc::new(Mutex::new(state)),
                shutdown_tx,
                shutdown_rx,
                listener,
            })
        }
    }

    pub async fn run(mut self) -> Result<()> {
        #[cfg(unix)]
        {
            run_unix(
                self.listener,
                self.state,
                self.shutdown_tx,
                &mut self.shutdown_rx,
            )
            .await
        }

        #[cfg(windows)]
        {
            run_tcp(
                self.listener,
                self.state,
                self.shutdown_tx,
                &mut self.shutdown_rx,
            )
            .await
        }
    }
}

#[cfg(unix)]
async fn run_unix(
    listener: UnixListener,
    state: Arc<Mutex<DaemonState>>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: &mut watch::Receiver<bool>,
) -> Result<()> {
    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    break;
                }
            }
            accept = listener.accept() => {
                let (stream, _) = accept.context("Daemon accept failed")?;
                let state = Arc::clone(&state);
                let shutdown_tx = shutdown_tx.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle_client(stream, state, shutdown_tx).await {
                        warn!(target = "pw.daemon", error = %err, "daemon connection error");
                    }
                });
            }
        }
    }

    Ok(())
}

#[cfg(windows)]
async fn run_tcp(
    listener: TcpListener,
    state: Arc<Mutex<DaemonState>>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: &mut watch::Receiver<bool>,
) -> Result<()> {
    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    break;
                }
            }
            accept = listener.accept() => {
                let (stream, _) = accept.context("Daemon accept failed")?;
                let state = Arc::clone(&state);
                let shutdown_tx = shutdown_tx.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle_client(stream, state, shutdown_tx).await {
                        warn!(target = "pw.daemon", error = %err, "daemon connection error");
                    }
                });
            }
        }
    }

    Ok(())
}

async fn handle_client<S>(
    stream: S,
    state: Arc<Mutex<DaemonState>>,
    shutdown_tx: watch::Sender<bool>,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (read_half, mut write_half) = tokio::io::split(stream);
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .await
            .context("Failed reading daemon request")?;
        if bytes == 0 {
            break;
        }

        let request = match serde_json::from_str::<DaemonRequest>(line.trim_end()) {
            Ok(req) => req,
            Err(err) => {
                let response = DaemonResponse::Error {
                    code: "invalid_request".to_string(),
                    message: err.to_string(),
                };
                write_response(&mut write_half, &response).await?;
                continue;
            }
        };

        let response = handle_request(&state, shutdown_tx.clone(), request).await;
        write_response(&mut write_half, &response).await?;
    }

    Ok(())
}

async fn write_response<W>(writer: &mut W, response: &DaemonResponse) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let payload = serde_json::to_string(response).context("Failed to serialize response")?;
    writer
        .write_all(format!("{}\n", payload).as_bytes())
        .await
        .context("Failed writing daemon response")?;
    writer
        .flush()
        .await
        .context("Failed flushing daemon response")?;
    Ok(())
}

async fn handle_request(
    state: &Arc<Mutex<DaemonState>>,
    shutdown_tx: watch::Sender<bool>,
    request: DaemonRequest,
) -> DaemonResponse {
    match request {
        DaemonRequest::Ping => DaemonResponse::Pong,
        DaemonRequest::AcquireBrowser {
            browser,
            headless,
            reuse_key,
        } => {
            let mut daemon = state.lock().await;
            match daemon.acquire_browser(browser, headless, reuse_key).await {
                Ok((port, cdp_endpoint)) => DaemonResponse::Browser { cdp_endpoint, port },
                Err(err) => daemon_error("acquire_failed", err),
            }
        }
        DaemonRequest::SpawnBrowser {
            browser,
            headless,
            port,
        } => {
            let mut daemon = state.lock().await;
            match daemon.spawn_browser(browser, headless, port, None).await {
                Ok((port, cdp_endpoint)) => DaemonResponse::Browser { cdp_endpoint, port },
                Err(err) => daemon_error("spawn_failed", err),
            }
        }
        DaemonRequest::GetBrowser { port } => {
            let daemon = state.lock().await;
            if daemon.browsers.contains_key(&port) {
                DaemonResponse::Browser {
                    cdp_endpoint: format!("http://127.0.0.1:{}", port),
                    port,
                }
            } else {
                daemon_error("not_found", anyhow!("No browser on port {port}"))
            }
        }
        DaemonRequest::KillBrowser { port } => {
            let mut daemon = state.lock().await;
            match daemon.kill_browser(port).await {
                Ok(()) => DaemonResponse::Ok,
                Err(err) => daemon_error("kill_failed", err),
            }
        }
        DaemonRequest::ReleaseBrowser { reuse_key } => {
            let mut daemon = state.lock().await;
            daemon.release_browser(&reuse_key);
            DaemonResponse::Ok
        }
        DaemonRequest::ListBrowsers => {
            let daemon = state.lock().await;
            let list = daemon
                .browsers
                .values()
                .map(|instance| instance.info.clone())
                .collect();
            DaemonResponse::Browsers { list }
        }
        DaemonRequest::Shutdown => {
            let mut daemon = state.lock().await;
            if let Err(err) = daemon.shutdown().await {
                return daemon_error("shutdown_failed", err);
            }
            let _ = shutdown_tx.send(true);
            DaemonResponse::Ok
        }
    }
}

impl DaemonState {
    /// Acquire a browser, reusing an existing one if reuse_key matches.
    async fn acquire_browser(
        &mut self,
        browser_kind: BrowserKind,
        headless: bool,
        reuse_key: Option<String>,
    ) -> Result<(u16, String)> {
        // Check for existing browser with matching reuse_key
        if let Some(key) = &reuse_key {
            if let Some(&port) = self.reuse_index.get(key) {
                if let Some(instance) = self.browsers.get_mut(&port) {
                    // Verify browser is still connected
                    if instance.browser.is_connected() {
                        debug!(target = "pw.daemon", port, reuse_key = %key, "reusing existing browser");
                        instance.info.last_used_at = now_ts();
                        let cdp_endpoint = format!("http://127.0.0.1:{}", port);
                        return Ok((port, cdp_endpoint));
                    } else {
                        // Browser disconnected, clean up stale entry
                        debug!(target = "pw.daemon", port, reuse_key = %key, "browser disconnected, removing");
                        self.browsers.remove(&port);
                        self.reuse_index.remove(key);
                    }
                }
            }
        }

        // No existing browser found, spawn a new one
        self.spawn_browser(browser_kind, headless, None, reuse_key)
            .await
    }

    /// Spawn a new browser with optional reuse_key.
    async fn spawn_browser(
        &mut self,
        browser_kind: BrowserKind,
        headless: bool,
        requested_port: Option<u16>,
        reuse_key: Option<String>,
    ) -> Result<(u16, String)> {
        if browser_kind != BrowserKind::Chromium {
            return Err(anyhow!(
                "Daemon-managed browsers currently require chromium"
            ));
        }

        let port = if let Some(port) = requested_port {
            if !(PORT_RANGE_START..=PORT_RANGE_END).contains(&port) {
                return Err(anyhow!("Port {port} outside allowed range"));
            }
            if self.browsers.contains_key(&port) {
                return Err(anyhow!("Port {port} already assigned"));
            }
            if !port_available(port) {
                return Err(anyhow!("Port {port} already in use"));
            }
            port
        } else {
            self.find_available_port()
                .ok_or_else(|| anyhow!("No available ports"))?
        };

        let launch_options = LaunchOptions {
            headless: Some(headless),
            remote_debugging_port: Some(port),
            handle_sighup: Some(false),
            handle_sigint: Some(false),
            handle_sigterm: Some(false),
            ..Default::default()
        };

        debug!(target = "pw.daemon", port, headless, reuse_key = ?reuse_key, "launching browser");
        let browser = self
            .playwright
            .chromium()
            .launch_with_options(launch_options)
            .await
            .map_err(|e| anyhow!(e.to_string()))?;

        let now = now_ts();
        let info = BrowserInfo {
            port,
            browser: browser_kind,
            headless,
            created_at: now,
            reuse_key: reuse_key.clone(),
            last_used_at: now,
        };

        self.browsers.insert(
            port,
            BrowserInstance {
                info: info.clone(),
                browser,
            },
        );

        // Index by reuse_key if provided
        if let Some(key) = reuse_key {
            self.reuse_index.insert(key, port);
        }

        let cdp_endpoint = format!("http://127.0.0.1:{}", port);
        Ok((port, cdp_endpoint))
    }

    /// Release a browser by reuse_key (removes from index but keeps browser running).
    fn release_browser(&mut self, reuse_key: &str) {
        if let Some(port) = self.reuse_index.remove(reuse_key) {
            if let Some(instance) = self.browsers.get_mut(&port) {
                instance.info.reuse_key = None;
            }
        }
    }

    async fn kill_browser(&mut self, port: u16) -> Result<()> {
        let Some(instance) = self.browsers.get(&port) else {
            return Err(anyhow!("No browser on port {port}"));
        };

        // Remove from reuse index if present
        if let Some(key) = &instance.info.reuse_key {
            self.reuse_index.remove(key);
        }

        instance
            .browser
            .close()
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
        self.browsers.remove(&port);
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        let ports: Vec<u16> = self.browsers.keys().copied().collect();
        for port in ports {
            let _ = self.kill_browser(port).await;
        }
        self.reuse_index.clear();
        self.playwright
            .shutdown()
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
        Ok(())
    }

    fn find_available_port(&self) -> Option<u16> {
        (PORT_RANGE_START..=PORT_RANGE_END)
            .find(|port| !self.browsers.contains_key(port) && port_available(*port))
    }
}

fn daemon_error(code: &str, err: anyhow::Error) -> DaemonResponse {
    DaemonResponse::Error {
        code: code.to_string(),
        message: err.to_string(),
    }
}

fn port_available(port: u16) -> bool {
    StdTcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
