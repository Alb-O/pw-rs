use pw::{BrowserContextOptions, GotoOptions, Playwright, StorageState, Subscription, WaitUntil};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::debug;

use crate::context::{BlockConfig, DownloadConfig, HarConfig};
use crate::error::{PwError, Result};
use crate::types::BrowserKind;

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

/// Build BrowserContextOptions with optional HAR and download configuration
fn build_context_options(
    storage_state: Option<StorageState>,
    har_config: &HarConfig,
    download_config: &DownloadConfig,
) -> BrowserContextOptions {
    let mut builder = BrowserContextOptions::builder();

    if let Some(state) = storage_state {
        builder = builder.storage_state(state);
    }

    // Enable downloads if tracking is configured
    if download_config.is_enabled() {
        builder = builder.accept_downloads(true);
    }

    // Apply HAR configuration if enabled
    if let Some(ref path) = har_config.path {
        debug!(
            target = "pw",
            har_path = %path.display(),
            "configuring HAR recording"
        );
        builder = builder.record_har_path(path.to_string_lossy());
        if let Some(policy) = har_config.content_policy {
            builder = builder.record_har_content(policy);
        }
        if let Some(mode) = har_config.mode {
            builder = builder.record_har_mode(mode);
        }
        if har_config.omit_content {
            builder = builder.record_har_omit_content(true);
        }
        if let Some(ref filter) = har_config.url_filter {
            builder = builder.record_har_url_filter(filter);
        }
    }

    builder.build()
}

/// Active HAR recording state
struct HarRecording {
    /// HAR ID returned by har_start
    id: String,
    /// Path to save the HAR file
    path: std::path::PathBuf,
}

pub struct BrowserSession {
    _playwright: Playwright,
    browser: pw::Browser,
    context: pw::BrowserContext,
    page: pw::Page,
    wait_until: WaitUntil,
    ws_endpoint: Option<String>,
    cdp_endpoint: Option<String>,
    launched_server: Option<pw::LaunchedServer>,
    keep_server_running: bool,
    keep_browser_running: bool,
    /// Active HAR recording, if any
    har_recording: Option<HarRecording>,
    /// Route subscriptions for request blocking; held to keep handlers alive.
    #[allow(dead_code)]
    route_subscriptions: Vec<Subscription>,
    /// Download handler subscription; held to keep handler alive.
    #[allow(dead_code)]
    download_subscription: Option<Subscription>,
    /// Collected download information (shared with download handler).
    downloads: Arc<Mutex<Vec<DownloadInfo>>>,
}

impl BrowserSession {
    pub async fn new(wait_until: WaitUntil) -> Result<Self> {
        Self::with_options(
            wait_until,
            None,
            true,
            BrowserKind::default(),
            None,
            false,
            &[],
            None,
            &HarConfig::default(),
            &BlockConfig::default(),
            &DownloadConfig::default(),
        )
        .await
    }

    /// Create a session with optional auth file (convenience for commands)
    pub async fn with_auth(
        wait_until: WaitUntil,
        auth_file: Option<&Path>,
        cdp_endpoint: Option<&str>,
    ) -> Result<Self> {
        Self::with_auth_and_browser(wait_until, auth_file, BrowserKind::default(), cdp_endpoint)
            .await
    }

    /// Create a session with optional auth file and specific browser
    pub async fn with_auth_and_browser(
        wait_until: WaitUntil,
        auth_file: Option<&Path>,
        browser_kind: BrowserKind,
        cdp_endpoint: Option<&str>,
    ) -> Result<Self> {
        match auth_file {
            Some(path) => {
                Self::with_auth_file_and_browser(wait_until, path, browser_kind, cdp_endpoint).await
            }
            None => {
                Self::with_options(
                    wait_until,
                    None,
                    true,
                    browser_kind,
                    cdp_endpoint,
                    false,
                    &[],
                    None,
                    &HarConfig::default(),
                    &BlockConfig::default(),
                    &DownloadConfig::default(),
                )
                .await
            }
        }
    }

