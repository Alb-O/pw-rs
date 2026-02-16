mod builder;
mod config;
mod context_factory;
mod features;
mod page_selection;
mod shutdown;
mod types;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub use config::SessionConfig;
use pw_rs::{BrowserContextOptions, GotoOptions, Playwright, StorageState, Subscription, WaitUntil};
pub use shutdown::ShutdownMode;
use tracing::debug;
pub use types::{AuthInjectionReport, DownloadInfo, SessionEndpoints};

use self::features::har::HarRecording;
use crate::error::{PwError, Result};
use crate::types::BrowserKind;

/// Active browser session used by command flows.
///
/// A session owns Playwright runtime handles and optional feature state
/// (HAR recording, request blocking, download tracking), and exposes
/// explicit shutdown semantics through [`ShutdownMode`].
pub struct BrowserSession {
	_playwright: Playwright,
	browser: pw_rs::Browser,
	context: pw_rs::BrowserContext,
	page: pw_rs::Page,
	wait_until: WaitUntil,
	endpoints: SessionEndpoints,
	launched_server: Option<pw_rs::LaunchedServer>,
	shutdown_mode: ShutdownMode,
	har_recording: Option<HarRecording>,
	#[allow(dead_code, reason = "RAII: stored to keep handlers alive until drop")]
	route_subscriptions: Vec<Subscription>,
	#[allow(dead_code, reason = "RAII: stored to keep handler alive until drop")]
	download_subscription: Option<Subscription>,
	downloads: Arc<Mutex<Vec<DownloadInfo>>>,
}

impl BrowserSession {
	/// Creates a session with default browser configuration.
	pub async fn new(wait_until: WaitUntil) -> Result<Self> {
		Self::with_config(SessionConfig::new(wait_until)).await
	}

	/// Creates a session with optional auth file and default browser selection.
	pub async fn with_auth(wait_until: WaitUntil, auth_file: Option<&Path>, cdp_endpoint: Option<&str>) -> Result<Self> {
		Self::with_auth_and_browser(wait_until, auth_file, BrowserKind::default(), cdp_endpoint).await
	}

	/// Creates a session with optional auth file and explicit browser kind.
	pub async fn with_auth_and_browser(wait_until: WaitUntil, auth_file: Option<&Path>, browser_kind: BrowserKind, cdp_endpoint: Option<&str>) -> Result<Self> {
		match auth_file {
			Some(path) => Self::with_auth_file_and_browser(wait_until, path, browser_kind, cdp_endpoint).await,
			None => {
				let mut config = SessionConfig::new(wait_until);
				config.browser_kind = browser_kind;
				config.cdp_endpoint = cdp_endpoint.map(str::to_string);
				Self::with_config(config).await
			}
		}
	}

	/// Creates a session seeded from auth storage-state file.
	pub async fn with_auth_file(wait_until: WaitUntil, auth_file: &Path) -> Result<Self> {
		Self::with_auth_file_and_browser(wait_until, auth_file, BrowserKind::default(), None).await
	}

	/// Creates a session seeded from auth storage-state file and browser kind.
	pub async fn with_auth_file_and_browser(wait_until: WaitUntil, auth_file: &Path, browser_kind: BrowserKind, cdp_endpoint: Option<&str>) -> Result<Self> {
		let mut config = SessionConfig::new(wait_until);
		config.storage_state = Some(load_storage_state(auth_file)?);
		config.browser_kind = browser_kind;
		config.cdp_endpoint = cdp_endpoint.map(str::to_string);
		Self::with_config(config).await
	}

	/// Creates a session in browser-server mode.
	pub async fn launch_server_session(wait_until: WaitUntil, storage_state: Option<StorageState>, headless: bool, browser_kind: BrowserKind) -> Result<Self> {
		let mut config = SessionConfig::new(wait_until);
		config.storage_state = storage_state;
		config.headless = headless;
		config.browser_kind = browser_kind;
		config.launch_server = true;
		Self::with_config(config).await
	}

	/// Launches a persistent Chromium session with explicit remote-debugging port.
	pub async fn launch_persistent(
		wait_until: WaitUntil,
		storage_state: Option<StorageState>,
		headless: bool,
		remote_debugging_port: u16,
		keep_browser_running: bool,
	) -> Result<Self> {
		debug!(
			target = "pw",
			browser = "chromium",
			port = remote_debugging_port,
			keep_browser_running,
			"launching persistent session..."
		);

		let mut playwright = Playwright::launch().await.map_err(|e| PwError::BrowserLaunch(e.to_string()))?;
		if keep_browser_running {
			playwright.keep_server_running();
		}

		let launch_options = pw_rs::LaunchOptions {
			headless: Some(headless),
			remote_debugging_port: Some(remote_debugging_port),
			handle_sighup: Some(!keep_browser_running),
			handle_sigint: Some(!keep_browser_running),
			handle_sigterm: Some(!keep_browser_running),
			..Default::default()
		};

		let browser = playwright.chromium().launch_with_options(launch_options).await?;
		let context = if let Some(state) = storage_state {
			let options = BrowserContextOptions::builder().storage_state(state).build();
			browser.new_context_with_options(options).await?
		} else {
			browser.new_context().await?
		};
		let page = context.new_page().await?;

		Ok(Self {
			_playwright: playwright,
			browser,
			context,
			page,
			wait_until,
			endpoints: SessionEndpoints {
				ws: None,
				cdp: Some(format!("http://localhost:{}", remote_debugging_port)),
			},
			launched_server: None,
			shutdown_mode: if keep_browser_running {
				ShutdownMode::KeepBrowserAlive
			} else {
				ShutdownMode::CloseSessionOnly
			},
			har_recording: None,
			route_subscriptions: Vec::new(),
			download_subscription: None,
			downloads: Arc::new(Mutex::new(Vec::new())),
		})
	}

