// BrowserContext protocol object
//
// Represents an isolated browser context (session) within a browser instance.
// Multiple contexts can exist in a single browser, each with its own cookies,
// cache, and local storage.

use crate::Page;
use crate::cookie::{ClearCookiesOptions, Cookie, StorageState, StorageStateOptions};
use crate::tracing::Tracing;
use pw_runtime::Result;
use pw_runtime::channel::Channel;
use pw_runtime::channel_owner::{ChannelOwner, ChannelOwnerImpl, ParentOrConnection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Options for [`BrowserContext::route_from_har`].
#[derive(Debug, Clone, Default)]
pub struct RouteFromHarOptions {
    /// URL pattern to match for HAR routing.
    pub url: Option<String>,
    /// How to handle requests not found in HAR.
    pub not_found: Option<HarNotFound>,
    /// Whether to update the HAR file with new requests.
    pub update: Option<bool>,
}

impl RouteFromHarOptions {
    /// Creates new options with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the URL pattern to match.
    pub fn url(mut self, pattern: impl Into<String>) -> Self {
        self.url = Some(pattern.into());
        self
    }

    /// Sets what to do when a request is not found in HAR.
    pub fn not_found(mut self, action: HarNotFound) -> Self {
        self.not_found = Some(action);
        self
    }

    /// Whether to update the HAR file with missing requests.
    pub fn update(mut self, update: bool) -> Self {
        self.update = Some(update);
        self
    }
}

/// What to do when a request is not found in the HAR file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarNotFound {
    /// Abort the request.
    Abort,
    /// Fall through to the network.
    Fallback,
}

impl HarNotFound {
    fn as_str(&self) -> &'static str {
        match self {
            HarNotFound::Abort => "abort",
            HarNotFound::Fallback => "fallback",
        }
    }
}

/// BrowserContext represents an isolated browser session.
///
/// Contexts are isolated environments within a browser instance. Each context
/// has its own cookies, cache, and local storage, enabling independent sessions
/// without interference.
///
/// # Example
///
/// ```ignore
/// use pw::protocol::Playwright;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let playwright = Playwright::launch().await?;
///     let browser = playwright.chromium().launch().await?;
///
///     // Create isolated contexts
///     let context1 = browser.new_context().await?;
///     let context2 = browser.new_context().await?;
///
///     // Create pages in each context
///     let page1 = context1.new_page().await?;
///     let page2 = context2.new_page().await?;
///
///     // Cleanup
///     context1.close().await?;
///     context2.close().await?;
///     browser.close().await?;
///     Ok(())
/// }
/// ```
///
/// See: <https://playwright.dev/docs/api/class-browsercontext>
#[derive(Clone)]
pub struct BrowserContext {
    base: ChannelOwnerImpl,
}

impl BrowserContext {
    /// Creates a new BrowserContext from protocol initialization
    ///
    /// This is called by the object factory when the server sends a `__create__` message
    /// for a BrowserContext object.
    ///
    /// # Arguments
    ///
    /// * `parent` - The parent Browser object
    /// * `type_name` - The protocol type name ("BrowserContext")
    /// * `guid` - The unique identifier for this context
    /// * `initializer` - The initialization data from the server
    ///
    /// # Errors
    ///
    /// Returns error if initializer is malformed
    pub fn new(
        parent: Arc<dyn ChannelOwner>,
        type_name: String,
        guid: Arc<str>,
        initializer: Value,
    ) -> Result<Self> {
        let base = ChannelOwnerImpl::new(
            ParentOrConnection::Parent(parent),
            type_name,
            guid,
            initializer,
        );

        let context = Self { base };

        // Enable dialog event subscription
        // Dialog events need to be explicitly subscribed to via updateSubscription command
        let channel = context.channel().clone();
        tokio::spawn(async move {
            let _ = channel
                .send_no_result(
                    "updateSubscription",
                    serde_json::json!({
                        "event": "dialog",
                        "enabled": true
                    }),
                )
                .await;
        });

        Ok(context)
    }

    /// Returns the channel for sending protocol messages
    ///
    /// Used internally for sending RPC calls to the context.
    fn channel(&self) -> &Channel {
        self.base.channel()
    }

