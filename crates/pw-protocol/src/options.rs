//! Option structs for Playwright API methods.
//!
//! These types represent the configuration options passed to various
//! Playwright methods. They are designed for serialization to JSON-RPC.

use crate::StorageState;
use crate::types::{
    Geolocation, HarContentPolicy, HarMode, HarNotFound, KeyboardModifier, MouseButton, Position,
    ScreenshotClip, ScreenshotType, Viewport, WaitUntil,
};
use serde::{Deserialize, Serialize};

/// Default timeout in milliseconds for Playwright operations.
///
/// This matches Playwright's standard default across all language implementations.
pub const DEFAULT_TIMEOUT_MS: f64 = 30000.0;

/// Navigation options for goto().
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GotoOptions {
    /// Maximum navigation time in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,

    /// When to consider navigation succeeded
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_until: Option<WaitUntil>,

    /// Referer header value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referer: Option<String>,
}

impl GotoOptions {
    /// Creates new default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the timeout.
    pub fn timeout(mut self, timeout: f64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets the wait_until condition.
    pub fn wait_until(mut self, wait_until: WaitUntil) -> Self {
        self.wait_until = Some(wait_until);
        self
    }

    /// Sets the referer header.
    pub fn referer(mut self, referer: impl Into<String>) -> Self {
        self.referer = Some(referer.into());
        self
    }
}

/// Click options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClickOptions {
    /// Mouse button to click
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button: Option<MouseButton>,

    /// Number of clicks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub click_count: Option<u32>,

    /// Time between mousedown and mouseup in ms
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<f64>,

    /// Bypass actionability checks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,

    /// Modifier keys to press
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifiers: Option<Vec<KeyboardModifier>>,

    /// Don't wait for navigation after click
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_wait_after: Option<bool>,

    /// Position relative to element
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,

    /// Maximum time in ms
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,

    /// Perform checks without clicking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trial: Option<bool>,
}

impl ClickOptions {
    /// Creates a new builder.
    pub fn builder() -> ClickOptionsBuilder {
        ClickOptionsBuilder::default()
    }
}

/// Builder for ClickOptions.
#[derive(Debug, Clone, Default)]
pub struct ClickOptionsBuilder {
    inner: ClickOptions,
}

impl ClickOptionsBuilder {
    /// Sets the mouse button.
    pub fn button(mut self, button: MouseButton) -> Self {
        self.inner.button = Some(button);
        self
    }

    /// Sets the click count.
    pub fn click_count(mut self, count: u32) -> Self {
        self.inner.click_count = Some(count);
        self
    }

    /// Sets the delay.
    pub fn delay(mut self, delay: f64) -> Self {
        self.inner.delay = Some(delay);
        self
    }

    /// Sets force mode.
    pub fn force(mut self, force: bool) -> Self {
        self.inner.force = Some(force);
        self
    }

    /// Sets modifier keys.
    pub fn modifiers(mut self, modifiers: Vec<KeyboardModifier>) -> Self {
        self.inner.modifiers = Some(modifiers);
        self
    }

    /// Sets no_wait_after.
    pub fn no_wait_after(mut self, no_wait_after: bool) -> Self {
        self.inner.no_wait_after = Some(no_wait_after);
        self
    }

    /// Sets the position.
    pub fn position(mut self, position: Position) -> Self {
        self.inner.position = Some(position);
        self
    }

    /// Sets the timeout.
    pub fn timeout(mut self, timeout: f64) -> Self {
        self.inner.timeout = Some(timeout);
        self
    }

    /// Sets trial mode.
    pub fn trial(mut self, trial: bool) -> Self {
        self.inner.trial = Some(trial);
        self
    }

    /// Builds the options.
    pub fn build(self) -> ClickOptions {
        self.inner
    }
}

/// Fill options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FillOptions {
    /// Bypass actionability checks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,

    /// Maximum time in ms
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,
}

impl FillOptions {
    /// Creates a new builder.
    pub fn builder() -> FillOptionsBuilder {
        FillOptionsBuilder::default()
    }
}

/// Builder for FillOptions.
#[derive(Debug, Clone, Default)]
pub struct FillOptionsBuilder {
    inner: FillOptions,
}

impl FillOptionsBuilder {
    /// Sets force mode.
    pub fn force(mut self, force: bool) -> Self {
        self.inner.force = Some(force);
        self
    }

    /// Sets the timeout.
    pub fn timeout(mut self, timeout: f64) -> Self {
        self.inner.timeout = Some(timeout);
        self
    }

