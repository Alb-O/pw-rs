//! Session orchestration for browser acquisition and lifecycle.

use std::fs;
use std::path::{Path, PathBuf};

use pw_rs::{StorageState, WaitUntil};
use tracing::{debug, warn};

use super::descriptor::{DRIVER_HASH, SESSION_DESCRIPTOR_SCHEMA_VERSION, SessionDescriptor, now_ts};
use super::strategy::{PrimarySessionStrategy, SessionStrategyInput, resolve_session_strategy};
use crate::artifact_collector::{CollectedArtifacts, collect_failure_artifacts};
use crate::browser::{BrowserSession, DownloadInfo, SessionOptions};
use crate::context::{BlockConfig, CommandContext, DownloadConfig, HarConfig};
use crate::daemon;
use crate::error::{PwError, Result};
use crate::output::SessionSource;
use crate::target::Target;
use crate::types::BrowserKind;

/// Fully resolved request for acquiring a browser session.
pub struct SessionRequest<'a> {
	/// Navigation wait strategy used by session page operations.
	pub wait_until: WaitUntil,
	/// Whether the session should run headless.
	pub headless: bool,
	/// Optional auth file used to bootstrap storage state.
	pub auth_file: Option<&'a Path>,
	/// Browser engine to launch/connect.
	pub browser: BrowserKind,
	/// Optional CDP endpoint to attach to an existing browser.
	pub cdp_endpoint: Option<&'a str>,
	/// Whether to launch a browser server instead of direct launch.
	pub launch_server: bool,
	/// Remote debugging port for persistent Chromium sessions.
	pub remote_debugging_port: Option<u16>,
	/// Whether browser lifecycle should outlive the session handle.
	pub keep_browser_running: bool,
	/// URL patterns excluded from page-reuse selection.
	pub protected_urls: &'a [String],
	/// Preferred URL for page-reuse selection.
	pub preferred_url: Option<&'a str>,
	/// HAR recording configuration.
	pub har_config: &'a HarConfig,
	/// Request-blocking configuration.
	pub block_config: &'a BlockConfig,
	/// Download-tracking configuration.
	pub download_config: &'a DownloadConfig,
}

impl<'a> SessionRequest<'a> {
	/// Builds a request from global command context defaults.
	pub fn from_context(wait_until: WaitUntil, ctx: &'a CommandContext) -> Self {
		Self {
			wait_until,
			headless: true,
			auth_file: ctx.auth_file(),
			browser: ctx.browser,
			cdp_endpoint: ctx.cdp_endpoint(),
			launch_server: ctx.launch_server(),
			remote_debugging_port: None,
			keep_browser_running: false,
			protected_urls: &[],
			preferred_url: None,
			har_config: ctx.har_config(),
			block_config: ctx.block_config(),
			download_config: ctx.download_config(),
		}
	}

	/// Sets protected URL patterns for page-reuse filtering.
	pub fn with_protected_urls(mut self, urls: &'a [String]) -> Self {
		self.protected_urls = urls;
		self
	}

	/// Sets headless/headful mode.
	pub fn with_headless(mut self, headless: bool) -> Self {
		self.headless = headless;
		self
	}

	/// Sets the auth storage-state file.
	pub fn with_auth_file(mut self, auth_file: Option<&'a Path>) -> Self {
		self.auth_file = auth_file;
		self
	}

	/// Sets the target browser engine.
	pub fn with_browser(mut self, browser: BrowserKind) -> Self {
		self.browser = browser;
		self
	}

	/// Sets an explicit CDP endpoint for attach mode.
	pub fn with_cdp_endpoint(mut self, endpoint: Option<&'a str>) -> Self {
		self.cdp_endpoint = endpoint;
		self
	}

	/// Sets the persistent remote-debugging port.
	pub fn with_remote_debugging_port(mut self, port: Option<u16>) -> Self {
		self.remote_debugging_port = port;
		self
	}

