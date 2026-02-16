use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use serde::{Deserialize, Serialize};

use crate::types::BrowserKind;

/// Leased browser endpoint details returned by daemon RPC methods.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserLease {
	pub cdp_endpoint: String,
	pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserInfo {
	pub port: u16,
	pub browser: BrowserKind,
	pub headless: bool,
	pub created_at: u64,
	/// Session key this browser is bound to.
	pub session_key: String,
	/// Last time this browser was used (unix timestamp).
	#[serde(default)]
	pub last_used_at: u64,
}

#[rpc(client, server)]
pub trait DaemonRpc {
	#[method(name = "daemon_ping")]
	async fn ping(&self) -> RpcResult<bool>;

	#[method(name = "daemon_acquire_browser")]
	async fn acquire_browser(&self, browser: BrowserKind, headless: bool, session_key: String) -> RpcResult<BrowserLease>;

	#[method(name = "daemon_spawn_browser")]
	async fn spawn_browser(&self, browser: BrowserKind, headless: bool, port: Option<u16>) -> RpcResult<BrowserLease>;

	#[method(name = "daemon_get_browser")]
	async fn get_browser(&self, port: u16) -> RpcResult<Option<BrowserLease>>;

	#[method(name = "daemon_kill_browser")]
	async fn kill_browser(&self, port: u16) -> RpcResult<()>;

	#[method(name = "daemon_release_browser")]
	async fn release_browser(&self, session_key: String) -> RpcResult<()>;

	#[method(name = "daemon_list_browsers")]
	async fn list_browsers(&self) -> RpcResult<Vec<BrowserInfo>>;

	#[method(name = "daemon_shutdown")]
	async fn shutdown(&self) -> RpcResult<()>;
}