    /// Create a new session with optional storage state and headless mode
    pub async fn with_options(
        wait_until: WaitUntil,
        storage_state: Option<StorageState>,
        headless: bool,
        browser_kind: BrowserKind,
        cdp_endpoint: Option<&str>,
        launch_server: bool,
        protected_urls: &[String],
        preferred_url: Option<&str>,
        har_config: &HarConfig,
        block_config: &BlockConfig,
        download_config: &DownloadConfig,
    ) -> Result<Self> {
        debug!(
            target = "pw",
            browser = %browser_kind,
            cdp = cdp_endpoint.is_some(),
            launch_server,
            "starting Playwright..."
        );
        let mut playwright = Playwright::launch()
            .await
            .map_err(|e| PwError::BrowserLaunch(e.to_string()))?;

        let mut ws_endpoint = None;
        let mut cdp_endpoint_stored = None;
        let mut launched_server = None;
        let mut keep_server_running = false;

        // Track whether we're connecting to existing browser (for page reuse)
        let mut reuse_existing_page = false;

        let (browser, context) = if let Some(endpoint) = cdp_endpoint {
            // Store the CDP endpoint for later retrieval
            cdp_endpoint_stored = Some(endpoint.to_string());
            if browser_kind != BrowserKind::Chromium {
                return Err(PwError::BrowserLaunch(
                    "CDP endpoint connections require the chromium browser".to_string(),
                ));
            }

            let connect_result = playwright
                .chromium()
                .connect_over_cdp(endpoint)
                .await
                .map_err(|e| PwError::BrowserLaunch(e.to_string()))?;

            let browser = connect_result.browser;
            let needs_custom_context =
                storage_state.is_some() || har_config.is_enabled() || download_config.is_enabled();
            let context = if needs_custom_context {
                let options =
                    build_context_options(storage_state.clone(), har_config, download_config);
                browser.new_context_with_options(options).await?
            } else if let Some(default_ctx) = connect_result.default_context {
                // Reuse existing pages when using default context from CDP
                reuse_existing_page = true;
                default_ctx
            } else {
                browser.new_context().await?
            };

            (browser, context)
        } else if launch_server {
            playwright.keep_server_running();
            keep_server_running = true;

            let launch_options = pw::LaunchOptions {
                headless: Some(headless),
                ..Default::default()
            };

            let launched = match browser_kind {
                BrowserKind::Chromium => playwright
                    .chromium()
                    .launch_server_with_options(launch_options)
                    .await
                    .map_err(|e| PwError::BrowserLaunch(e.to_string()))?,
                BrowserKind::Firefox => playwright
                    .firefox()
                    .launch_server_with_options(launch_options)
                    .await
                    .map_err(|e| PwError::BrowserLaunch(e.to_string()))?,
                BrowserKind::Webkit => playwright
                    .webkit()
                    .launch_server_with_options(launch_options)
                    .await
                    .map_err(|e| PwError::BrowserLaunch(e.to_string()))?,
            };

            ws_endpoint = Some(launched.ws_endpoint().to_string());
            launched_server = Some(launched.clone());

            let browser = launched.browser().clone();
            let needs_custom_context =
                storage_state.is_some() || har_config.is_enabled() || download_config.is_enabled();
            let context = if needs_custom_context {
                let options =
                    build_context_options(storage_state.clone(), har_config, download_config);
                browser.new_context_with_options(options).await?
            } else {
                browser.new_context().await?
            };

            (browser, context)
        } else {
            let launch_options = pw::LaunchOptions {
                headless: Some(headless),
                ..Default::default()
            };

            // Select browser type based on browser_kind
            let browser = match browser_kind {
                BrowserKind::Chromium => {
                    playwright
                        .chromium()
                        .launch_with_options(launch_options)
                        .await?
                }
                BrowserKind::Firefox => {
                    playwright
                        .firefox()
                        .launch_with_options(launch_options)
                        .await?
                }
                BrowserKind::Webkit => {
                    playwright
                        .webkit()
                        .launch_with_options(launch_options)
                        .await?
                }
            };

            // Create context with optional storage state, HAR, or download config
            let needs_custom_context =
                storage_state.is_some() || har_config.is_enabled() || download_config.is_enabled();
            let context = if needs_custom_context {
                let options = build_context_options(storage_state, har_config, download_config);
                browser.new_context_with_options(options).await?
            } else {
                browser.new_context().await?
            };

            (browser, context)
        };

        // Reuse existing page if connecting to existing browser, otherwise create new
        let page = if reuse_existing_page {
            let existing_pages = context.pages();
            // Use page.url() (cached) instead of evaluate_value to avoid JS execution on each page
            // First, try to find a page matching preferred_url
            let mut preferred_page = None;
            let mut fallback_page = None;

            for page in existing_pages {
                let url = page.url();
                let is_protected = protected_urls
                    .iter()
                    .any(|pattern| url.to_lowercase().contains(&pattern.to_lowercase()));

                if is_protected {
                    debug!(target = "pw", url = %url, "skipping protected page");
                    continue;
                }

                // Check if this page matches the preferred URL
                if let Some(pref) = preferred_url {
                    if url.starts_with(pref)
                        || pref.starts_with(&url)
                        || urls_match_loosely(&url, pref)
                    {
                        debug!(target = "pw", url = %url, preferred = %pref, "found preferred page");
                        preferred_page = Some(page);
                        break;
                    }
                }

                // Keep first non-protected page as fallback
                if fallback_page.is_none() {
                    fallback_page = Some(page);
                }
            }

            match preferred_page.or(fallback_page) {
                Some(page) => {
                    debug!(target = "pw", url = %page.url(), "reusing existing page");
                    page
                }
                None => {
                    debug!(target = "pw", "no suitable pages found, creating new");
                    context.new_page().await?
                }
            }
        } else {
            context.new_page().await?
        };

        // Start HAR recording if configured
        let har_recording = if let Some(ref path) = har_config.path {
            debug!(
                target = "pw",
                har_path = %path.display(),
                "starting HAR recording"
            );
            let options = pw::HarStartOptions {
                content: har_config.content_policy,
                mode: har_config.mode,
                url_glob: har_config.url_filter.clone(),
            };
            let har_id = context.har_start(options).await.map_err(|e| {
                PwError::BrowserLaunch(format!("Failed to start HAR recording: {}", e))
            })?;
            Some(HarRecording {
                id: har_id,
                path: path.clone(),
            })
        } else {
            None
        };

        // Set up request blocking routes
        let mut route_subscriptions = Vec::with_capacity(block_config.patterns.len());
        for pattern in &block_config.patterns {
            debug!(target = "pw", %pattern, "blocking pattern");
            let sub = page
                .route(pattern, |route| async move { route.abort(None).await })
                .await
                .map_err(|e| PwError::BrowserLaunch(format!("route setup failed: {e}")))?;
            route_subscriptions.push(sub);
        }

        // Set up download handler if tracking is enabled
        let downloads: Arc<Mutex<Vec<DownloadInfo>>> = Arc::new(Mutex::new(Vec::new()));
        let download_subscription = if let Some(ref dir) = download_config.dir {
            let downloads_dir = dir.clone();
            let downloads_ref = Arc::clone(&downloads);
            debug!(target = "pw", dir = %downloads_dir.display(), "download tracking enabled");

            // Ensure downloads directory exists
            if let Err(e) = std::fs::create_dir_all(&downloads_dir) {
                return Err(PwError::BrowserLaunch(format!(
                    "failed to create downloads dir: {e}"
                )));
            }

            let sub = page.on_download(move |download| {
                let downloads_dir = downloads_dir.clone();
                let downloads_ref = Arc::clone(&downloads_ref);
                async move {
                    let url = download.url().to_string();
                    let suggested = download.suggested_filename().to_string();
                    let save_path = downloads_dir.join(&suggested);

                    debug!(
                        target = "pw",
                        url = %url,
                        filename = %suggested,
                        path = %save_path.display(),
                        "saving download"
                    );

                    download.save_as(&save_path).await?;

                    let info = DownloadInfo {
                        url,
                        suggested_filename: suggested,
                        path: save_path,
                    };
                    downloads_ref.lock().unwrap().push(info);
                    Ok(())
                }
            });
            Some(sub)
        } else {
            None
        };

        Ok(Self {
            _playwright: playwright,
            browser,
            context,
            page,
            wait_until,
            ws_endpoint,
            cdp_endpoint: cdp_endpoint_stored,
            launched_server,
            keep_server_running,
            keep_browser_running: false,
            har_recording,
            route_subscriptions,
            download_subscription,
            downloads,
        })
    }