    /// Returns all pages in this browser context.
    ///
    /// This returns all currently open pages (tabs) within this context.
    /// Useful for reusing existing pages instead of creating new ones.
    ///
    /// See: <https://playwright.dev/docs/api/class-browsercontext#browser-context-pages>
    pub fn pages(&self) -> Vec<Page> {
        self.base
            .children()
            .into_iter()
            .filter_map(|child| child.downcast_ref::<Page>().cloned())
            .collect()
    }

    /// Creates a new page in this browser context.
    ///
    /// Pages are isolated tabs/windows within a context. Each page starts
    /// at "about:blank" and can be navigated independently.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Context has been closed
    /// - Communication with browser process fails
    ///
    /// See: <https://playwright.dev/docs/api/class-browsercontext#browser-context-new-page>
    pub async fn new_page(&self) -> Result<Page> {
        // Response contains the GUID of the created Page
        #[derive(Deserialize)]
        struct NewPageResponse {
            page: GuidRef,
        }

        #[derive(Deserialize)]
        struct GuidRef {
            #[serde(deserialize_with = "pw_runtime::connection::deserialize_arc_str")]
            guid: Arc<str>,
        }

        // Send newPage RPC to server
        let response: NewPageResponse = self
            .channel()
            .send("newPage", serde_json::json!({}))
            .await?;

        // Retrieve the Page object from the connection registry
        let page_arc = self.connection().get_object(&response.page.guid).await?;

        // Downcast to Page
        let page = page_arc.downcast_ref::<Page>().ok_or_else(|| {
            pw_runtime::Error::ProtocolError(format!(
                "Expected Page object, got {}",
                page_arc.type_name()
            ))
        })?;

        Ok(page.clone())
    }

    /// Closes the browser context and all its pages.
    ///
    /// This is a graceful operation that sends a close command to the context
    /// and waits for it to shut down properly.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Context has already been closed
    /// - Communication with browser process fails
    ///
    /// See: <https://playwright.dev/docs/api/class-browsercontext#browser-context-close>
    pub async fn close(&self) -> Result<()> {
        // Send close RPC to server
        self.channel()
            .send_no_result("close", serde_json::json!({}))
            .await
    }

    /// Adds cookies to the browser context.
    ///
    /// Cookies can be specified with either a domain or a URL. If URL is provided,
    /// domain and path will be inferred from it.
    ///
    /// # Arguments
    ///
    /// * `cookies` - List of cookies to add
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pw_core::protocol::{Cookie, SameSite};
    ///
    /// // Add a session cookie
    /// context.add_cookies(vec![
    ///     Cookie::new("session", "abc123", ".example.com")
    ///         .path("/")
    ///         .http_only(true)
    ///         .secure(true)
    ///         .same_site(SameSite::Lax),
    /// ]).await?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Context has been closed
    /// - Cookie specification is invalid (missing both domain and url)
    /// - Communication with browser process fails
    ///
    /// See: <https://playwright.dev/docs/api/class-browsercontext#browser-context-add-cookies>
    pub async fn add_cookies(&self, cookies: Vec<Cookie>) -> Result<()> {
        self.channel()
            .send_no_result("addCookies", serde_json::json!({ "cookies": cookies }))
            .await
    }

    /// Returns all cookies in the browser context.
    ///
    /// If URLs are specified, only cookies affecting those URLs are returned.
    /// If no URLs are specified, all cookies are returned.
    ///
    /// # Arguments
    ///
    /// * `urls` - Optional list of URLs to filter cookies by
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Get all cookies
    /// let all_cookies = context.cookies(None).await?;
    ///
    /// // Get cookies for specific URLs
    /// let cookies = context.cookies(Some(vec!["https://example.com"])).await?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Context has been closed
    /// - Communication with browser process fails
    ///
    /// See: <https://playwright.dev/docs/api/class-browsercontext#browser-context-cookies>
    pub async fn cookies(&self, urls: Option<Vec<&str>>) -> Result<Vec<Cookie>> {
        #[derive(Deserialize)]
        struct CookiesResponse {
            cookies: Vec<Cookie>,
        }

        let params = match urls {
            Some(url_list) => serde_json::json!({ "urls": url_list }),
            None => serde_json::json!({ "urls": [] }),
        };

        let response: CookiesResponse = self.channel().send("cookies", params).await?;
        Ok(response.cookies)
    }