    /// Builds the options.
    pub fn build(self) -> FillOptions {
        self.inner
    }
}

/// Press options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PressOptions {
    /// Time between keydown and keyup in ms
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<f64>,

    /// Maximum time in ms
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,
}

impl PressOptions {
    /// Creates a new builder.
    pub fn builder() -> PressOptionsBuilder {
        PressOptionsBuilder::default()
    }
}

/// Builder for PressOptions.
#[derive(Debug, Clone, Default)]
pub struct PressOptionsBuilder {
    inner: PressOptions,
}

impl PressOptionsBuilder {
    /// Sets the delay.
    pub fn delay(mut self, delay: f64) -> Self {
        self.inner.delay = Some(delay);
        self
    }

    /// Sets the timeout.
    pub fn timeout(mut self, timeout: f64) -> Self {
        self.inner.timeout = Some(timeout);
        self
    }

    /// Builds the options.
    pub fn build(self) -> PressOptions {
        self.inner
    }
}

/// Check options (for checkbox/radio).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckOptions {
    /// Bypass actionability checks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,

    /// Position relative to element
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,

    /// Maximum time in ms
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,

    /// Perform checks without clicking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trial: Option<bool>,
}

impl CheckOptions {
    /// Creates a new builder.
    pub fn builder() -> CheckOptionsBuilder {
        CheckOptionsBuilder::default()
    }
}

/// Builder for CheckOptions.
#[derive(Debug, Clone, Default)]
pub struct CheckOptionsBuilder {
    inner: CheckOptions,
}

impl CheckOptionsBuilder {
    /// Sets force mode.
    pub fn force(mut self, force: bool) -> Self {
        self.inner.force = Some(force);
        self
    }

    /// Sets the position.
    pub fn position(mut self, position: Position) -> Self {
        self.inner.position = Some(position);
        self
    }

    /// Sets the timeout.
    pub fn timeout(mut self, timeout: f64) -> Self {
        self.inner.timeout = Some(timeout);
        self
    }

    /// Sets trial mode.
    pub fn trial(mut self, trial: bool) -> Self {
        self.inner.trial = Some(trial);
        self
    }

    /// Builds the options.
    pub fn build(self) -> CheckOptions {
        self.inner
    }
}

/// Hover options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoverOptions {
    /// Bypass actionability checks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,

    /// Modifier keys to press
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifiers: Option<Vec<KeyboardModifier>>,

    /// Position relative to element
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,

    /// Maximum time in ms
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,

    /// Perform checks without hovering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trial: Option<bool>,
}

impl HoverOptions {
    /// Creates a new builder.
    pub fn builder() -> HoverOptionsBuilder {
        HoverOptionsBuilder::default()
    }
}

/// Builder for HoverOptions.
#[derive(Debug, Clone, Default)]
pub struct HoverOptionsBuilder {
    inner: HoverOptions,
}

impl HoverOptionsBuilder {
    /// Sets force mode.
    pub fn force(mut self, force: bool) -> Self {
        self.inner.force = Some(force);
        self
    }

    /// Sets modifier keys.
    pub fn modifiers(mut self, modifiers: Vec<KeyboardModifier>) -> Self {
        self.inner.modifiers = Some(modifiers);
        self
    }

    /// Sets the position.
    pub fn position(mut self, position: Position) -> Self {
        self.inner.position = Some(position);
        self
    }

    /// Sets the timeout.
    pub fn timeout(mut self, timeout: f64) -> Self {
        self.inner.timeout = Some(timeout);
        self
    }

    /// Sets trial mode.
    pub fn trial(mut self, trial: bool) -> Self {
        self.inner.trial = Some(trial);
        self
    }

    /// Builds the options.
    pub fn build(self) -> HoverOptions {
        self.inner
    }
}

/// Select options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectOptions {
    /// Bypass actionability checks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,

    /// Maximum time in ms
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,
}

impl SelectOptions {
    /// Creates a new builder.
    pub fn builder() -> SelectOptionsBuilder {
        SelectOptionsBuilder::default()
    }
}

/// Builder for SelectOptions.
#[derive(Debug, Clone, Default)]
pub struct SelectOptionsBuilder {
    inner: SelectOptions,
}

impl SelectOptionsBuilder {
    /// Sets force mode.
    pub fn force(mut self, force: bool) -> Self {
        self.inner.force = Some(force);
        self
    }