    /// Create a session with auth loaded from a file
    pub async fn with_auth_file(wait_until: WaitUntil, auth_file: &Path) -> Result<Self> {
        Self::with_auth_file_and_browser(wait_until, auth_file, BrowserKind::default(), None).await
    }

    /// Create a session with auth loaded from a file and specific browser
    pub async fn with_auth_file_and_browser(
        wait_until: WaitUntil,
        auth_file: &Path,
        browser_kind: BrowserKind,
        cdp_endpoint: Option<&str>,
    ) -> Result<Self> {
        let storage_state = StorageState::from_file(auth_file)
            .map_err(|e| PwError::BrowserLaunch(format!("Failed to load auth file: {}", e)))?;
        Self::with_options(
            wait_until,
            Some(storage_state),
            true,
            browser_kind,
            cdp_endpoint,
            false,
            &[],
            None,
            &HarConfig::default(),
            &BlockConfig::default(),
            &DownloadConfig::default(),
        )
        .await
    }

    pub async fn launch_server_session(
        wait_until: WaitUntil,
        storage_state: Option<StorageState>,
        headless: bool,
        browser_kind: BrowserKind,
    ) -> Result<Self> {
        Self::with_options(
            wait_until,
            storage_state,
            headless,
            browser_kind,
            None,
            true,
            &[],
            None,
            &HarConfig::default(),
            &BlockConfig::default(),
            &DownloadConfig::default(),
        )
        .await
    }