	/// Controls whether browser shutdown is skipped on close.
	pub fn with_keep_browser_running(mut self, keep: bool) -> Self {
		self.keep_browser_running = keep;
		self
	}

	/// Sets the preferred URL used for tab/page reuse.
	pub fn with_preferred_url(mut self, url: Option<&'a str>) -> Self {
		self.preferred_url = url;
		self
	}
}

/// Session manager that applies strategy selection and orchestrates acquisition.
pub struct SessionManager<'a> {
	ctx: &'a CommandContext,
	descriptor_path: Option<PathBuf>,
	namespace_id: Option<String>,
	refresh: bool,
}

impl<'a> SessionManager<'a> {
	/// Creates a manager for the current command execution scope.
	pub fn new(ctx: &'a CommandContext, descriptor_path: Option<PathBuf>, namespace_id: Option<String>, refresh: bool) -> Self {
		Self {
			ctx,
			descriptor_path,
			namespace_id,
			refresh,
		}
	}

	/// Acquires a session using descriptor reuse, daemon leasing, or launch flows.
	pub async fn session(&mut self, request: SessionRequest<'_>) -> Result<SessionHandle> {
		let storage_state = match request.auth_file {
			Some(path) => Some(load_storage_state(path)?),
			None => None,
		};

		let strategy = resolve_session_strategy(SessionStrategyInput {
			has_descriptor_path: self.descriptor_path.is_some(),
			refresh: self.refresh,
			no_daemon: self.ctx.no_daemon(),
			browser: request.browser,
			cdp_endpoint: request.cdp_endpoint,
			remote_debugging_port: request.remote_debugging_port,
			launch_server: request.launch_server,
		});

		if let Some(path) = &self.descriptor_path {
			if self.refresh {
				let _ = fs::remove_file(path);
			} else if strategy.try_descriptor_reuse {
				if let Some(descriptor) = SessionDescriptor::load(path)? {
					if descriptor.belongs_to(self.ctx)
						&& descriptor.matches(request.browser, request.headless, request.cdp_endpoint, Some(DRIVER_HASH))
						&& descriptor.is_alive()
					{
						if let Some(endpoint) = descriptor.cdp_endpoint.as_deref().or(descriptor.ws_endpoint.as_deref()) {
							debug!(
								target = "pw.session",
								%endpoint,
								pid = descriptor.pid,
								"reusing existing browser via cdp"
							);
							let mut session = BrowserSession::with_options(SessionOptions {
								wait_until: request.wait_until,
								storage_state: storage_state.clone(),
								headless: request.headless,
								browser_kind: request.browser,
								cdp_endpoint: Some(endpoint),
								launch_server: false,
								protected_urls: request.protected_urls,
								preferred_url: request.preferred_url,
								har_config: request.har_config,
								block_config: request.block_config,
								download_config: request.download_config,
							})
							.await?;
							session.set_keep_browser_running(true);
							return Ok(SessionHandle {
								session,
								source: SessionSource::CachedDescriptor,
							});
						} else {
							debug!(target = "pw.session", "descriptor lacks endpoint; ignoring");
						}
					}
				}
			}
		}

		let mut daemon_endpoint = None;
		let mut daemon_session_key = None;
		if strategy.try_daemon_lease {
			if let Some(client) = daemon::try_connect().await {
				if let Some(namespace_id) = &self.namespace_id {
					let session_key = format!("{}:{}:{}", namespace_id, request.browser, if request.headless { "headless" } else { "headful" });
					match daemon::request_browser(&client, request.browser, request.headless, &session_key).await {
						Ok(endpoint) => {
							debug!(
								target = "pw.session",
								%endpoint,
								session_key = %session_key,
								"using daemon browser"
							);
							daemon_endpoint = Some(endpoint);
							daemon_session_key = Some(session_key);
						}
						Err(err) => {
							debug!(
								target = "pw.session",
								error = %err,
								"daemon request failed; falling back"
							);
						}
					}
				}
			}
		}

		let (session, source) = if let Some(endpoint) = daemon_endpoint.as_deref() {
			let mut s = BrowserSession::with_options(SessionOptions {
				wait_until: request.wait_until,
				storage_state: storage_state.clone(),
				headless: request.headless,
				browser_kind: request.browser,
				cdp_endpoint: Some(endpoint),
				launch_server: false,
				protected_urls: request.protected_urls,
				preferred_url: request.preferred_url,
				har_config: request.har_config,
				block_config: request.block_config,
				download_config: request.download_config,
			})
			.await?;
			s.set_keep_browser_running(true);
			(s, SessionSource::Daemon)
		} else {
			match strategy.primary {
				PrimarySessionStrategy::AttachCdp => {
					let endpoint = request
						.cdp_endpoint
						.ok_or_else(|| PwError::Context("missing CDP endpoint for attach strategy".to_string()))?;
					let mut s = BrowserSession::with_options(SessionOptions {
						wait_until: request.wait_until,
						storage_state,
						headless: request.headless,
						browser_kind: request.browser,
						cdp_endpoint: Some(endpoint),
						launch_server: false,
						protected_urls: request.protected_urls,
						preferred_url: request.preferred_url,
						har_config: request.har_config,
						block_config: request.block_config,
						download_config: request.download_config,
					})
					.await?;
					s.set_keep_browser_running(true);
					(s, SessionSource::CdpConnect)
				}
				PrimarySessionStrategy::PersistentDebug => {
					let port = request
						.remote_debugging_port
						.ok_or_else(|| PwError::Context("missing remote_debugging_port for persistent strategy".to_string()))?;
					if request.browser != BrowserKind::Chromium {
						return Err(PwError::BrowserLaunch(
							"Persistent sessions with remote_debugging_port require Chromium".to_string(),
						));
					}
					let s = BrowserSession::launch_persistent(request.wait_until, storage_state, request.headless, port, request.keep_browser_running).await?;
					(s, SessionSource::PersistentDebug)
				}
				PrimarySessionStrategy::LaunchServer => {
					let s = BrowserSession::launch_server_session(request.wait_until, storage_state, request.headless, request.browser).await?;
					(s, SessionSource::BrowserServer)
				}
				PrimarySessionStrategy::FreshLaunch => {
					let s = BrowserSession::with_options(SessionOptions {
						wait_until: request.wait_until,
						storage_state,
						headless: request.headless,
						browser_kind: request.browser,
						cdp_endpoint: None,
						launch_server: false,
						protected_urls: request.protected_urls,
						preferred_url: request.preferred_url,
						har_config: request.har_config,
						block_config: request.block_config,
						download_config: request.download_config,
					})
					.await?;
					(s, SessionSource::Fresh)
				}
			}
		};

		let attached_endpoint = request.cdp_endpoint.is_some() || daemon_endpoint.is_some();
		if attached_endpoint && request.auth_file.is_none() {
			let auth_files = self.ctx.auth_files();
			if !auth_files.is_empty() {
				debug!(
					target = "pw.session",
					count = auth_files.len(),
					"auto-injecting cookies from project auth files"
				);
				session.inject_auth_files(&auth_files).await?;
			}
		}

		if let Some(path) = &self.descriptor_path {
			let cdp = session.cdp_endpoint().map(|e| e.to_string());
			let ws = session.ws_endpoint().map(|e| e.to_string());

			if cdp.is_some() || ws.is_some() {
				let descriptor = SessionDescriptor {
					schema_version: SESSION_DESCRIPTOR_SCHEMA_VERSION,
					pid: std::process::id(),
					browser: request.browser,
					headless: request.headless,
					cdp_endpoint: cdp,
					ws_endpoint: ws,
					workspace_id: Some(self.ctx.workspace_id().to_string()),
					namespace: Some(self.ctx.namespace().to_string()),
					session_key: daemon_session_key.or_else(|| Some(self.ctx.session_key(request.browser, request.headless).to_string())),
					driver_hash: Some(DRIVER_HASH.to_string()),
					created_at: now_ts(),
				};
				if let Err(err) = descriptor.save(path) {
					warn!(
						target = "pw.session",
						path = %path.display(),
						error = %err,
						"failed to save session descriptor"
					);
				} else {
					debug!(
						target = "pw.session",
						cdp = ?descriptor.cdp_endpoint,
						ws = ?descriptor.ws_endpoint,
						"saved session descriptor"
					);
				}
			} else {
				debug!(target = "pw.session", "no endpoint available; skipping descriptor save");
			}
		}

		Ok(SessionHandle { session, source })
	}