	/// Builds a session from fully owned configuration.
	pub async fn with_config(config: SessionConfig) -> Result<Self> {
		builder::build(config).await
	}

	/// Navigates the active page to a URL with optional timeout.
	pub async fn goto(&self, url: &str, timeout_ms: Option<u64>) -> Result<()> {
		let mut goto_opts = GotoOptions {
			wait_until: Some(self.wait_until),
			..Default::default()
		};
		if let Some(ms) = timeout_ms {
			goto_opts.timeout = Some(std::time::Duration::from_millis(ms));
		}

		self.page.goto(url, Some(goto_opts)).await.map(|_| ()).map_err(|e| PwError::Navigation {
			url: url.to_string(),
			source: anyhow::Error::new(e),
		})
	}

	/// Returns the active page handle.
	pub fn page(&self) -> &pw_rs::Page {
		&self.page
	}

	/// Returns the active browser context handle.
	pub fn context(&self) -> &pw_rs::BrowserContext {
		&self.context
	}

	/// Returns discovered session endpoints.
	pub fn endpoints(&self) -> &SessionEndpoints {
		&self.endpoints
	}

	/// Returns active browser handle.
	pub fn browser(&self) -> &pw_rs::Browser {
		&self.browser
	}

	/// Returns downloads collected during this session.
	pub fn downloads(&self) -> Vec<DownloadInfo> {
		self.downloads.lock().unwrap().clone()
	}

	/// Updates default close behavior used by higher-level session handles.
	pub fn set_shutdown_mode(&mut self, mode: ShutdownMode) {
		self.shutdown_mode = mode;
	}

	/// Returns default close behavior used by higher-level session handles.
	pub fn shutdown_mode(&self) -> ShutdownMode {
		self.shutdown_mode
	}

	/// Injects cookies from auth storage-state files into current browser context.
	pub async fn inject_auth_files(&self, auth_files: &[PathBuf]) -> Result<AuthInjectionReport> {
		let mut report = AuthInjectionReport {
			files_seen: auth_files.len(),
			..Default::default()
		};

		for path in auth_files {
			match load_storage_state(path) {
				Ok(state) => {
					report.files_loaded += 1;
					let cookie_count = state.cookies.len();
					if cookie_count == 0 {
						continue;
					}

					debug!(
						target = "pw",
						path = %path.display(),
						count = cookie_count,
						"injecting cookies from auth file"
					);
					self.context.add_cookies(state.cookies).await?;
					report.cookies_added += cookie_count;
				}
				Err(err) => {
					debug!(
						target = "pw",
						path = %path.display(),
						error = %err,
						"failed to load auth file, skipping"
					);
				}
			}
		}

		Ok(report)
	}

	/// Shuts down session resources according to explicit mode.
	pub async fn shutdown(mut self, mode: ShutdownMode) -> Result<()> {
		features::har::export_if_active(&self.context, self.har_recording.as_ref()).await;
		let _ = self.context.close().await;

		match mode {
			ShutdownMode::CloseSessionOnly => {
				self.browser.close().await?;
			}
			ShutdownMode::KeepBrowserAlive => {}
			ShutdownMode::ShutdownServer => {
				if let Some(server) = self.launched_server.take() {
					server.close().await?;
					self._playwright.enable_server_shutdown();
				} else {
					self.browser.close().await?;
				}
			}
		}

		Ok(())
	}
}

fn load_storage_state(path: &Path) -> Result<StorageState> {
	StorageState::from_file(path).map_err(|e| PwError::BrowserLaunch(format!("Failed to load auth file: {}", e)))
}

#[cfg(test)]
mod tests {
	use std::fs;

	use tempfile::TempDir;

	use super::*;

	#[test]
	fn load_storage_state_errors_for_missing_file() {
		let err = load_storage_state(Path::new("/definitely/missing/auth.json")).unwrap_err();
		assert!(err.to_string().contains("Failed to load auth file"));
	}

	#[test]
	fn load_storage_state_accepts_storage_state_file() {
		let temp = TempDir::new().unwrap();
		let auth_file = temp.path().join("auth.json");
		fs::write(
			&auth_file,
			r#"{
  "cookies": [
    {
      "name": "session",
      "value": "token",
      "domain": ".example.com",
      "path": "/",
      "expires": -1.0,
      "httpOnly": true,
      "secure": true,
      "sameSite": "Lax"
    }
  ],
  "origins": []
}"#,
		)
		.unwrap();

		let state = load_storage_state(&auth_file).unwrap();
		assert_eq!(state.cookies.len(), 1);
		assert_eq!(state.origins.len(), 0);
	}
}