    /// Clears cookies from the browser context.
    ///
    /// If options are provided, only cookies matching all specified criteria
    /// will be cleared. If no options are provided, all cookies are cleared.
    ///
    /// # Arguments
    ///
    /// * `options` - Optional filter criteria for which cookies to clear
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pw_core::protocol::ClearCookiesOptions;
    ///
    /// // Clear all cookies
    /// context.clear_cookies(None).await?;
    ///
    /// // Clear only session cookies
    /// context.clear_cookies(Some(
    ///     ClearCookiesOptions::new().name("session")
    /// )).await?;
    ///
    /// // Clear all cookies for a domain
    /// context.clear_cookies(Some(
    ///     ClearCookiesOptions::new().domain("example.com")
    /// )).await?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Context has been closed
    /// - Communication with browser process fails
    ///
    /// See: <https://playwright.dev/docs/api/class-browsercontext#browser-context-clear-cookies>
    pub async fn clear_cookies(&self, options: Option<ClearCookiesOptions>) -> Result<()> {
        let params = match options {
            Some(opts) => serde_json::to_value(opts).unwrap_or_default(),
            None => serde_json::json!({}),
        };

        self.channel().send_no_result("clearCookies", params).await
    }

    /// Returns the storage state for the browser context.
    ///
    /// The storage state includes cookies and localStorage for all origins.
    /// This can be saved to a file and later restored using the `storage_state`
    /// option when creating a new context.
    ///
    /// # Arguments
    ///
    /// * `options` - Optional path to save the storage state to
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pw_core::protocol::StorageStateOptions;
    ///
    /// // Get storage state
    /// let state = context.storage_state(None).await?;
    ///
    /// // Save to file manually
    /// std::fs::write("auth.json", serde_json::to_string_pretty(&state)?)?;
    ///
    /// // Or save directly via options
    /// context.storage_state(Some(
    ///     StorageStateOptions::new().path("auth.json")
    /// )).await?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Context has been closed
    /// - Communication with browser process fails
    /// - File write fails (if path is specified)
    ///
    /// See: <https://playwright.dev/docs/api/class-browsercontext#browser-context-storage-state>
    pub async fn storage_state(
        &self,
        options: Option<StorageStateOptions>,
    ) -> Result<StorageState> {
        let params = match &options {
            Some(opts) => serde_json::to_value(opts).unwrap_or_default(),
            None => serde_json::json!({}),
        };

        let state: StorageState = self.channel().send("storageState", params).await?;

        // If path was specified, save to file
        if let Some(opts) = options {
            if let Some(path) = opts.path {
                state.to_file(&path)?;
            }
        }

        Ok(state)
    }

    /// Saves the storage state to the specified `path`.
    ///
    /// This is a convenience method equivalent to calling [`storage_state`] and
    /// writing to a file. The storage state includes cookies and localStorage
    /// for all origins.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Save authentication state
    /// context.save_storage_state("auth.json").await?;
    ///
    /// // Later, restore in a new context
    /// let state = StorageState::from_file("auth.json")?;
    /// let options = BrowserContextOptions::builder()
    ///     .storage_state(state)
    ///     .build();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProtocolError`] if the context has been closed, or
    /// [`Error::IoError`] if the file cannot be written.
    ///
    /// [`Error::ProtocolError`]: pw_runtime::Error::ProtocolError
    /// [`Error::IoError`]: pw_runtime::Error::IoError
    /// [`storage_state`]: Self::storage_state
    ///
    /// See: <https://playwright.dev/docs/api/class-browsercontext#browser-context-storage-state>
    pub async fn save_storage_state(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let state = self.storage_state(None).await?;
        state.to_file(path.as_ref())?;
        Ok(())
    }

