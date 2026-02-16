use std::path::PathBuf;

/// Information about a completed download.
#[derive(Debug, Clone)]
pub struct DownloadInfo {
	/// URL the download was initiated from.
	pub url: String,
	/// Suggested filename from the server.
	pub suggested_filename: String,
	/// Path where the file was saved.
	pub path: PathBuf,
}

/// Session endpoints exposed by a browser session.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionEndpoints {
	/// BrowserServer WebSocket endpoint when launched in server mode.
	pub ws: Option<String>,
	/// CDP endpoint when attached/launched with debugging.
	pub cdp: Option<String>,
}

impl SessionEndpoints {
	/// Returns true when neither endpoint is available.
	pub fn is_empty(&self) -> bool {
		self.ws.is_none() && self.cdp.is_none()
	}

	/// Returns WebSocket endpoint when available.
	pub fn ws_endpoint(&self) -> Option<&str> {
		self.ws.as_deref()
	}

	/// Returns CDP endpoint when available.
	pub fn cdp_endpoint(&self) -> Option<&str> {
		self.cdp.as_deref()
	}
}

/// Summary emitted after attempting auth-cookie injection from files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthInjectionReport {
	/// Number of auth files considered.
	pub files_seen: usize,
	/// Number of auth files that successfully loaded.
	pub files_loaded: usize,
	/// Total cookies added to the browser context.
	pub cookies_added: usize,
}
