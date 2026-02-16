mod client;
mod rpc;
mod server;

use anyhow::{Result, anyhow};
use jsonrpsee::core::ClientError;
use jsonrpsee::http_client::HttpClient;
use rpc::DaemonRpcClient as _;
pub use rpc::{BrowserInfo, BrowserLease};
pub use server::Daemon;
use tracing::debug;

use crate::types::BrowserKind;

pub const DAEMON_TCP_PORT: u16 = 19222;

#[derive(Debug, Clone)]
pub struct DaemonClient {
	client: HttpClient,
}

pub async fn try_connect() -> Option<DaemonClient> {
	let probe = match client::connect_probe_client() {
		Ok(client) => client,
		Err(err) => {
			debug!(target = "pw.daemon", error = %err, "failed to build daemon RPC client");
			return None;
		}
	};

	match probe.ping().await {
		Ok(true) => {
			let client = match client::connect_client() {
				Ok(client) => client,
				Err(err) => {
					debug!(target = "pw.daemon", error = %err, "failed to build daemon RPC client");
					return None;
				}
			};
			Some(DaemonClient { client })
		}
		Ok(false) => None,
		Err(err) if is_not_running(&err) => None,
		Err(err) => {
			debug!(target = "pw.daemon", error = %err, "daemon connection failed");
			None
		}
	}
}

/// Request a browser from the daemon with a deterministic session key.
///
/// Browsers are reused only when session keys match exactly.
pub async fn request_browser(client: &DaemonClient, kind: BrowserKind, headless: bool, session_key: &str) -> Result<String> {
	let lease = client
		.client
		.acquire_browser(kind, headless, session_key.to_string())
		.await
		.map_err(|err| anyhow!("daemon RPC acquire_browser failed: {err}"))?;
	Ok(lease.cdp_endpoint)
}

pub async fn ping() -> Result<Option<bool>> {
	let client = client::connect_probe_client()?;
	match client.ping().await {
		Ok(value) => Ok(Some(value)),
		Err(err) if is_not_running(&err) => Ok(None),
		Err(err) => Err(anyhow!("daemon RPC ping failed: {err}")),
	}
}

pub async fn shutdown() -> Result<Option<()>> {
	let probe = client::connect_probe_client()?;
	match probe.ping().await {
		Ok(true) => {}
		Ok(false) => return Ok(None),
		Err(err) if is_not_running(&err) => return Ok(None),
		Err(err) => return Err(anyhow!("daemon RPC ping failed before shutdown: {err}")),
	}

	let client = client::connect_client()?;
	match client.shutdown().await {
		Ok(()) => Ok(Some(())),
		Err(err) if is_not_running(&err) => Ok(None),
		Err(err) => Err(anyhow!("daemon RPC shutdown failed: {err}")),
	}
}

pub async fn list_browsers() -> Result<Option<Vec<BrowserInfo>>> {
	let client = client::connect_probe_client()?;
	match client.list_browsers().await {
		Ok(list) => Ok(Some(list)),
		Err(err) if is_not_running(&err) => Ok(None),
		Err(err) => Err(anyhow!("daemon RPC list_browsers failed: {err}")),
	}
}

fn is_not_running(err: &ClientError) -> bool {
	client::is_not_running_error(err)
}