	/// Returns immutable command context used by this manager.
	pub fn context(&self) -> &'a CommandContext {
		self.ctx
	}
}

/// Active session handle used by command flows.
pub struct SessionHandle {
	session: BrowserSession,
	source: SessionSource,
}

impl SessionHandle {
	/// Returns where this session was sourced from.
	pub fn source(&self) -> SessionSource {
		self.source
	}

	/// Navigates to a URL.
	pub async fn goto(&self, url: &str, timeout_ms: Option<u64>) -> Result<()> {
		self.session.goto(url, timeout_ms).await
	}

	/// Navigates only when current URL differs from `url`.
	pub async fn goto_if_needed(&self, url: &str, timeout_ms: Option<u64>) -> Result<bool> {
		let current_url = self.page().evaluate_value("window.location.href").await.unwrap_or_else(|_| self.page().url());
		let current = current_url.trim_matches('"');

		if urls_match(current, url) {
			Ok(false)
		} else {
			self.session.goto(url, timeout_ms).await?;
			Ok(true)
		}
	}

	/// Navigates according to typed [`Target`] semantics.
	pub async fn goto_target(&self, target: &Target, timeout_ms: Option<u64>) -> Result<bool> {
		match target {
			Target::Navigate(url) => self.goto_if_needed(url.as_str(), timeout_ms).await,
			Target::CurrentPage => Ok(false),
		}
	}