    /// Enables HAR-based request playback from `har_path`.
    ///
    /// Intercepts requests matching the HAR file and returns recorded responses.
    /// This is useful for replaying network traffic in tests. Pass `options` to
    /// configure URL filtering and behavior for unmatched requests.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Record HAR first
    /// let options = BrowserContextOptions::builder()
    ///     .record_har_path("network.har")
    ///     .build();
    /// let context = browser.new_context_with_options(options).await?;
    /// // ... perform actions ...
    /// context.close().await?;
    ///
    /// // Replay later
    /// let context = browser.new_context().await?;
    /// context.route_from_har("network.har", None).await?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProtocolError`] if the HAR file is invalid, or
    /// [`Error::IoError`] if the HAR file cannot be read.
    ///
    /// [`Error::ProtocolError`]: pw_runtime::Error::ProtocolError
    /// [`Error::IoError`]: pw_runtime::Error::IoError
    ///
    /// See: <https://playwright.dev/docs/api/class-browsercontext#browser-context-route-from-har>
    pub async fn route_from_har(
        &self,
        har_path: impl AsRef<std::path::Path>,
        options: Option<RouteFromHarOptions>,
    ) -> Result<()> {
        let mut params = serde_json::json!({
            "har": har_path.as_ref().to_string_lossy()
        });

        if let Some(opts) = options {
            if let Some(url) = opts.url {
                params["url"] = serde_json::json!(url);
            }
            if let Some(not_found) = opts.not_found {
                params["notFound"] = serde_json::json!(not_found.as_str());
            }
            if let Some(update) = opts.update {
                params["update"] = serde_json::json!(update);
            }
        }

        self.channel().send_no_result("routeFromHAR", params).await
    }

    /// Returns a handle for managing Playwright traces.
    ///
    /// Tracing captures a trace of browser operations that can be viewed in the
    /// [Playwright Trace Viewer](https://playwright.dev/docs/trace-viewer).
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pw::protocol::{TracingStartOptions, TracingStopOptions};
    ///
    /// // Start tracing
    /// context.tracing().start(TracingStartOptions {
    ///     screenshots: Some(true),
    ///     snapshots: Some(true),
    ///     ..Default::default()
    /// }).await?;
    ///
    /// // Perform actions
    /// page.goto("https://example.com", None).await?;
    ///
    /// // Stop and save
    /// context.tracing().stop(TracingStopOptions::with_path("trace.zip")).await?;
    /// ```
    ///
    /// See: <https://playwright.dev/docs/api/class-browsercontext#browser-context-tracing>
    pub fn tracing(&self) -> Option<Tracing> {
        // The Tracing object is created as a child of BrowserContext
        // Its GUID is in the initializer: {"tracing": {"guid": "tracing@..."}}
        let tracing_guid = self
            .base
            .initializer()
            .get("tracing")
            .and_then(|v| v.get("guid"))
            .and_then(|v| v.as_str())?;

        self.base
            .children()
            .into_iter()
            .find(|child| child.guid() == tracing_guid)
            .and_then(|child| child.downcast_ref::<Tracing>().cloned())
    }
}

impl pw_runtime::channel_owner::private::Sealed for BrowserContext {}

impl ChannelOwner for BrowserContext {
    fn guid(&self) -> &str {
        self.base.guid()
    }

    fn type_name(&self) -> &str {
        self.base.type_name()
    }

    fn parent(&self) -> Option<Arc<dyn ChannelOwner>> {
        self.base.parent()
    }

    fn connection(&self) -> Arc<dyn pw_runtime::connection::ConnectionLike> {
        self.base.connection()
    }

    fn initializer(&self) -> &Value {
        self.base.initializer()
    }

    fn channel(&self) -> &Channel {
        self.base.channel()
    }

    fn dispose(&self, reason: pw_runtime::channel_owner::DisposeReason) {
        self.base.dispose(reason)
    }

    fn adopt(&self, child: Arc<dyn ChannelOwner>) {
        self.base.adopt(child)
    }

    fn add_child(&self, guid: Arc<str>, child: Arc<dyn ChannelOwner>) {
        self.base.add_child(guid, child)
    }

    fn remove_child(&self, guid: &str) {
        self.base.remove_child(guid)
    }

