use std::collections::HashMap;
use std::net::TcpListener as StdTcpListener;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use jsonrpsee::core::{RpcResult, async_trait};
use jsonrpsee::server::ServerBuilder;
use jsonrpsee::types::error::ErrorObjectOwned;
use pw_rs::{LaunchOptions, Playwright};
use serde_json::json;
use tokio::sync::{Mutex, watch};
use tracing::{debug, info, warn};

use super::DAEMON_TCP_PORT;
use super::rpc::{BrowserInfo, BrowserLease, DaemonRpcServer};
use crate::types::BrowserKind;

const PORT_RANGE_START: u16 = 9222;
const PORT_RANGE_END: u16 = 10221;

const RPC_ACQUIRE_FAILED: i32 = -32050;
const RPC_SPAWN_FAILED: i32 = -32051;
const RPC_KILL_FAILED: i32 = -32052;
const RPC_SHUTDOWN_FAILED: i32 = -32053;

struct BrowserInstance {
	info: BrowserInfo,
	browser: pw_rs::Browser,
}

struct DaemonState {
	playwright: Playwright,
	/// Browsers indexed by port.
	browsers: HashMap<u16, BrowserInstance>,
	/// Maps session_key -> port for browser reuse lookup.
	session_index: HashMap<String, u16>,
}

struct DaemonRpcHandler {
	state: Arc<Mutex<DaemonState>>,
	shutdown_tx: watch::Sender<bool>,
}

#[async_trait]
impl DaemonRpcServer for DaemonRpcHandler {
	async fn ping(&self) -> RpcResult<bool> {
		Ok(true)
	}

	async fn acquire_browser(&self, browser: BrowserKind, headless: bool, session_key: String) -> RpcResult<BrowserLease> {
		let mut daemon = self.state.lock().await;
		daemon
			.acquire_browser(browser, headless, session_key)
			.await
			.map(|(port, cdp_endpoint)| BrowserLease { cdp_endpoint, port })
			.map_err(|err| rpc_error("acquire_failed", RPC_ACQUIRE_FAILED, err))
	}

	async fn spawn_browser(&self, browser: BrowserKind, headless: bool, port: Option<u16>) -> RpcResult<BrowserLease> {
		let mut daemon = self.state.lock().await;
		let session_key = format!("spawn:{}:{}:{}", browser, headless, now_ts());
		daemon
			.spawn_browser(browser, headless, port, session_key)
			.await
			.map(|(port, cdp_endpoint)| BrowserLease { cdp_endpoint, port })
			.map_err(|err| rpc_error("spawn_failed", RPC_SPAWN_FAILED, err))
	}

	async fn get_browser(&self, port: u16) -> RpcResult<Option<BrowserLease>> {
		let daemon = self.state.lock().await;
		if daemon.browsers.contains_key(&port) {
			Ok(Some(BrowserLease {
				cdp_endpoint: format!("http://127.0.0.1:{}", port),
				port,
			}))
		} else {
			Ok(None)
		}
	}

	async fn kill_browser(&self, port: u16) -> RpcResult<()> {
		let mut daemon = self.state.lock().await;
		daemon.kill_browser(port).await.map_err(|err| rpc_error("kill_failed", RPC_KILL_FAILED, err))
	}

	async fn release_browser(&self, session_key: String) -> RpcResult<()> {
		let mut daemon = self.state.lock().await;
		daemon.release_browser(&session_key);
		Ok(())
	}

	async fn list_browsers(&self) -> RpcResult<Vec<BrowserInfo>> {
		let daemon = self.state.lock().await;
		Ok(daemon.browsers.values().map(|instance| instance.info.clone()).collect())
	}

	async fn shutdown(&self) -> RpcResult<()> {
		let mut daemon = self.state.lock().await;
		daemon.shutdown().await.map_err(|err| rpc_error("shutdown_failed", RPC_SHUTDOWN_FAILED, err))?;
		let _ = self.shutdown_tx.send(true);
		Ok(())
	}
}

pub struct Daemon {
	state: Arc<Mutex<DaemonState>>,
	shutdown_tx: watch::Sender<bool>,
	shutdown_rx: watch::Receiver<bool>,
}

impl Daemon {
	pub async fn start() -> Result<Self> {
		let playwright = Playwright::launch().await.map_err(|e| anyhow!(e.to_string()))?;
		let state = DaemonState {
			playwright,
			browsers: HashMap::new(),
			session_index: HashMap::new(),
		};
		let (shutdown_tx, shutdown_rx) = watch::channel(false);
		Ok(Self {
			state: Arc::new(Mutex::new(state)),
			shutdown_tx,
			shutdown_rx,
		})
	}