	/// Returns the active page.
	pub fn page(&self) -> &pw_rs::Page {
		self.session.page()
	}

	/// Returns the active browser context.
	pub fn context(&self) -> &pw_rs::BrowserContext {
		self.session.context()
	}

	/// Returns WebSocket endpoint when available.
	pub fn ws_endpoint(&self) -> Option<&str> {
		self.session.ws_endpoint()
	}

	/// Returns CDP endpoint when available.
	pub fn cdp_endpoint(&self) -> Option<&str> {
		self.session.cdp_endpoint()
	}

	/// Returns browser handle.
	pub fn browser(&self) -> &pw_rs::Browser {
		self.session.browser()
	}

	/// Returns downloads observed in this session.
	pub fn downloads(&self) -> Vec<DownloadInfo> {
		self.session.downloads()
	}

	/// Closes session resources.
	pub async fn close(self) -> Result<()> {
		self.session.close().await
	}

	/// Shuts down launched browser server (when applicable).
	pub async fn shutdown_server(self) -> Result<()> {
		self.session.shutdown_server().await
	}

	/// Collects failure artifacts from current page state.
	pub async fn collect_failure_artifacts(&self, artifacts_dir: Option<&Path>, command_name: &str) -> CollectedArtifacts {
		match artifacts_dir {
			Some(dir) => collect_failure_artifacts(self.page(), dir, command_name).await,
			None => CollectedArtifacts::default(),
		}
	}
}

fn load_storage_state(path: &Path) -> Result<StorageState> {
	StorageState::from_file(path).map_err(|e| PwError::BrowserLaunch(format!("Failed to load auth file: {}", e)))
}

fn urls_match(current: &str, target: &str) -> bool {
	if current == target {
		return true;
	}

	let current_normalized = current.trim_end_matches('/');
	let target_normalized = target.trim_end_matches('/');

	current_normalized == target_normalized
}

#[cfg(test)]
mod tests {
	use tempfile::tempdir;

	use super::*;

	static DEFAULT_HAR_CONFIG: HarConfig = HarConfig {
		path: None,
		content_policy: None,
		mode: None,
		omit_content: false,
		url_filter: None,
	};