    fn on_event(&self, method: &str, params: Value) {
        match method {
            "dialog" => {
                // Dialog events come to BrowserContext, need to forward to the associated Page
                // Event format: {dialog: {guid: "..."}}
                // The Dialog protocol object has the Page as its parent
                if let Some(dialog_guid) = params
                    .get("dialog")
                    .and_then(|v| v.get("guid"))
                    .and_then(|v| v.as_str())
                {
                    let connection = self.connection();
                    let dialog_guid_owned = dialog_guid.to_string();

                    tokio::spawn(async move {
                        // Get the Dialog object
                        let dialog_arc = match connection.get_object(&dialog_guid_owned).await {
                            Ok(obj) => obj,
                            Err(_) => return,
                        };

                        // Downcast to Dialog
                        let dialog = match dialog_arc.downcast_ref::<crate::Dialog>() {
                            Some(d) => d.clone(),
                            None => return,
                        };

                        // Get the Page from the Dialog's parent
                        let page_arc = match dialog_arc.parent() {
                            Some(parent) => parent,
                            None => return,
                        };

                        // Downcast to Page
                        let page = match page_arc.downcast_ref::<Page>() {
                            Some(p) => p.clone(),
                            None => return,
                        };

                        // Forward to Page's dialog handlers
                        page.trigger_dialog_event(dialog).await;
                    });
                }
            }
            _ => {
                // Other events will be handled in future phases
            }
        }
    }

    fn was_collected(&self) -> bool {
        self.base.was_collected()
    }
}

impl std::fmt::Debug for BrowserContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserContext")
            .field("guid", &self.guid())
            .finish()
    }
}

/// Viewport dimensions for browser context.
///
/// See: <https://playwright.dev/docs/api/class-browser#browser-new-context>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Viewport {
    /// Page width in pixels
    pub width: u32,
    /// Page height in pixels
    pub height: u32,
}

/// Geolocation coordinates.
///
/// See: <https://playwright.dev/docs/api/class-browser#browser-new-context>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Geolocation {
    /// Latitude between -90 and 90
    pub latitude: f64,
    /// Longitude between -180 and 180
    pub longitude: f64,
    /// Optional accuracy in meters (default: 0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accuracy: Option<f64>,
}

/// Policy for what content to include in HAR recordings.
///
/// See: <https://playwright.dev/docs/api/class-browser#browser-new-context>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HarContentPolicy {
    /// Include content inline (base64).
    Embed,
    /// Store content in separate files.
    Attach,
    /// Omit content entirely.
    Omit,
}

/// Mode for HAR recording.
///
/// See: <https://playwright.dev/docs/api/class-browser#browser-new-context>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HarMode {
    /// Store all content.
    Full,
    /// Store only essential content to replay the HAR.
    Minimal,
}

/// Options for creating a new browser context.
///
/// Allows customizing viewport, user agent, locale, timezone, geolocation,
/// permissions, and other browser context settings.
///
/// See: <https://playwright.dev/docs/api/class-browser#browser-new-context>
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserContextOptions {
    /// Sets consistent viewport for all pages in the context.
    /// Set to null via `no_viewport(true)` to disable viewport emulation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport: Option<Viewport>,

    /// Disables viewport emulation when set to true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_viewport: Option<bool>,

    /// Custom user agent string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,

    /// Locale for the context (e.g., "en-GB", "de-DE", "fr-FR")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,

    /// Timezone identifier (e.g., "America/New_York", "Europe/Berlin")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone_id: Option<String>,

    /// Geolocation coordinates
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geolocation: Option<Geolocation>,

    /// List of permissions to grant (e.g., "geolocation", "notifications")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,

    /// Emulates 'prefers-colors-scheme' media feature ("light", "dark", "no-preference")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_scheme: Option<String>,

    /// Whether the viewport supports touch events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_touch: Option<bool>,

    /// Whether the meta viewport tag is respected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_mobile: Option<bool>,

    /// Whether JavaScript is enabled in the context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub javascript_enabled: Option<bool>,

    /// Emulates network being offline
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offline: Option<bool>,

    /// Whether to automatically download attachments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_downloads: Option<bool>,

    /// Whether to bypass Content-Security-Policy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bypass_csp: Option<bool>,

    /// Whether to ignore HTTPS errors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_https_errors: Option<bool>,

    /// Device scale factor (default: 1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_scale_factor: Option<f64>,

    /// Extra HTTP headers to send with every request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_http_headers: Option<HashMap<String, String>>,

    /// Base URL for relative navigation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    /// Storage state to initialize the context with (cookies and localStorage).
    /// Can be loaded from a file saved by `context.storage_state()`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_state: Option<StorageState>,

    /// Directory to save videos to. Enables video recording for all pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_video_dir: Option<String>,

    /// Size of recorded videos (defaults to viewport size scaled to fit 800x800).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_video_size: Option<Viewport>,

    /// Path to save HAR file. Enables HAR recording for all pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_har_path: Option<String>,

    /// HAR recording content policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_har_content: Option<HarContentPolicy>,

    /// HAR recording mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_har_mode: Option<HarMode>,

    /// Whether to omit request content from HAR.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_har_omit_content: Option<bool>,

    /// URL pattern to filter HAR entries by URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_har_url_filter: Option<String>,
}