	pub async fn run(mut self) -> Result<()> {
		let addr = format!("127.0.0.1:{}", DAEMON_TCP_PORT);
		let server = ServerBuilder::default()
			.build(&addr)
			.await
			.with_context(|| format!("Failed to bind daemon RPC server: {addr}"))?;

		let rpc = DaemonRpcHandler {
			state: Arc::clone(&self.state),
			shutdown_tx: self.shutdown_tx.clone(),
		};
		let handle = server.start(rpc.into_rpc());
		info!(target = "pw.daemon", addr, "daemon listening");

		#[cfg(unix)]
		{
			use tokio::signal::unix::{SignalKind, signal};

			let mut sigterm = signal(SignalKind::terminate()).context("Failed to install SIGTERM handler")?;
			let mut sigint = signal(SignalKind::interrupt()).context("Failed to install SIGINT handler")?;

			loop {
				tokio::select! {
					_ = self.shutdown_rx.changed() => {
						if *self.shutdown_rx.borrow() {
							info!(target = "pw.daemon", "shutdown requested via RPC");
							break;
						}
					}
					_ = sigterm.recv() => {
						info!(target = "pw.daemon", "received SIGTERM, shutting down");
						shutdown_daemon_state(&self.state).await;
						let _ = self.shutdown_tx.send(true);
						break;
					}
					_ = sigint.recv() => {
						info!(target = "pw.daemon", "received SIGINT, shutting down");
						shutdown_daemon_state(&self.state).await;
						let _ = self.shutdown_tx.send(true);
						break;
					}
				}
			}
		}

		#[cfg(windows)]
		{
			loop {
				tokio::select! {
					_ = self.shutdown_rx.changed() => {
						if *self.shutdown_rx.borrow() {
							info!(target = "pw.daemon", "shutdown requested via RPC");
							break;
						}
					}
					_ = tokio::signal::ctrl_c() => {
						info!(target = "pw.daemon", "received Ctrl+C, shutting down");
						shutdown_daemon_state(&self.state).await;
						let _ = self.shutdown_tx.send(true);
						break;
					}
				}
			}
		}

		let _ = handle.stop();
		handle.stopped().await;
		Ok(())
	}
}

impl DaemonState {
	/// Acquire a browser, reusing an existing one if session_key matches.
	async fn acquire_browser(&mut self, browser_kind: BrowserKind, headless: bool, session_key: String) -> Result<(u16, String)> {
		// Check for existing browser with matching session_key.
		if let Some(&port) = self.session_index.get(&session_key) {
			if let Some(instance) = self.browsers.get_mut(&port) {
				// Verify browser is still connected.
				if instance.browser.is_connected() {
					debug!(target = "pw.daemon", port, session_key = %session_key, "reusing existing browser");
					instance.info.last_used_at = now_ts();
					let cdp_endpoint = format!("http://127.0.0.1:{}", port);
					return Ok((port, cdp_endpoint));
				}

				// Browser disconnected, clean up stale entry.
				debug!(target = "pw.daemon", port, session_key = %session_key, "browser disconnected, removing");
				self.browsers.remove(&port);
				self.session_index.remove(&session_key);
			}
		}

		// No existing browser found, spawn a new one.
		self.spawn_browser(browser_kind, headless, None, session_key).await
	}

	/// Spawn a new browser bound to `session_key`.
	async fn spawn_browser(&mut self, browser_kind: BrowserKind, headless: bool, requested_port: Option<u16>, session_key: String) -> Result<(u16, String)> {
		if browser_kind != BrowserKind::Chromium {
			return Err(anyhow!("Daemon-managed browsers currently require chromium"));
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
			self.find_available_port().ok_or_else(|| anyhow!("No available ports"))?
		};

		let launch_options = LaunchOptions {
			headless: Some(headless),
			remote_debugging_port: Some(port),
			handle_sighup: Some(false),
			handle_sigint: Some(false),
			handle_sigterm: Some(false),
			..Default::default()
		};

		debug!(target = "pw.daemon", port, headless, session_key = %session_key, "launching browser");
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
			session_key: session_key.clone(),
			last_used_at: now,
		};

		self.browsers.insert(port, BrowserInstance { info: info.clone(), browser });
		self.session_index.insert(session_key, port);

		let cdp_endpoint = format!("http://127.0.0.1:{}", port);
		Ok((port, cdp_endpoint))
	}

	/// Release a browser by session key (removes from index but keeps browser running).
	fn release_browser(&mut self, session_key: &str) {
		if let Some(port) = self.session_index.remove(session_key) {
			if let Some(instance) = self.browsers.get_mut(&port) {
				instance.info.session_key.clear();
			}
		}
	}

	async fn kill_browser(&mut self, port: u16) -> Result<()> {
		let Some(instance) = self.browsers.get(&port) else {
			return Err(anyhow!("No browser on port {port}"));
		};

		// Remove from session index.
		if !instance.info.session_key.is_empty() {
			self.session_index.remove(&instance.info.session_key);
		}

		instance.browser.close().await.map_err(|e| anyhow!(e.to_string()))?;
		self.browsers.remove(&port);
		Ok(())
	}

	async fn shutdown(&mut self) -> Result<()> {
		let ports: Vec<u16> = self.browsers.keys().copied().collect();
		for port in ports {
			let _ = self.kill_browser(port).await;
		}
		self.session_index.clear();
		self.playwright.shutdown().await.map_err(|e| anyhow!(e.to_string()))?;
		Ok(())
	}

	fn find_available_port(&self) -> Option<u16> {
		(PORT_RANGE_START..=PORT_RANGE_END).find(|port| !self.browsers.contains_key(port) && port_available(*port))
	}
}

async fn shutdown_daemon_state(state: &Arc<Mutex<DaemonState>>) {
	let mut daemon = state.lock().await;
	if let Err(err) = daemon.shutdown().await {
		warn!(target = "pw.daemon", error = %err, "error during shutdown");
	}
}

fn rpc_error(code: &str, rpc_code: i32, err: anyhow::Error) -> ErrorObjectOwned {
	ErrorObjectOwned::owned(rpc_code, err.to_string(), Some(json!({ "code": code })))
}

fn port_available(port: u16) -> bool {
	StdTcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn now_ts() -> u64 {
	std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}