	static DEFAULT_BLOCK_CONFIG: BlockConfig = BlockConfig { patterns: Vec::new() };
	static DEFAULT_DOWNLOAD_CONFIG: DownloadConfig = DownloadConfig { dir: None };

	#[test]
	fn descriptor_round_trip_and_match() {
		let dir = tempdir().unwrap();
		let path = dir.path().join("session.json");

		let desc = SessionDescriptor {
			schema_version: SESSION_DESCRIPTOR_SCHEMA_VERSION,
			pid: std::process::id(),
			browser: BrowserKind::Chromium,
			headless: true,
			cdp_endpoint: Some("ws://localhost:1234".into()),
			ws_endpoint: Some("ws://localhost:1234".into()),
			workspace_id: Some("ws".into()),
			namespace: Some("default".into()),
			session_key: Some("ws:default:chromium:headless".into()),
			driver_hash: Some(DRIVER_HASH.to_string()),
			created_at: 123,
		};

		desc.save(&path).unwrap();
		let loaded = SessionDescriptor::load(&path).unwrap().unwrap();
		assert!(loaded.is_alive());
		assert!(loaded.matches(BrowserKind::Chromium, true, Some("ws://localhost:1234"), Some(DRIVER_HASH)));
	}

	#[test]
	fn descriptor_save_creates_state_gitignore_for_state_paths() {
		let dir = tempdir().unwrap();
		let path = dir
			.path()
			.join("playwright")
			.join(crate::workspace::STATE_VERSION_DIR)
			.join("profiles")
			.join("default")
			.join("sessions")
			.join("session.json");

		let desc = SessionDescriptor {
			schema_version: SESSION_DESCRIPTOR_SCHEMA_VERSION,
			pid: std::process::id(),
			browser: BrowserKind::Chromium,
			headless: true,
			cdp_endpoint: Some("ws://localhost:1234".into()),
			ws_endpoint: Some("ws://localhost:1234".into()),
			workspace_id: Some("ws".into()),
			namespace: Some("default".into()),
			session_key: Some("ws:default:chromium:headless".into()),
			driver_hash: Some(DRIVER_HASH.to_string()),
			created_at: 123,
		};

		desc.save(&path).unwrap();
		let gitignore = dir.path().join("playwright").join(crate::workspace::STATE_VERSION_DIR).join(".gitignore");
		assert!(gitignore.exists());
		assert_eq!(std::fs::read_to_string(gitignore).unwrap(), "*\n");
	}

	#[test]
	fn descriptor_mismatch_when_endpoint_differs() {
		let desc = SessionDescriptor {
			schema_version: SESSION_DESCRIPTOR_SCHEMA_VERSION,
			pid: std::process::id(),
			browser: BrowserKind::Chromium,
			headless: true,
			cdp_endpoint: Some("ws://localhost:9999".into()),
			ws_endpoint: Some("ws://localhost:9999".into()),
			workspace_id: Some("ws".into()),
			namespace: Some("default".into()),
			session_key: Some("ws:default:chromium:headless".into()),
			driver_hash: Some(DRIVER_HASH.to_string()),
			created_at: 0,
		};

		assert!(!desc.matches(BrowserKind::Chromium, true, Some("ws://localhost:1234"), Some(DRIVER_HASH)));
	}

	#[test]
	fn descriptor_invalidated_by_driver_hash_change() {
		let desc = SessionDescriptor {
			schema_version: SESSION_DESCRIPTOR_SCHEMA_VERSION,
			pid: std::process::id(),
			browser: BrowserKind::Chromium,
			headless: true,
			cdp_endpoint: Some("ws://localhost:1234".into()),
			ws_endpoint: Some("ws://localhost:1234".into()),
			workspace_id: Some("ws".into()),
			namespace: Some("default".into()),
			session_key: Some("ws:default:chromium:headless".into()),
			driver_hash: Some("old-hash".into()),
			created_at: 42,
		};

		assert!(!desc.matches(BrowserKind::Chromium, true, Some("ws://localhost:1234"), Some(DRIVER_HASH)));
	}