    /// Sets the timeout.
    pub fn timeout(mut self, timeout: f64) -> Self {
        self.inner.timeout = Some(timeout);
        self
    }

    /// Builds the options.
    pub fn build(self) -> SelectOptions {
        self.inner
    }
}

/// Screenshot options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotOptions {
    /// Image format
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub screenshot_type: Option<ScreenshotType>,

    /// JPEG quality (0-100)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<u8>,

    /// Capture full scrollable page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_page: Option<bool>,

    /// Clip region to capture
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip: Option<ScreenshotClip>,

    /// Hide default white background
    #[serde(skip_serializing_if = "Option::is_none")]
    pub omit_background: Option<bool>,

    /// Screenshot timeout in ms
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,
}

impl ScreenshotOptions {
    /// Creates a new builder.
    pub fn builder() -> ScreenshotOptionsBuilder {
        ScreenshotOptionsBuilder::default()
    }
}

/// Builder for ScreenshotOptions.
#[derive(Debug, Clone, Default)]
pub struct ScreenshotOptionsBuilder {
    inner: ScreenshotOptions,
}

impl ScreenshotOptionsBuilder {
    /// Sets the screenshot type.
    pub fn screenshot_type(mut self, t: ScreenshotType) -> Self {
        self.inner.screenshot_type = Some(t);
        self
    }

    /// Sets the quality.
    pub fn quality(mut self, quality: u8) -> Self {
        self.inner.quality = Some(quality);
        self
    }

    /// Sets full page mode.
    pub fn full_page(mut self, full_page: bool) -> Self {
        self.inner.full_page = Some(full_page);
        self
    }

    /// Sets the clip region.
    pub fn clip(mut self, clip: ScreenshotClip) -> Self {
        self.inner.clip = Some(clip);
        self
    }

    /// Sets omit_background.
    pub fn omit_background(mut self, omit: bool) -> Self {
        self.inner.omit_background = Some(omit);
        self
    }

    /// Sets the timeout.
    pub fn timeout(mut self, timeout: f64) -> Self {
        self.inner.timeout = Some(timeout);
        self
    }

    /// Builds the options.
    pub fn build(self) -> ScreenshotOptions {
        self.inner
    }
}

/// Browser context options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserContextOptions {
    /// User agent string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,

    /// Viewport size
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport: Option<Viewport>,

    /// Device scale factor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_scale_factor: Option<f64>,

    /// Whether the viewport is mobile
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_mobile: Option<bool>,

    /// Whether the viewport supports touch
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_touch: Option<bool>,

    /// Locale
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,

    /// Timezone ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone_id: Option<String>,

    /// Geolocation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geolocation: Option<Geolocation>,

    /// Granted permissions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,

    /// Color scheme
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_scheme: Option<String>,

    /// Reduced motion
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reduced_motion: Option<String>,

    /// Forced colors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forced_colors: Option<String>,

    /// Accept downloads
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_downloads: Option<bool>,

    /// Extra HTTP headers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_http_headers: Option<std::collections::HashMap<String, String>>,

    /// Offline mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offline: Option<bool>,

    /// HTTP credentials
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_credentials: Option<HttpCredentials>,

    /// Bypass CSP
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bypass_csp: Option<bool>,

    /// Base URL for relative URLs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    /// Storage state to restore
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_state: Option<StorageState>,

    /// Record video directory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_video_dir: Option<String>,

    /// Record video size
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_video_size: Option<Viewport>,

    /// Record HAR path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_har_path: Option<String>,

    /// Record HAR content policy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_har_content: Option<HarContentPolicy>,

    /// Record HAR mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_har_mode: Option<HarMode>,

    /// Omit content from HAR
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_har_omit_content: Option<bool>,

    /// HAR URL filter pattern
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_har_url_filter: Option<String>,
}

impl BrowserContextOptions {
    /// Creates a new builder.
    pub fn builder() -> BrowserContextOptionsBuilder {
        BrowserContextOptionsBuilder::default()
    }
}

/// Builder for BrowserContextOptions.
#[derive(Debug, Clone, Default)]
pub struct BrowserContextOptionsBuilder {
    inner: BrowserContextOptions,
}