    /// Launch a persistent browser session with CDP debugging port.
    ///
    /// This enables session reuse by exposing Chrome's remote debugging port.
    /// The browser will stay alive after close() if keep_browser_running is true.
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

        let mut playwright = Playwright::launch()
            .await
            .map_err(|e| PwError::BrowserLaunch(e.to_string()))?;

        // For persistent sessions, prevent the driver from killing the browser on exit
        if keep_browser_running {
            playwright.keep_server_running();
        }

        let launch_options = pw::LaunchOptions {
            headless: Some(headless),
            remote_debugging_port: Some(remote_debugging_port),
            // Prevent browser from closing on signals (for persistent sessions)
            handle_sighup: Some(!keep_browser_running),
            handle_sigint: Some(!keep_browser_running),
            handle_sigterm: Some(!keep_browser_running),
            ..Default::default()
        };

        let browser = playwright
            .chromium()
            .launch_with_options(launch_options)
            .await?;

        let context = if let Some(state) = storage_state {
            let options = BrowserContextOptions::builder()
                .storage_state(state)
                .build();
            browser.new_context_with_options(options).await?
        } else {
            browser.new_context().await?
        };

        let page = context.new_page().await?;
        let cdp_endpoint = format!("http://localhost:{}", remote_debugging_port);

