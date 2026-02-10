use serde::{Deserialize, Serialize};

use crate::types::BrowserKind;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonRequest {
	Ping,
	/// Acquire a browser, reusing an existing one if session_key matches.
	AcquireBrowser {
		browser: BrowserKind,
		headless: bool,
		/// Deterministic key for browser reuse and isolation.
		session_key: String,
	},
	/// Legacy: spawn a new browser without reuse (kept for compatibility).
	SpawnBrowser {
		browser: BrowserKind,
		headless: bool,
		port: Option<u16>,
	},
	GetBrowser {
		port: u16,
	},
	KillBrowser {
		port: u16,
	},
	/// Release a browser by session_key (marks it available but doesn't close it).
	ReleaseBrowser {
		session_key: String,
	},
	ListBrowsers,
	Shutdown,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonResponse {
	Pong,
	Browser { cdp_endpoint: String, port: u16 },
	Browsers { list: Vec<BrowserInfo> },
	Ok,
	Error { code: String, message: String },
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