	#[test]
	fn test_urls_match() {
		assert!(urls_match("https://example.com", "https://example.com"));
		assert!(urls_match("https://example.com/", "https://example.com"));
		assert!(urls_match("https://example.com", "https://example.com/"));
		assert!(urls_match("https://example.com/path/", "https://example.com/path"));

		assert!(!urls_match("https://example.com", "https://other.com"));
		assert!(!urls_match("https://example.com/a", "https://example.com/b"));
		assert!(!urls_match("https://example.com", "http://example.com"));
	}

	#[test]
	fn session_request_builders_round_trip() {
		let ctx = CommandContext::new(BrowserKind::Chromium, false, None, None, false, false);
		let request = SessionRequest::from_context(WaitUntil::NetworkIdle, &ctx)
			.with_headless(false)
			.with_browser(BrowserKind::Chromium)
			.with_auth_file(None)
			.with_cdp_endpoint(Some("http://127.0.0.1:9222"))
			.with_remote_debugging_port(Some(9555))
			.with_keep_browser_running(true)
			.with_preferred_url(Some("https://example.com"))
			.with_protected_urls(&[]);
		assert!(!request.headless);
		assert_eq!(request.cdp_endpoint, Some("http://127.0.0.1:9222"));
		assert_eq!(request.remote_debugging_port, Some(9555));
		assert!(request.keep_browser_running);
		assert_eq!(request.preferred_url, Some("https://example.com"));
	}

	#[test]
	fn descriptor_match_helper_handles_no_requested_endpoint() {
		let desc = SessionDescriptor {
			schema_version: SESSION_DESCRIPTOR_SCHEMA_VERSION,
			pid: std::process::id(),
			browser: BrowserKind::Chromium,
			headless: true,
			cdp_endpoint: Some("ws://localhost:1234".into()),
			ws_endpoint: None,
			workspace_id: Some("ws".into()),
			namespace: Some("default".into()),
			session_key: Some("ws:default:chromium:headless".into()),
			driver_hash: Some(DRIVER_HASH.to_string()),
			created_at: 0,
		};
		assert!(desc.matches(BrowserKind::Chromium, true, None, Some(DRIVER_HASH)));
	}

	#[test]
	fn descriptor_match_helper_respects_browser_and_headless() {
		let desc = SessionDescriptor {
			schema_version: SESSION_DESCRIPTOR_SCHEMA_VERSION,
			pid: std::process::id(),
			browser: BrowserKind::Chromium,
			headless: true,
			cdp_endpoint: Some("ws://localhost:1234".into()),
			ws_endpoint: None,
			workspace_id: Some("ws".into()),
			namespace: Some("default".into()),
			session_key: Some("ws:default:chromium:headless".into()),
			driver_hash: Some(DRIVER_HASH.to_string()),
			created_at: 0,
		};
		assert!(!desc.matches(BrowserKind::Firefox, true, Some("ws://localhost:1234"), Some(DRIVER_HASH)));
		assert!(!desc.matches(BrowserKind::Chromium, false, Some("ws://localhost:1234"), Some(DRIVER_HASH)));
	}

	#[test]
	fn default_configs_are_accessible() {
		let request = SessionRequest {
			wait_until: WaitUntil::NetworkIdle,
			headless: true,
			auth_file: None,
			browser: BrowserKind::Chromium,
			cdp_endpoint: None,
			launch_server: false,
			remote_debugging_port: None,
			keep_browser_running: false,
			protected_urls: &[],
			preferred_url: None,
			har_config: &DEFAULT_HAR_CONFIG,
			block_config: &DEFAULT_BLOCK_CONFIG,
			download_config: &DEFAULT_DOWNLOAD_CONFIG,
		};
		assert_eq!(request.block_config.patterns.len(), 0);
		assert!(request.download_config.dir.is_none());
	}
}
