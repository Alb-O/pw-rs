use std::fs;
use std::path::{Path, PathBuf};

use crate::artifact_collector::{CollectedArtifacts, collect_failure_artifacts};
use crate::browser::{BrowserSession, DownloadInfo};
use crate::context::{BlockConfig, CommandContext, DownloadConfig, HarConfig};
use crate::daemon;
use crate::error::{PwError, Result};
use crate::target::Target;
use crate::types::BrowserKind;
use pw::{StorageState, WaitUntil};
use serde::{Deserialize, Serialize};
use tracing::debug;

const DRIVER_HASH: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SessionDescriptor {
    pub(crate) pid: u32,
    pub(crate) browser: BrowserKind,
    pub(crate) headless: bool,
    pub(crate) cdp_endpoint: Option<String>,
    pub(crate) ws_endpoint: Option<String>,
    pub(crate) driver_hash: Option<String>,
    pub(crate) created_at: u64,
}

impl SessionDescriptor {
    pub(crate) fn load(path: &Path) -> Result<Option<Self>> {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(PwError::Io(err)),
        };

        let parsed: Self = serde_json::from_str(&content)?;
        Ok(Some(parsed))
    }

    pub(crate) fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub(crate) fn matches(&self, request: &SessionRequest<'_>, driver_hash: Option<&str>) -> bool {
        let endpoint_match = if let Some(req_endpoint) = request.cdp_endpoint {
            // Specific endpoint requested - must match
            self.cdp_endpoint.as_deref() == Some(req_endpoint)
                || self.ws_endpoint.as_deref() == Some(req_endpoint)
        } else {
            // No specific endpoint requested - match if we have any endpoint
            self.ws_endpoint.is_some() || self.cdp_endpoint.is_some()
        };

        let driver_match = match (driver_hash, self.driver_hash.as_deref()) {
            (Some(expected), Some(actual)) => expected == actual,
            (None, _) => true,
            (_, None) => true,
        };

        self.browser == request.browser
            && self.headless == request.headless
            && endpoint_match
            && driver_match
    }

    pub(crate) fn is_alive(&self) -> bool {
        // Best-effort: on Linux, check /proc; otherwise assume alive if pid matches current process
        let proc_path = PathBuf::from("/proc").join(self.pid.to_string());
        proc_path.exists()
    }
}

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Request for a browser session; future reuse/daemon logic will live here.
pub struct SessionRequest<'a> {
    pub wait_until: WaitUntil,
    pub headless: bool,
    pub auth_file: Option<&'a Path>,
    pub browser: BrowserKind,
    pub cdp_endpoint: Option<&'a str>,
    pub launch_server: bool,
    /// Remote debugging port for persistent sessions (Chromium only).
    /// When set, launches with --remote-debugging-port and enables CDP reconnection.
    pub remote_debugging_port: Option<u16>,
    /// Whether to keep the browser running after the session closes.
    pub keep_browser_running: bool,
    /// URL patterns to exclude when selecting which existing page to reuse.
    pub protected_urls: &'a [String],
    /// Preferred URL to match when selecting which page to reuse.
    /// When set, pages matching this URL are preferred over other non-protected pages.
    pub preferred_url: Option<&'a str>,
    /// HAR recording configuration
    pub har_config: &'a HarConfig,
    /// Request blocking configuration
    pub block_config: &'a BlockConfig,
    /// Download management configuration
    pub download_config: &'a DownloadConfig,
}

impl<'a> SessionRequest<'a> {
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

    pub fn with_protected_urls(mut self, urls: &'a [String]) -> Self {
        self.protected_urls = urls;
        self
    }

    pub fn with_headless(mut self, headless: bool) -> Self {
        self.headless = headless;
        self
    }

    pub fn with_auth_file(mut self, auth_file: Option<&'a Path>) -> Self {
        self.auth_file = auth_file;
        self
    }

    pub fn with_browser(mut self, browser: BrowserKind) -> Self {
        self.browser = browser;
        self
    }

    pub fn with_cdp_endpoint(mut self, endpoint: Option<&'a str>) -> Self {
        self.cdp_endpoint = endpoint;
        self
    }

    pub fn with_remote_debugging_port(mut self, port: Option<u16>) -> Self {
        self.remote_debugging_port = port;
        self
    }

    pub fn with_keep_browser_running(mut self, keep: bool) -> Self {
        self.keep_browser_running = keep;
        self
    }