impl BrowserContextOptions {
    /// Creates a new builder for BrowserContextOptions
    pub fn builder() -> BrowserContextOptionsBuilder {
        BrowserContextOptionsBuilder::default()
    }
}

/// Builder for BrowserContextOptions
#[derive(Debug, Clone, Default)]
pub struct BrowserContextOptionsBuilder {
    viewport: Option<Viewport>,
    no_viewport: Option<bool>,
    user_agent: Option<String>,
    locale: Option<String>,
    timezone_id: Option<String>,
    geolocation: Option<Geolocation>,
    permissions: Option<Vec<String>>,
    color_scheme: Option<String>,
    has_touch: Option<bool>,
    is_mobile: Option<bool>,
    javascript_enabled: Option<bool>,
    offline: Option<bool>,
    accept_downloads: Option<bool>,
    bypass_csp: Option<bool>,
    ignore_https_errors: Option<bool>,
    device_scale_factor: Option<f64>,
    extra_http_headers: Option<HashMap<String, String>>,
    base_url: Option<String>,
    storage_state: Option<StorageState>,
    record_video_dir: Option<String>,
    record_video_size: Option<Viewport>,
    record_har_path: Option<String>,
    record_har_content: Option<HarContentPolicy>,
    record_har_mode: Option<HarMode>,
    record_har_omit_content: Option<bool>,
    record_har_url_filter: Option<String>,
}

impl BrowserContextOptionsBuilder {
    /// Sets the viewport dimensions
    pub fn viewport(mut self, viewport: Viewport) -> Self {
        self.viewport = Some(viewport);
        self.no_viewport = None; // Clear no_viewport if setting viewport
        self
    }

    /// Disables viewport emulation
    pub fn no_viewport(mut self, no_viewport: bool) -> Self {
        self.no_viewport = Some(no_viewport);
        if no_viewport {
            self.viewport = None; // Clear viewport if setting no_viewport
        }
        self
    }

    /// Sets the user agent string
    pub fn user_agent(mut self, user_agent: String) -> Self {
        self.user_agent = Some(user_agent);
        self
    }

    /// Sets the locale
    pub fn locale(mut self, locale: String) -> Self {
        self.locale = Some(locale);
        self
    }

    /// Sets the timezone identifier
    pub fn timezone_id(mut self, timezone_id: String) -> Self {
        self.timezone_id = Some(timezone_id);
        self
    }

    /// Sets the geolocation
    pub fn geolocation(mut self, geolocation: Geolocation) -> Self {
        self.geolocation = Some(geolocation);
        self
    }

    /// Sets the permissions to grant
    pub fn permissions(mut self, permissions: Vec<String>) -> Self {
        self.permissions = Some(permissions);
        self
    }

    /// Sets the color scheme preference
    pub fn color_scheme(mut self, color_scheme: String) -> Self {
        self.color_scheme = Some(color_scheme);
        self
    }

    /// Sets whether the viewport supports touch events
    pub fn has_touch(mut self, has_touch: bool) -> Self {
        self.has_touch = Some(has_touch);
        self
    }

    /// Sets whether this is a mobile viewport
    pub fn is_mobile(mut self, is_mobile: bool) -> Self {
        self.is_mobile = Some(is_mobile);
        self
    }

    /// Sets whether JavaScript is enabled
    pub fn javascript_enabled(mut self, javascript_enabled: bool) -> Self {
        self.javascript_enabled = Some(javascript_enabled);
        self
    }

    /// Sets whether to emulate offline network
    pub fn offline(mut self, offline: bool) -> Self {
        self.offline = Some(offline);
        self
    }

    /// Sets whether to automatically download attachments
    pub fn accept_downloads(mut self, accept_downloads: bool) -> Self {
        self.accept_downloads = Some(accept_downloads);
        self
    }

    /// Sets whether to bypass Content-Security-Policy
    pub fn bypass_csp(mut self, bypass_csp: bool) -> Self {
        self.bypass_csp = Some(bypass_csp);
        self
    }