        Ok(Self {
            _playwright: playwright,
            browser,
            context,
            page,
            wait_until,
            ws_endpoint: None,
            cdp_endpoint: Some(cdp_endpoint),
            launched_server: None,
            keep_server_running: keep_browser_running,
            keep_browser_running,
            har_recording: None,
            route_subscriptions: Vec::new(),
            download_subscription: None,
            downloads: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Navigate to a URL with optional timeout.
    ///
    /// If `timeout_ms` is `None`, uses Playwright's default timeout (30s).
    pub async fn goto(&self, url: &str, timeout_ms: Option<u64>) -> Result<()> {
        let mut goto_opts = GotoOptions {
            wait_until: Some(self.wait_until),
            ..Default::default()
        };

        if let Some(ms) = timeout_ms {
            goto_opts.timeout = Some(std::time::Duration::from_millis(ms));
        }

        self.page
            .goto(url, Some(goto_opts))
            .await
            .map(|_| ())
            .map_err(|e| PwError::Navigation {
                url: url.to_string(),
                source: anyhow::Error::new(e),
            })
    }

    pub fn page(&self) -> &pw::Page {
        &self.page
    }

    pub fn context(&self) -> &pw::BrowserContext {
        &self.context
    }

    pub fn ws_endpoint(&self) -> Option<&str> {
        self.ws_endpoint.as_deref()
    }

    pub fn cdp_endpoint(&self) -> Option<&str> {
        self.cdp_endpoint.as_deref()
    }

    pub fn browser(&self) -> &pw::Browser {
        &self.browser
    }

    /// Returns downloads collected during this session.
    pub fn downloads(&self) -> Vec<DownloadInfo> {
        self.downloads.lock().unwrap().clone()
    }

    /// Set whether to keep the browser running after close()
    pub fn set_keep_browser_running(&mut self, keep: bool) {
        self.keep_browser_running = keep;
    }

    /// Inject cookies from auth files into the browser context.
    /// Used when connecting to real browser via CDP to add saved auth state.
    pub async fn inject_auth_files(&self, auth_files: &[std::path::PathBuf]) -> Result<()> {
        for path in auth_files {
            match StorageState::from_file(path) {
                Ok(state) => {
                    if !state.cookies.is_empty() {
                        debug!(
                            target = "pw",
                            path = %path.display(),
                            count = state.cookies.len(),
                            "injecting cookies from auth file"
                        );
                        self.context.add_cookies(state.cookies).await?;
                    }
                }
                Err(e) => {
                    debug!(
                        target = "pw",
                        path = %path.display(),
                        error = %e,
                        "failed to load auth file, skipping"
                    );
                }
            }
        }
        Ok(())
    }

    pub async fn close(self) -> Result<()> {
        // Export HAR recording if active
        if let Some(har) = &self.har_recording {
            debug!(
                target = "pw",
                har_path = %har.path.display(),
                "exporting HAR recording"
            );
            if let Err(e) = self.context.har_export(&har.id, &har.path).await {
                debug!(
                    target = "pw",
                    error = %e,
                    "failed to export HAR recording"
                );
            }
        }

        // Close the context
        let _ = self.context.close().await;

        if self.keep_browser_running || self.launched_server.is_some() {
            // Keep the browser running for reuse
            return Ok(());
        }

        self.browser.close().await?;
        Ok(())
    }

    pub async fn shutdown_server(mut self) -> Result<()> {
        if let Some(server) = self.launched_server.take() {
            server.close().await?;
            self.keep_server_running = false;
            self._playwright.enable_server_shutdown();
        } else {
            self.browser.close().await?;
        }

        Ok(())
    }
}

/// Check if two URLs match loosely (same origin/host).
fn urls_match_loosely(a: &str, b: &str) -> bool {
    // Extract host from URLs
    fn get_host(url: &str) -> Option<&str> {
        let url = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))?;
        url.split('/').next()
    }

    match (get_host(a), get_host(b)) {
        (Some(ha), Some(hb)) => ha == hb,
        _ => false,
    }
}