impl BrowserContextOptionsBuilder {
    /// Sets the user agent.
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.inner.user_agent = Some(user_agent.into());
        self
    }

    /// Sets the viewport.
    pub fn viewport(mut self, viewport: Viewport) -> Self {
        self.inner.viewport = Some(viewport);
        self
    }

    /// Sets the device scale factor.
    pub fn device_scale_factor(mut self, factor: f64) -> Self {
        self.inner.device_scale_factor = Some(factor);
        self
    }

    /// Sets mobile mode.
    pub fn is_mobile(mut self, is_mobile: bool) -> Self {
        self.inner.is_mobile = Some(is_mobile);
        self
    }

    /// Sets touch support.
    pub fn has_touch(mut self, has_touch: bool) -> Self {
        self.inner.has_touch = Some(has_touch);
        self
    }

    /// Sets the locale.
    pub fn locale(mut self, locale: impl Into<String>) -> Self {
        self.inner.locale = Some(locale.into());
        self
    }

    /// Sets the timezone.
    pub fn timezone_id(mut self, timezone: impl Into<String>) -> Self {
        self.inner.timezone_id = Some(timezone.into());
        self
    }

    /// Sets the geolocation.
    pub fn geolocation(mut self, geo: Geolocation) -> Self {
        self.inner.geolocation = Some(geo);
        self
    }

    /// Sets the permissions.
    pub fn permissions(mut self, permissions: Vec<String>) -> Self {
        self.inner.permissions = Some(permissions);
        self
    }

    /// Sets the color scheme.
    pub fn color_scheme(mut self, scheme: impl Into<String>) -> Self {
        self.inner.color_scheme = Some(scheme.into());
        self
    }

    /// Sets reduced motion.
    pub fn reduced_motion(mut self, motion: impl Into<String>) -> Self {
        self.inner.reduced_motion = Some(motion.into());
        self
    }

    /// Sets accept downloads.
    pub fn accept_downloads(mut self, accept: bool) -> Self {
        self.inner.accept_downloads = Some(accept);
        self
    }

    /// Sets extra HTTP headers.
    pub fn extra_http_headers(
        mut self,
        headers: std::collections::HashMap<String, String>,
    ) -> Self {
        self.inner.extra_http_headers = Some(headers);
        self
    }

    /// Sets offline mode.
    pub fn offline(mut self, offline: bool) -> Self {
        self.inner.offline = Some(offline);
        self
    }

    /// Sets HTTP credentials.
    pub fn http_credentials(mut self, creds: HttpCredentials) -> Self {
        self.inner.http_credentials = Some(creds);
        self
    }

    /// Sets bypass CSP.
    pub fn bypass_csp(mut self, bypass: bool) -> Self {
        self.inner.bypass_csp = Some(bypass);
        self
    }

    /// Sets the base URL.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.inner.base_url = Some(url.into());
        self
    }

    /// Sets the storage state.
    pub fn storage_state(mut self, state: StorageState) -> Self {
        self.inner.storage_state = Some(state);
        self
    }

    /// Sets the video recording directory.
    pub fn record_video_dir(mut self, dir: impl Into<String>) -> Self {
        self.inner.record_video_dir = Some(dir.into());
        self
    }

    /// Sets the video recording size.
    pub fn record_video_size(mut self, size: Viewport) -> Self {
        self.inner.record_video_size = Some(size);
        self
    }

    /// Builds the options.
    pub fn build(self) -> BrowserContextOptions {
        self.inner
    }
}

/// HTTP credentials for authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpCredentials {
    /// Username
    pub username: String,
    /// Password
    pub password: String,
    /// Origin to send credentials to (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

impl HttpCredentials {
    /// Creates new HTTP credentials.
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
            origin: None,
        }
    }

    /// Sets the origin.
    pub fn origin(mut self, origin: impl Into<String>) -> Self {
        self.origin = Some(origin.into());
        self
    }
}

/// Route fulfill options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FulfillOptions {
    /// Response status code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,

    /// Response headers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::HashMap<String, String>>,

    /// Response body as string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// Response body as base64
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_bytes: Option<String>,

    /// Content type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,

    /// Path to file to serve
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl FulfillOptions {
    /// Creates a new builder.
    pub fn builder() -> FulfillOptionsBuilder {
        FulfillOptionsBuilder::default()
    }
}

/// Builder for FulfillOptions.
#[derive(Debug, Clone, Default)]
pub struct FulfillOptionsBuilder {
    inner: FulfillOptions,
}

impl FulfillOptionsBuilder {
    /// Sets the status code.
    pub fn status(mut self, status: u16) -> Self {
        self.inner.status = Some(status);
        self
    }

    /// Sets the headers.
    pub fn headers(mut self, headers: std::collections::HashMap<String, String>) -> Self {
        self.inner.headers = Some(headers);
        self
    }