    pub fn with_preferred_url(mut self, url: Option<&'a str>) -> Self {
        self.preferred_url = url;
        self
    }
}

pub struct SessionBroker<'a> {
    ctx: &'a CommandContext,
    descriptor_path: Option<PathBuf>,
    refresh: bool,
}

impl<'a> SessionBroker<'a> {
    pub fn new(ctx: &'a CommandContext, descriptor_path: Option<PathBuf>, refresh: bool) -> Self {
        Self {
            ctx,
            descriptor_path,
            refresh,
        }
    }

    pub async fn session(&mut self, request: SessionRequest<'_>) -> Result<SessionHandle> {
        let storage_state = match request.auth_file {
            Some(path) => Some(load_storage_state(path)?),
            None => None,
        };

        if let Some(path) = &self.descriptor_path {
            if self.refresh {
                let _ = fs::remove_file(path);
            } else if let Some(descriptor) = SessionDescriptor::load(path)? {
                if descriptor.matches(&request, Some(DRIVER_HASH)) && descriptor.is_alive() {
                    // Prefer CDP endpoint (for persistent sessions) over ws_endpoint
                    if let Some(endpoint) = descriptor
                        .cdp_endpoint
                        .as_deref()
                        .or(descriptor.ws_endpoint.as_deref())
                    {
                        debug!(
                            target = "pw.session",
                            %endpoint,
                            pid = descriptor.pid,
                            "reusing existing browser via cdp"
                        );
                        let session = BrowserSession::with_options(
                            request.wait_until,
                            storage_state.clone(),
                            request.headless,
                            request.browser,
                            Some(endpoint),
                            false,
                            request.protected_urls,
                            request.preferred_url,
                            request.har_config,
                            request.block_config,
                            request.download_config,
                        )
                        .await?;
                        return Ok(SessionHandle { session });
                    } else {
                        debug!(target = "pw.session", "descriptor lacks endpoint; ignoring");
                    }
                }
            }
        }

        let mut daemon_endpoint = None;
        if !self.ctx.no_daemon()
            && request.cdp_endpoint.is_none()
            && request.remote_debugging_port.is_none()
            && !request.launch_server
            && request.browser == BrowserKind::Chromium
        {
            if let Some(client) = daemon::try_connect().await {
                // Use descriptor path as reuse_key for consistent browser reuse per context
                let reuse_key = self
                    .descriptor_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string());
                match daemon::request_browser(
                    &client,
                    request.browser,
                    request.headless,
                    reuse_key.as_deref(),
                )
                .await
                {
                    Ok(endpoint) => {
                        debug!(target = "pw.session", %endpoint, reuse_key = ?reuse_key, "using daemon browser");
                        daemon_endpoint = Some(endpoint);
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

        let session = if let Some(endpoint) = daemon_endpoint.as_deref() {
            let mut s = BrowserSession::with_options(
                request.wait_until,
                storage_state.clone(),
                request.headless,
                request.browser,
                Some(endpoint),
                false,
                request.protected_urls,
                request.preferred_url,
                request.har_config,
                request.block_config,
                request.download_config,
            )
            .await?;
            // Daemon manages the browser lifecycle - don't close it on session close
            s.set_keep_browser_running(true);
            s
        } else if let Some(port) = request.remote_debugging_port {
            // Persistent session with CDP debugging port (Chromium only)
            if request.browser != BrowserKind::Chromium {
                return Err(PwError::BrowserLaunch(
                    "Persistent sessions with remote_debugging_port require Chromium".to_string(),
                ));
            }
            BrowserSession::launch_persistent(
                request.wait_until,
                storage_state,
                request.headless,
                port,
                request.keep_browser_running,
            )
            .await?
        } else if request.launch_server {
            BrowserSession::launch_server_session(
                request.wait_until,
                storage_state,
                request.headless,
                request.browser,
            )
            .await?
        } else {
            BrowserSession::with_options(
                request.wait_until,
                storage_state,
                request.headless,
                request.browser,
                request.cdp_endpoint,
                false,
                request.protected_urls,
                request.preferred_url,
                request.har_config,
                request.block_config,
                request.download_config,
            )
            .await?
        };

        // Auto-inject cookies from project auth files when using CDP without explicit auth
        if request.cdp_endpoint.is_some() && request.auth_file.is_none() {
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

        // Save session descriptor if we have a path and an endpoint
        if let Some(path) = &self.descriptor_path {
            let cdp = session.cdp_endpoint().map(|e| e.to_string());
            let ws = session.ws_endpoint().map(|e| e.to_string());

            // Prefer CDP endpoint if available, otherwise use ws_endpoint
            if cdp.is_some() || ws.is_some() {
                let descriptor = SessionDescriptor {
                    pid: std::process::id(),
                    browser: request.browser,
                    headless: request.headless,
                    cdp_endpoint: cdp,
                    ws_endpoint: ws,
                    driver_hash: Some(DRIVER_HASH.to_string()),
                    created_at: now_ts(),
                };
                let _ = descriptor.save(path);
                debug!(
                    target = "pw.session",
                    cdp = ?descriptor.cdp_endpoint,
                    ws = ?descriptor.ws_endpoint,
                    "saved session descriptor"
                );
            } else {
                debug!(
                    target = "pw.session",
                    "no endpoint available; skipping descriptor save"
                );
            }
        }

        Ok(SessionHandle { session })
    }

    pub fn context(&self) -> &'a CommandContext {
        self.ctx
    }
}

pub struct SessionHandle {
    session: BrowserSession,
}

impl SessionHandle {
    pub async fn goto(&self, url: &str) -> Result<()> {
        self.session.goto(url).await
    }

    /// Navigate to URL only if not already on that page.
    ///
    /// Returns `true` if navigation was performed, `false` if already on the page.
    pub async fn goto_if_needed(&self, url: &str) -> Result<bool> {
        let current_url = self
            .page()
            .evaluate_value("window.location.href")
            .await
            .unwrap_or_else(|_| self.page().url());
        let current = current_url.trim_matches('"');

        if urls_match(current, url) {
            Ok(false)
        } else {
            self.session.goto(url).await?;
            Ok(true)
        }
    }

    /// Navigate based on a typed [`Target`].
    ///
    /// For `Target::Navigate(url)`, navigates to the URL (if not already there).
    /// For `Target::CurrentPage`, does nothing (operates on current page).
    ///
    /// Returns `true` if navigation was performed, `false` if skipped.
    pub async fn goto_target(&self, target: &Target) -> Result<bool> {
        match target {
            Target::Navigate(url) => self.goto_if_needed(url.as_str()).await,
            Target::CurrentPage => Ok(false),
        }
    }

    pub fn page(&self) -> &pw::Page {
        self.session.page()
    }

    pub fn context(&self) -> &pw::BrowserContext {
        self.session.context()
    }

    pub fn ws_endpoint(&self) -> Option<&str> {
        self.session.ws_endpoint()
    }

    pub fn cdp_endpoint(&self) -> Option<&str> {
        self.session.cdp_endpoint()
    }

    pub fn browser(&self) -> &pw::Browser {
        self.session.browser()
    }

    /// Returns downloads collected during this session.
    pub fn downloads(&self) -> Vec<DownloadInfo> {
        self.session.downloads()
    }

    pub async fn close(self) -> Result<()> {
        self.session.close().await
    }

    pub async fn shutdown_server(self) -> Result<()> {
        self.session.shutdown_server().await
    }

    /// Collect failure artifacts (screenshot, HTML) from the current page state.
    ///
    /// This should be called when a command fails after navigation to capture
    /// diagnostic information. Returns empty if artifacts_dir is None.
    pub async fn collect_failure_artifacts(
        &self,
        artifacts_dir: Option<&Path>,
        command_name: &str,
    ) -> CollectedArtifacts {
        match artifacts_dir {
            Some(dir) => collect_failure_artifacts(self.page(), dir, command_name).await,
            None => CollectedArtifacts::default(),
        }
    }
}

fn load_storage_state(path: &Path) -> Result<StorageState> {
    StorageState::from_file(path)
        .map_err(|e| PwError::BrowserLaunch(format!("Failed to load auth file: {}", e)))
}

/// Check if two URLs match for navigation purposes.
///
/// Handles common variations like trailing slashes.
fn urls_match(current: &str, target: &str) -> bool {
    // Exact match
    if current == target {
        return true;
    }

    // Normalize trailing slashes for comparison
    let current_normalized = current.trim_end_matches('/');
    let target_normalized = target.trim_end_matches('/');

    current_normalized == target_normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // Default HAR config for tests
    static DEFAULT_HAR_CONFIG: HarConfig = HarConfig {
        path: None,
        content_policy: None,
        mode: None,
        omit_content: false,
        url_filter: None,
    };

    // Default block config for tests
    static DEFAULT_BLOCK_CONFIG: BlockConfig = BlockConfig {
        patterns: Vec::new(),
    };

    // Default download config for tests
    static DEFAULT_DOWNLOAD_CONFIG: DownloadConfig = DownloadConfig { dir: None };

    #[test]
    fn descriptor_round_trip_and_match() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");

        let desc = SessionDescriptor {
            pid: std::process::id(),
            browser: BrowserKind::Chromium,
            headless: true,
            cdp_endpoint: Some("ws://localhost:1234".into()),
            ws_endpoint: Some("ws://localhost:1234".into()),
            driver_hash: Some(DRIVER_HASH.to_string()),
            created_at: 123,
        };

        desc.save(&path).unwrap();
        let loaded = SessionDescriptor::load(&path).unwrap().unwrap();
        assert!(loaded.is_alive());

        let req = SessionRequest {
            wait_until: WaitUntil::NetworkIdle,
            headless: true,
            auth_file: None,
            browser: BrowserKind::Chromium,
            cdp_endpoint: Some("ws://localhost:1234"),
            launch_server: true,
            remote_debugging_port: None,
            keep_browser_running: false,
            protected_urls: &[],
            preferred_url: None,
            har_config: &DEFAULT_HAR_CONFIG,
            block_config: &DEFAULT_BLOCK_CONFIG,
            download_config: &DEFAULT_DOWNLOAD_CONFIG,
        };
        assert!(loaded.matches(&req, Some(DRIVER_HASH)));
    }

    #[test]
    fn descriptor_mismatch_when_endpoint_differs() {
        let desc = SessionDescriptor {
            pid: std::process::id(),
            browser: BrowserKind::Chromium,
            headless: true,
            cdp_endpoint: Some("ws://localhost:9999".into()),
            ws_endpoint: Some("ws://localhost:9999".into()),
            driver_hash: Some(DRIVER_HASH.to_string()),
            created_at: 0,
        };

        let req = SessionRequest {
            wait_until: WaitUntil::NetworkIdle,
            headless: true,
            auth_file: None,
            browser: BrowserKind::Chromium,
            cdp_endpoint: Some("ws://localhost:1234"),
            launch_server: true,
            remote_debugging_port: None,
            keep_browser_running: false,
            protected_urls: &[],
            preferred_url: None,
            har_config: &DEFAULT_HAR_CONFIG,
            block_config: &DEFAULT_BLOCK_CONFIG,
            download_config: &DEFAULT_DOWNLOAD_CONFIG,
        };

        assert!(!desc.matches(&req, Some(DRIVER_HASH)));
    }

    #[test]
    fn descriptor_invalidated_by_driver_hash_change() {
        let desc = SessionDescriptor {
            pid: std::process::id(),
            browser: BrowserKind::Chromium,
            headless: true,
            cdp_endpoint: Some("ws://localhost:1234".into()),
            ws_endpoint: Some("ws://localhost:1234".into()),
            driver_hash: Some("old-hash".into()),
            created_at: 42,
        };

        let req = SessionRequest {
            wait_until: WaitUntil::NetworkIdle,
            headless: true,
            auth_file: None,
            browser: BrowserKind::Chromium,
            cdp_endpoint: Some("ws://localhost:1234"),
            launch_server: true,
            remote_debugging_port: None,
            keep_browser_running: false,
            protected_urls: &[],
            preferred_url: None,
            har_config: &DEFAULT_HAR_CONFIG,
            block_config: &DEFAULT_BLOCK_CONFIG,
            download_config: &DEFAULT_DOWNLOAD_CONFIG,
        };

        assert!(!desc.matches(&req, Some(DRIVER_HASH)));
    }

    #[test]
    fn test_urls_match() {
        // Exact match
        assert!(urls_match("https://example.com", "https://example.com"));

        // Trailing slash normalization
        assert!(urls_match("https://example.com/", "https://example.com"));
        assert!(urls_match("https://example.com", "https://example.com/"));
        assert!(urls_match(
            "https://example.com/path/",
            "https://example.com/path"
        ));

        // Different URLs should not match
        assert!(!urls_match("https://example.com", "https://other.com"));
        assert!(!urls_match(
            "https://example.com/a",
            "https://example.com/b"
        ));
        assert!(!urls_match("https://example.com", "http://example.com"));
    }
}
