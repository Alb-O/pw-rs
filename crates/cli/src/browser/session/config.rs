use pw_rs::{StorageState, WaitUntil};

use crate::context::{BlockConfig, DownloadConfig, HarConfig};
use crate::types::BrowserKind;

/// Fully owned browser-session configuration.
///
/// This type is the stable handoff between higher-level session orchestration
/// and the browser-session builder internals.
#[derive(Debug, Clone)]
pub struct SessionConfig {
	/// Navigation wait strategy used by page operations.
	pub wait_until: WaitUntil,
	/// Optional storage state used to seed browser context auth.
	pub storage_state: Option<StorageState>,
	/// Whether browser launches headless.
	pub headless: bool,
	/// Browser engine used for launch/connect operations.
	pub browser_kind: BrowserKind,
	/// Optional CDP endpoint used for attach flows.
	pub cdp_endpoint: Option<String>,
	/// Whether to launch Playwright browser server mode.
	pub launch_server: bool,
	/// URL patterns excluded from page-reuse candidates.
	pub protected_urls: Vec<String>,
	/// Preferred URL for page-reuse candidate selection.
	pub preferred_url: Option<String>,
	/// HAR recording configuration.
	pub har: HarConfig,
	/// Request-blocking configuration.
	pub block: BlockConfig,
	/// Download-tracking configuration.
	pub download: DownloadConfig,
}

impl SessionConfig {
	/// Creates a baseline config with default browser/session behavior.
	pub fn new(wait_until: WaitUntil) -> Self {
		Self {
			wait_until,
			storage_state: None,
			headless: true,
			browser_kind: BrowserKind::default(),
			cdp_endpoint: None,
			launch_server: false,
			protected_urls: Vec::new(),
			preferred_url: None,
			har: HarConfig::default(),
			block: BlockConfig::default(),
			download: DownloadConfig::default(),
		}
	}

	/// Returns true when context creation must use explicit options.
	pub(crate) fn needs_custom_context(&self) -> bool {
		self.storage_state.is_some() || self.har.is_enabled() || self.download.is_enabled()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn session_config_defaults_do_not_require_custom_context() {
		let cfg = SessionConfig::new(WaitUntil::NetworkIdle);
		assert!(!cfg.needs_custom_context());
	}

	#[test]
	fn session_config_requires_custom_context_for_storage_state() {
		let mut cfg = SessionConfig::new(WaitUntil::NetworkIdle);
		cfg.storage_state = Some(StorageState {
			cookies: Vec::new(),
			origins: Vec::new(),
		});
		assert!(cfg.needs_custom_context());
	}

	#[test]
	fn session_config_requires_custom_context_for_har_or_downloads() {
		let mut har_cfg = SessionConfig::new(WaitUntil::NetworkIdle);
		har_cfg.har.path = Some("network.har".into());
		assert!(har_cfg.needs_custom_context());

		let mut dl_cfg = SessionConfig::new(WaitUntil::NetworkIdle);
		dl_cfg.download.dir = Some("downloads".into());
		assert!(dl_cfg.needs_custom_context());
	}
}