    /// Sets the body.
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.inner.body = Some(body.into());
        self
    }

    /// Sets the content type.
    pub fn content_type(mut self, content_type: impl Into<String>) -> Self {
        self.inner.content_type = Some(content_type.into());
        self
    }

    /// Builds the options.
    pub fn build(self) -> FulfillOptions {
        self.inner
    }
}

/// Route continue options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueOptions {
    /// URL to use instead
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// HTTP method to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,

    /// Headers to override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::HashMap<String, String>>,

    /// Post data to override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_data: Option<String>,
}

impl ContinueOptions {
    /// Creates a new builder.
    pub fn builder() -> ContinueOptionsBuilder {
        ContinueOptionsBuilder::default()
    }
}

/// Builder for ContinueOptions.
#[derive(Debug, Clone, Default)]
pub struct ContinueOptionsBuilder {
    inner: ContinueOptions,
}

impl ContinueOptionsBuilder {
    /// Sets the URL.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.inner.url = Some(url.into());
        self
    }

    /// Sets the method.
    pub fn method(mut self, method: impl Into<String>) -> Self {
        self.inner.method = Some(method.into());
        self
    }

    /// Sets the headers.
    pub fn headers(mut self, headers: std::collections::HashMap<String, String>) -> Self {
        self.inner.headers = Some(headers);
        self
    }

    /// Sets the post data.
    pub fn post_data(mut self, data: impl Into<String>) -> Self {
        self.inner.post_data = Some(data.into());
        self
    }

    /// Builds the options.
    pub fn build(self) -> ContinueOptions {
        self.inner
    }
}

/// Route from HAR options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteFromHarOptions {
    /// How to handle requests not in HAR
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_found: Option<HarNotFound>,

    /// Whether to update the HAR file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update: Option<bool>,

    /// HAR content policy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_content: Option<HarContentPolicy>,

    /// HAR mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_mode: Option<HarMode>,

    /// URL pattern filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Tracing start options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracingStartOptions {
    /// Whether to capture screenshots
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshots: Option<bool>,

    /// Whether to capture snapshots
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshots: Option<bool>,

    /// Whether to include sources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<bool>,

    /// Name of the trace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Title of the trace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

impl TracingStartOptions {
    /// Creates a new builder.
    pub fn builder() -> TracingStartOptionsBuilder {
        TracingStartOptionsBuilder::default()
    }
}

/// Builder for TracingStartOptions.
#[derive(Debug, Clone, Default)]
pub struct TracingStartOptionsBuilder {
    inner: TracingStartOptions,
}

impl TracingStartOptionsBuilder {
    /// Enable screenshots.
    pub fn screenshots(mut self, screenshots: bool) -> Self {
        self.inner.screenshots = Some(screenshots);
        self
    }

    /// Enable snapshots.
    pub fn snapshots(mut self, snapshots: bool) -> Self {
        self.inner.snapshots = Some(snapshots);
        self
    }

    /// Enable sources.
    pub fn sources(mut self, sources: bool) -> Self {
        self.inner.sources = Some(sources);
        self
    }

    /// Set the name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.inner.name = Some(name.into());
        self
    }

    /// Set the title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.inner.title = Some(title.into());
        self
    }

    /// Builds the options.
    pub fn build(self) -> TracingStartOptions {
        self.inner
    }
}

/// Tracing stop options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracingStopOptions {
    /// Path to save the trace to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Tracing start chunk options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracingStartChunkOptions {
    /// Name of the chunk
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Title of the chunk
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Accessibility snapshot options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessibilitySnapshotOptions {
    /// Whether to include interesting nodes only
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interesting_only: Option<bool>,

    /// Root element to snapshot
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
}

impl AccessibilitySnapshotOptions {
    /// Creates a new builder.
    pub fn builder() -> AccessibilitySnapshotOptionsBuilder {
        AccessibilitySnapshotOptionsBuilder::default()
    }
}

/// Builder for AccessibilitySnapshotOptions.
#[derive(Debug, Clone, Default)]
pub struct AccessibilitySnapshotOptionsBuilder {
    inner: AccessibilitySnapshotOptions,
}

impl AccessibilitySnapshotOptionsBuilder {
    /// Set interesting_only.
    pub fn interesting_only(mut self, interesting_only: bool) -> Self {
        self.inner.interesting_only = Some(interesting_only);
        self
    }

    /// Builds the options.
    pub fn build(self) -> AccessibilitySnapshotOptions {
        self.inner
    }
}