    /// Sets whether to ignore HTTPS errors
    pub fn ignore_https_errors(mut self, ignore_https_errors: bool) -> Self {
        self.ignore_https_errors = Some(ignore_https_errors);
        self
    }

    /// Sets the device scale factor
    pub fn device_scale_factor(mut self, device_scale_factor: f64) -> Self {
        self.device_scale_factor = Some(device_scale_factor);
        self
    }

    /// Sets extra HTTP headers
    pub fn extra_http_headers(mut self, extra_http_headers: HashMap<String, String>) -> Self {
        self.extra_http_headers = Some(extra_http_headers);
        self
    }

    /// Sets the base URL for relative navigation
    pub fn base_url(mut self, base_url: String) -> Self {
        self.base_url = Some(base_url);
        self
    }

    /// Sets the storage state (cookies and localStorage) to initialize the context with.
    ///
    /// Use this to restore a previously saved authentication state.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pw_core::protocol::{BrowserContextOptions, StorageState};
    ///
    /// // Load from file
    /// let state = StorageState::from_file("auth.json")?;
    /// let options = BrowserContextOptions::builder()
    ///     .storage_state(state)
    ///     .build();
    /// ```
    pub fn storage_state(mut self, storage_state: StorageState) -> Self {
        self.storage_state = Some(storage_state);
        self
    }

    /// Enables video recording for all pages in this context.
    ///
    /// Videos are saved to the specified directory.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let options = BrowserContextOptions::builder()
    ///     .record_video_dir("/tmp/videos")
    ///     .build();
    /// ```
    pub fn record_video_dir(mut self, dir: impl Into<String>) -> Self {
        self.record_video_dir = Some(dir.into());
        self
    }

    /// Sets the size of recorded videos.
    ///
    /// Defaults to viewport size scaled to fit 800x800.
    pub fn record_video_size(mut self, size: Viewport) -> Self {
        self.record_video_size = Some(size);
        self
    }

    /// Enables HAR recording and saves to the specified path.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let options = BrowserContextOptions::builder()
    ///     .record_har_path("network.har")
    ///     .record_har_content(HarContentPolicy::Embed)
    ///     .build();
    /// ```
    pub fn record_har_path(mut self, path: impl Into<String>) -> Self {
        self.record_har_path = Some(path.into());
        self
    }

    /// Sets the HAR content policy.
    pub fn record_har_content(mut self, policy: HarContentPolicy) -> Self {
        self.record_har_content = Some(policy);
        self
    }

    /// Sets the HAR recording mode.
    pub fn record_har_mode(mut self, mode: HarMode) -> Self {
        self.record_har_mode = Some(mode);
        self
    }

    /// Whether to omit request content from HAR.
    pub fn record_har_omit_content(mut self, omit: bool) -> Self {
        self.record_har_omit_content = Some(omit);
        self
    }

    /// URL pattern to filter HAR entries.
    pub fn record_har_url_filter(mut self, pattern: impl Into<String>) -> Self {
        self.record_har_url_filter = Some(pattern.into());
        self
    }

    /// Builds the BrowserContextOptions
    pub fn build(self) -> BrowserContextOptions {
        BrowserContextOptions {
            viewport: self.viewport,
            no_viewport: self.no_viewport,
            user_agent: self.user_agent,
            locale: self.locale,
            timezone_id: self.timezone_id,
            geolocation: self.geolocation,
            permissions: self.permissions,
            color_scheme: self.color_scheme,
            has_touch: self.has_touch,
            is_mobile: self.is_mobile,
            javascript_enabled: self.javascript_enabled,
            offline: self.offline,
            accept_downloads: self.accept_downloads,
            bypass_csp: self.bypass_csp,
            ignore_https_errors: self.ignore_https_errors,
            device_scale_factor: self.device_scale_factor,
            extra_http_headers: self.extra_http_headers,
            base_url: self.base_url,
            storage_state: self.storage_state,
            record_video_dir: self.record_video_dir,
            record_video_size: self.record_video_size,
            record_har_path: self.record_har_path,
            record_har_content: self.record_har_content,
            record_har_mode: self.record_har_mode,
            record_har_omit_content: self.record_har_omit_content,
            record_har_url_filter: self.record_har_url_filter,
        }
    }
}
