// Page protocol object
//
// Represents a web page within a browser context.
// Pages are isolated tabs or windows within a context.

use crate::{Dialog, Download, Route};
use base64::Engine;
use parking_lot::Mutex;
use pw_runtime::channel::Channel;
use pw_runtime::channel_owner::{ChannelOwner, ChannelOwnerImpl, ParentOrConnection};
use pw_runtime::{Error, Result};
use serde::Deserialize;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;

/// Page represents a web page within a browser context.
///
/// A Page is created when you call `BrowserContext::new_page()` or `Browser::new_page()`.
/// Each page is an isolated tab/window within its parent context.
///
/// Initially, pages are navigated to "about:blank". Use navigation methods
/// (implemented in Phase 3) to navigate to URLs.
///
/// # Example
///
/// ```ignore
/// use pw::protocol::{Playwright, ScreenshotOptions, ScreenshotType};
/// use std::path::PathBuf;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let playwright = Playwright::launch().await?;
///     let browser = playwright.chromium().launch().await?;
///     let page = browser.new_page().await?;
///
///     // Demonstrate url() - initially at about:blank
///     assert_eq!(page.url(), "about:blank");
///
///     // Demonstrate goto() - navigate to a page
///     let html = r#"
///         <html>
///             <head><title>Test Page</title></head>
///             <body>
///                 <h1 id="heading">Hello World</h1>
///                 <p>First paragraph</p>
///                 <p>Second paragraph</p>
///                 <button onclick="alert('Alert!')">Alert</button>
///                 <a href="data:text/plain,file" download="test.txt">Download</a>
///             </body>
///         </html>
///     "#;
///     // Data URLs may not return a response (this is normal)
///     let _response = page.goto(&format!("data:text/html,{}", html), None).await?;
///
///     // Demonstrate title()
///     let title = page.title().await?;
///     assert_eq!(title, "Test Page");
///
///     // Demonstrate locator()
///     let heading = page.locator("#heading").await;
///     let text = heading.text_content().await?;
///     assert_eq!(text, Some("Hello World".to_string()));
///
///     // Demonstrate query_selector()
///     let element = page.query_selector("h1").await?;
///     assert!(element.is_some(), "Should find the h1 element");
///
///     // Demonstrate query_selector_all()
///     let paragraphs = page.query_selector_all("p").await?;
///     assert_eq!(paragraphs.len(), 2);
///
///     // Demonstrate evaluate()
///     page.evaluate("console.log('Hello from Playwright!')").await?;
///
///     // Demonstrate evaluate_value()
///     let result = page.evaluate_value("1 + 1").await?;
///     assert_eq!(result, "2");
///
///     // Demonstrate screenshot()
///     let bytes = page.screenshot(None).await?;
///     assert!(!bytes.is_empty());
///
///     // Demonstrate screenshot_to_file()
///     let temp_dir = std::env::temp_dir();
///     let path = temp_dir.join("playwright_doctest_screenshot.png");
///     let bytes = page.screenshot_to_file(&path, Some(
///         ScreenshotOptions::builder()
///             .screenshot_type(ScreenshotType::Png)
///             .build()
///     )).await?;
///     assert!(!bytes.is_empty());
///
///     // Demonstrate reload()
///     // Data URLs may not return a response on reload (this is normal)
///     let _response = page.reload(None).await?;
///
///     // Demonstrate route() - network interception (returns Subscription)
///     let _route_sub = page.route("**/*.png", |route| async move {
///         route.abort(None).await
///     }).await?;
///
///     // Demonstrate on_download() - download handler (returns Subscription)
///     let _download_sub = page.on_download(|download| async move {
///         println!("Download started: {}", download.url());
///         Ok(())
///     });
///
///     // Demonstrate on_dialog() - dialog handler (returns Subscription)
///     let _dialog_sub = page.on_dialog(|dialog| async move {
///         println!("Dialog: {} - {}", dialog.type_(), dialog.message());
///         dialog.accept(None).await
///     });
///
///     // Demonstrate close()
///     page.close().await?;
///
///     browser.close().await?;
///     Ok(())
/// }
/// ```
///
/// See: <https://playwright.dev/docs/api/class-page>
#[derive(Clone)]
pub struct Page {
    base: ChannelOwnerImpl,
    /// Current URL of the page
    /// Wrapped in RwLock to allow updates from events
    url: Arc<RwLock<String>>,
    /// GUID of the main frame
    main_frame_guid: Arc<str>,
    /// Route handlers for network interception
    route_handlers: Arc<Mutex<Vec<RouteHandlerEntry>>>,
    /// Download event handlers
    download_handlers: Arc<Mutex<Vec<DownloadHandlerEntry>>>,
    /// Dialog event handlers
    dialog_handlers: Arc<Mutex<Vec<DialogHandlerEntry>>>,
    /// Console message broadcast channel
    console_tx: broadcast::Sender<ConsoleMessage>,
}

/// Type alias for boxed route handler future
type RouteHandlerFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;

/// Type alias for boxed download handler future
type DownloadHandlerFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;

/// Type alias for boxed dialog handler future
type DialogHandlerFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;

/// Console message received from a page.
///
/// Console messages are emitted when JavaScript code in the page calls console API
/// methods like `console.log()`, `console.error()`, etc.
///
/// # Example
///
/// ```ignore
/// let mut rx = page.console_messages();
/// while let Ok(msg) = rx.recv().await {
///     println!("[{}] {}", msg.kind(), msg.text());
/// }
/// ```
///
/// See: <https://playwright.dev/docs/api/class-consolemessage>
#[derive(Debug, Clone)]
pub struct ConsoleMessage {
    /// The type of console message (log, error, warning, etc.)
    kind: ConsoleMessageKind,
    /// The text content of the message
    text: String,
    /// Source location where the message was logged
    location: Option<ConsoleLocation>,
}

impl ConsoleMessage {
    /// Returns the type of console message.
    pub fn kind(&self) -> ConsoleMessageKind {
        self.kind
    }

    /// Returns the text content of the message.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the source location where the message was logged, if available.
    pub fn location(&self) -> Option<&ConsoleLocation> {
        self.location.as_ref()
    }
}

/// The type of console message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleMessageKind {
    /// `console.log()`
    Log,
    /// `console.debug()`
    Debug,
    /// `console.info()`
    Info,
    /// `console.warn()`
    Warning,
    /// `console.error()`
    Error,
    /// `console.dir()`
    Dir,
    /// `console.dirxml()`
    DirXml,
    /// `console.table()`
    Table,
    /// `console.trace()`
    Trace,
    /// `console.clear()`
    Clear,
    /// `console.count()`
    Count,
    /// `console.assert()`
    Assert,
    /// `console.profile()`
    Profile,
    /// `console.profileEnd()`
    ProfileEnd,
    /// `console.timeEnd()`
    TimeEnd,
    /// Unknown console type
    Other,
}

impl ConsoleMessageKind {
    fn from_str(s: &str) -> Self {
        match s {
            "log" => Self::Log,
            "debug" => Self::Debug,
            "info" => Self::Info,
            "warning" => Self::Warning,
            "error" => Self::Error,
            "dir" => Self::Dir,
            "dirxml" => Self::DirXml,
            "table" => Self::Table,
            "trace" => Self::Trace,
            "clear" => Self::Clear,
            "count" => Self::Count,
            "assert" => Self::Assert,
            "profile" => Self::Profile,
            "profileEnd" => Self::ProfileEnd,
            "timeEnd" => Self::TimeEnd,
            _ => Self::Other,
        }
    }
}

impl std::fmt::Display for ConsoleMessageKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Log => write!(f, "log"),
            Self::Debug => write!(f, "debug"),
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
            Self::Dir => write!(f, "dir"),
            Self::DirXml => write!(f, "dirxml"),
            Self::Table => write!(f, "table"),
            Self::Trace => write!(f, "trace"),
            Self::Clear => write!(f, "clear"),
            Self::Count => write!(f, "count"),
            Self::Assert => write!(f, "assert"),
            Self::Profile => write!(f, "profile"),
            Self::ProfileEnd => write!(f, "profileEnd"),
            Self::TimeEnd => write!(f, "timeEnd"),
            Self::Other => write!(f, "other"),
        }
    }
}

/// Source code location for a console message.
#[derive(Debug, Clone)]
pub struct ConsoleLocation {
    /// Source URL
    pub url: String,
    /// Line number (0-indexed)
    pub line_number: u32,
    /// Column number (0-indexed)
    pub column_number: u32,
}

/// Unique identifier for event handlers
type HandlerId = u64;

/// Counter for generating unique handler IDs
static NEXT_HANDLER_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Generates a new unique handler ID
fn next_handler_id() -> HandlerId {
    NEXT_HANDLER_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

/// Storage for a single route handler
#[derive(Clone)]
struct RouteHandlerEntry {
    id: HandlerId,
    pattern: String,
    handler: Arc<dyn Fn(Route) -> RouteHandlerFuture + Send + Sync>,
}

/// Storage for a download handler with ID
#[derive(Clone)]
struct DownloadHandlerEntry {
    id: HandlerId,
    handler: Arc<dyn Fn(Download) -> DownloadHandlerFuture + Send + Sync>,
}

/// Storage for a dialog handler with ID
#[derive(Clone)]
struct DialogHandlerEntry {
    id: HandlerId,
    handler: Arc<dyn Fn(Dialog) -> DialogHandlerFuture + Send + Sync>,
}

/// A subscription handle that unregisters the event handler when dropped.
///
/// This type implements the RAII (Resource Acquisition Is Initialization) pattern
/// for event subscriptions. When a `Subscription` is dropped, the associated event
/// handler is automatically removed from the page. This ensures that handlers don't
/// leak and provides explicit lifetime control.
///
/// # Lifetime Management
///
/// The subscription holds a weak reference to the handler list, so dropping it
/// after the page is closed is safe and will simply do nothing.
///
/// # Example
///
/// ```ignore
/// // Register a route handler - returns a Subscription
/// let subscription = page.route("**/*.png", |route| async move {
///     route.abort(None).await
/// }).await?;
///
/// // Handler is active while subscription is held...
/// do_something().await;
///
/// // Explicitly unsubscribe when done
/// subscription.unsubscribe();
///
/// // Or simply drop it
/// // drop(subscription);
/// ```
///
/// # Storing Subscriptions
///
/// To keep handlers active for the lifetime of your application, store the
/// subscription in a struct or collection:
///
/// ```ignore
/// struct MyApp {
///     page: Page,
///     _route_subscription: Subscription,
/// }
/// ```
pub struct Subscription {
    id: HandlerId,
    inner: SubscriptionInner,
}

/// Internal storage for the weak reference to the handler list.
enum SubscriptionInner {
    Route(std::sync::Weak<Mutex<Vec<RouteHandlerEntry>>>),
    Download(std::sync::Weak<Mutex<Vec<DownloadHandlerEntry>>>),
    Dialog(std::sync::Weak<Mutex<Vec<DialogHandlerEntry>>>),
}

impl Subscription {
    fn new_route(id: HandlerId, handlers: &Arc<Mutex<Vec<RouteHandlerEntry>>>) -> Self {
        Self {
            id,
            inner: SubscriptionInner::Route(Arc::downgrade(handlers)),
        }
    }

    fn new_download(id: HandlerId, handlers: &Arc<Mutex<Vec<DownloadHandlerEntry>>>) -> Self {
        Self {
            id,
            inner: SubscriptionInner::Download(Arc::downgrade(handlers)),
        }
    }

    fn new_dialog(id: HandlerId, handlers: &Arc<Mutex<Vec<DialogHandlerEntry>>>) -> Self {
        Self {
            id,
            inner: SubscriptionInner::Dialog(Arc::downgrade(handlers)),
        }
    }

    /// Returns the unique identifier for this subscription.
    ///
    /// Each subscription has a unique ID that can be used for debugging or
    /// tracking purposes. IDs are globally unique across all subscription types.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Explicitly unsubscribes the event handler.
    ///
    /// This is equivalent to dropping the subscription, but provides a more
    /// explicit API when you want to clearly indicate the intent to unsubscribe.
    ///
    /// After calling this method, the handler will no longer be invoked for
    /// matching events.
    pub fn unsubscribe(self) {
        drop(self);
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        match &self.inner {
            SubscriptionInner::Route(weak) => {
                if let Some(handlers) = weak.upgrade() {
                    handlers.lock().retain(|e| e.id != self.id);
                }
            }
            SubscriptionInner::Download(weak) => {
                if let Some(handlers) = weak.upgrade() {
                    handlers.lock().retain(|e| e.id != self.id);
                }
            }
            SubscriptionInner::Dialog(weak) => {
                if let Some(handlers) = weak.upgrade() {
                    handlers.lock().retain(|e| e.id != self.id);
                }
            }
        }
    }
}

impl std::fmt::Debug for Subscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match &self.inner {
            SubscriptionInner::Route(_) => "Route",
            SubscriptionInner::Download(_) => "Download",
            SubscriptionInner::Dialog(_) => "Dialog",
        };
        f.debug_struct("Subscription")
            .field("id", &self.id)
            .field("kind", &kind)
            .finish()
    }
}

impl Page {
    /// Creates a new Page from protocol initialization
    ///
    /// This is called by the object factory when the server sends a `__create__` message
    /// for a Page object.
    ///
    /// # Arguments
    ///
    /// * `parent` - The parent BrowserContext object
    /// * `type_name` - The protocol type name ("Page")
    /// * `guid` - The unique identifier for this page
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
        let main_frame_guid: Arc<str> =
            Arc::from(initializer["mainFrame"]["guid"].as_str().ok_or_else(|| {
                pw_runtime::Error::ProtocolError(
                    "Page initializer missing 'mainFrame.guid' field".to_string(),
                )
            })?);

        let base = ChannelOwnerImpl::new(
            ParentOrConnection::Parent(parent),
            type_name,
            guid,
            initializer,
        );

        let url = Arc::new(RwLock::new("about:blank".to_string()));
        let route_handlers = Arc::new(Mutex::new(Vec::new()));
        let download_handlers = Arc::new(Mutex::new(Vec::new()));
        let dialog_handlers = Arc::new(Mutex::new(Vec::new()));
        let (console_tx, _) = broadcast::channel(256);

        Ok(Self {
            base,
            url,
            main_frame_guid,
            route_handlers,
            download_handlers,
            dialog_handlers,
            console_tx,
        })
    }

    /// Returns the channel for sending protocol messages.
    ///
    /// Used internally for sending RPC calls to the page.
    pub(crate) fn channel(&self) -> &Channel {
        self.base.channel()
    }

    /// Returns the main frame of the page.
    ///
    /// The main frame is where navigation and DOM operations actually happen.
    pub(crate) async fn main_frame(&self) -> Result<crate::Frame> {
        let frame_arc = self.connection().get_object(&self.main_frame_guid).await?;

        let frame = frame_arc.downcast_ref::<crate::Frame>().ok_or_else(|| {
            pw_runtime::Error::ProtocolError(format!(
                "Expected Frame object, got {}",
                frame_arc.type_name()
            ))
        })?;

        Ok(frame.clone())
    }

    /// Returns the current URL of the page.
    ///
    /// This returns the last committed URL. Initially, pages are at "about:blank".
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-url>
    pub fn url(&self) -> String {
        self.url.read().unwrap().clone()
    }

    /// Closes the page.
    ///
    /// This is a graceful operation that sends a close command to the page
    /// and waits for it to shut down properly.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Page has already been closed
    /// - Communication with browser process fails
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-close>
    pub async fn close(&self) -> Result<()> {
        self.channel()
            .send_no_result("close", serde_json::json!({}))
            .await
    }

    /// Brings the page to the front (activates the tab).
    ///
    /// # Errors
    ///
    /// Returns error if page has been closed or communication fails.
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-bring-to-front>
    pub async fn bring_to_front(&self) -> Result<()> {
        self.channel()
            .send_no_result("bringToFront", serde_json::json!({}))
            .await
    }

    /// Navigates to the specified URL.
    ///
    /// Returns `None` when navigating to URLs that don't produce responses (e.g., data URLs,
    /// about:blank). This matches Playwright's behavior across all language bindings.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to navigate to
    /// * `options` - Optional navigation options (timeout, wait_until)
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - URL is invalid
    /// - Navigation timeout (default 30s)
    /// - Network error
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-goto>
    pub async fn goto(&self, url: &str, options: Option<GotoOptions>) -> Result<Option<Response>> {
        // Delegate to main frame
        let frame = self.main_frame().await.map_err(|e| match e {
            Error::TargetClosed { context, .. } => Error::TargetClosed {
                target_type: "Page".to_string(),
                context,
            },
            other => other,
        })?;

        let response = frame.goto(url, options).await.map_err(|e| match e {
            Error::TargetClosed { context, .. } => Error::TargetClosed {
                target_type: "Page".to_string(),
                context,
            },
            other => other,
        })?;

        if let Some(ref resp) = response {
            if let Ok(mut page_url) = self.url.write() {
                *page_url = resp.url().to_string();
            }
        }

        Ok(response)
    }

    /// Returns the page's title.
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-title>
    pub async fn title(&self) -> Result<String> {
        let frame = self.main_frame().await?;
        frame.title().await
    }

    /// Creates a locator for finding elements on the page.
    ///
    /// Locators are the central piece of Playwright's auto-waiting and retry-ability.
    /// They don't execute queries until an action is performed.
    ///
    /// # Arguments
    ///
    /// * `selector` - CSS selector or other locating strategy
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-locator>
    pub async fn locator(&self, selector: &str) -> crate::Locator {
        let frame = self.main_frame().await.expect("Main frame should exist");

        crate::Locator::new(Arc::new(frame), selector.to_string())
    }

    /// Returns the keyboard instance for low-level keyboard control.
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-keyboard>
    pub fn keyboard(&self) -> crate::Keyboard {
        crate::Keyboard::new(self.clone())
    }

    /// Returns the mouse instance for low-level mouse control.
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-mouse>
    pub fn mouse(&self) -> crate::Mouse {
        crate::Mouse::new(self.clone())
    }

    /// Returns the accessibility handle for inspecting the page's accessibility tree.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let snapshot = page.accessibility().snapshot(None).await?;
    /// if let Some(tree) = snapshot {
    ///     println!("Root role: {}", tree.role);
    /// }
    /// ```
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-accessibility>
    pub fn accessibility(&self) -> crate::Accessibility {
        crate::Accessibility::new(self.clone())
    }

    /// Returns the video recording handle if video recording is enabled.
    ///
    /// Video recording is enabled when [`BrowserContextOptions::record_video_dir`]
    /// is set when creating the browser context.
    ///
    /// Returns `None` if video recording is not enabled for this page's context.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Create context with video recording
    /// let options = BrowserContextOptions::builder()
    ///     .record_video_dir("/tmp/videos")
    ///     .build();
    /// let context = browser.new_context_with_options(options).await?;
    /// let page = context.new_page().await?;
    ///
    /// // Perform actions...
    /// page.goto("https://example.com", None).await?;
    ///
    /// // Get video after closing page
    /// if let Some(video) = page.video() {
    ///     page.close().await?;
    ///     let path = video.path().await?;
    ///     println!("Video saved: {}", path.display());
    /// }
    /// ```
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-video>
    ///
    /// [`BrowserContextOptions::record_video_dir`]: crate::BrowserContextOptions::record_video_dir
    pub fn video(&self) -> Option<crate::Video> {
        let video_guid = self
            .base
            .initializer()
            .get("video")
            .and_then(|v| v.get("guid"))
            .and_then(|v| v.as_str())?;

        self.base
            .children()
            .into_iter()
            .find(|child| child.guid() == video_guid)
            .and_then(|child| child.downcast_ref::<crate::Video>().cloned())
    }

    // Internal keyboard methods (called by Keyboard struct)

    pub(crate) async fn keyboard_down(&self, key: &str) -> Result<()> {
        self.channel()
            .send_no_result(
                "keyboardDown",
                serde_json::json!({
                    "key": key
                }),
            )
            .await
    }

    pub(crate) async fn keyboard_up(&self, key: &str) -> Result<()> {
        self.channel()
            .send_no_result(
                "keyboardUp",
                serde_json::json!({
                    "key": key
                }),
            )
            .await
    }

    pub(crate) async fn keyboard_press(
        &self,
        key: &str,
        options: Option<crate::KeyboardOptions>,
    ) -> Result<()> {
        let mut params = serde_json::json!({
            "key": key
        });

        if let Some(opts) = options {
            let opts_json = opts.to_json();
            if let Some(obj) = params.as_object_mut() {
                if let Some(opts_obj) = opts_json.as_object() {
                    obj.extend(opts_obj.clone());
                }
            }
        }

        self.channel().send_no_result("keyboardPress", params).await
    }

    pub(crate) async fn keyboard_type(
        &self,
        text: &str,
        options: Option<crate::KeyboardOptions>,
    ) -> Result<()> {
        let mut params = serde_json::json!({
            "text": text
        });

        if let Some(opts) = options {
            let opts_json = opts.to_json();
            if let Some(obj) = params.as_object_mut() {
                if let Some(opts_obj) = opts_json.as_object() {
                    obj.extend(opts_obj.clone());
                }
            }
        }

        self.channel().send_no_result("keyboardType", params).await
    }

    pub(crate) async fn keyboard_insert_text(&self, text: &str) -> Result<()> {
        self.channel()
            .send_no_result(
                "keyboardInsertText",
                serde_json::json!({
                    "text": text
                }),
            )
            .await
    }

    // Internal mouse methods (called by Mouse struct)

    pub(crate) async fn mouse_move(
        &self,
        x: i32,
        y: i32,
        options: Option<crate::MouseOptions>,
    ) -> Result<()> {
        let mut params = serde_json::json!({
            "x": x,
            "y": y
        });

        if let Some(opts) = options {
            let opts_json = opts.to_json();
            if let Some(obj) = params.as_object_mut() {
                if let Some(opts_obj) = opts_json.as_object() {
                    obj.extend(opts_obj.clone());
                }
            }
        }

        self.channel().send_no_result("mouseMove", params).await
    }

    pub(crate) async fn mouse_click(
        &self,
        x: i32,
        y: i32,
        options: Option<crate::MouseOptions>,
    ) -> Result<()> {
        let mut params = serde_json::json!({
            "x": x,
            "y": y
        });

        if let Some(opts) = options {
            let opts_json = opts.to_json();
            if let Some(obj) = params.as_object_mut() {
                if let Some(opts_obj) = opts_json.as_object() {
                    obj.extend(opts_obj.clone());
                }
            }
        }

        self.channel().send_no_result("mouseClick", params).await
    }

    pub(crate) async fn mouse_dblclick(
        &self,
        x: i32,
        y: i32,
        options: Option<crate::MouseOptions>,
    ) -> Result<()> {
        let mut params = serde_json::json!({
            "x": x,
            "y": y,
            "clickCount": 2
        });

        if let Some(opts) = options {
            let opts_json = opts.to_json();
            if let Some(obj) = params.as_object_mut() {
                if let Some(opts_obj) = opts_json.as_object() {
                    obj.extend(opts_obj.clone());
                }
            }
        }

        self.channel().send_no_result("mouseClick", params).await
    }

    pub(crate) async fn mouse_down(&self, options: Option<crate::MouseOptions>) -> Result<()> {
        let mut params = serde_json::json!({});

        if let Some(opts) = options {
            let opts_json = opts.to_json();
            if let Some(obj) = params.as_object_mut() {
                if let Some(opts_obj) = opts_json.as_object() {
                    obj.extend(opts_obj.clone());
                }
            }
        }

        self.channel().send_no_result("mouseDown", params).await
    }

    pub(crate) async fn mouse_up(&self, options: Option<crate::MouseOptions>) -> Result<()> {
        let mut params = serde_json::json!({});

        if let Some(opts) = options {
            let opts_json = opts.to_json();
            if let Some(obj) = params.as_object_mut() {
                if let Some(opts_obj) = opts_json.as_object() {
                    obj.extend(opts_obj.clone());
                }
            }
        }

        self.channel().send_no_result("mouseUp", params).await
    }

    pub(crate) async fn mouse_wheel(&self, delta_x: i32, delta_y: i32) -> Result<()> {
        self.channel()
            .send_no_result(
                "mouseWheel",
                serde_json::json!({
                    "deltaX": delta_x,
                    "deltaY": delta_y
                }),
            )
            .await
    }

    /// Reloads the current page.
    ///
    /// # Arguments
    ///
    /// * `options` - Optional reload options (timeout, wait_until)
    ///
    /// Returns `None` when reloading pages that don't produce responses (e.g., data URLs,
    /// about:blank). This matches Playwright's behavior across all language bindings.
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-reload>
    pub async fn reload(&self, options: Option<GotoOptions>) -> Result<Option<Response>> {
        // Build params
        let mut params = serde_json::json!({});

        if let Some(opts) = options {
            if let Some(timeout) = opts.timeout {
                params["timeout"] = serde_json::json!(timeout.as_millis() as u64);
            } else {
                params["timeout"] = serde_json::json!(pw_protocol::options::DEFAULT_TIMEOUT_MS);
            }
            if let Some(wait_until) = opts.wait_until {
                params["waitUntil"] = serde_json::json!(wait_until.as_str());
            }
        } else {
            params["timeout"] = serde_json::json!(pw_protocol::options::DEFAULT_TIMEOUT_MS);
        }

        // Send reload RPC directly to Page (not Frame!)
        #[derive(Deserialize)]
        struct ReloadResponse {
            response: Option<ResponseReference>,
        }

        #[derive(Deserialize)]
        struct ResponseReference {
            #[serde(deserialize_with = "pw_runtime::connection::deserialize_arc_str")]
            guid: Arc<str>,
        }

        let reload_result: ReloadResponse = self.channel().send("reload", params).await?;

        if let Some(response_ref) = reload_result.response {
            // Wait for Response object - __create__ may arrive after the response
            let response_arc = self
                .connection()
                .wait_for_object(&response_ref.guid, std::time::Duration::from_secs(1))
                .await?;

            let initializer = response_arc.initializer();

            let status = initializer["status"].as_u64().ok_or_else(|| {
                pw_runtime::Error::ProtocolError("Response missing status".to_string())
            })? as u16;

            let headers = initializer["headers"]
                .as_array()
                .ok_or_else(|| {
                    pw_runtime::Error::ProtocolError("Response missing headers".to_string())
                })?
                .iter()
                .filter_map(|h| {
                    let name = h["name"].as_str()?;
                    let value = h["value"].as_str()?;
                    Some((name.to_string(), value.to_string()))
                })
                .collect();

            let response = Response {
                url: initializer["url"]
                    .as_str()
                    .ok_or_else(|| {
                        pw_runtime::Error::ProtocolError("Response missing url".to_string())
                    })?
                    .to_string(),
                status,
                status_text: initializer["statusText"].as_str().unwrap_or("").to_string(),
                ok: (200..300).contains(&status),
                headers,
            };

            // Update the page's URL
            if let Ok(mut page_url) = self.url.write() {
                *page_url = response.url().to_string();
            }

            Ok(Some(response))
        } else {
            // Reload returned null (e.g., data URLs, about:blank)
            // This is a valid result, not an error
            Ok(None)
        }
    }

    /// Returns the first element matching the selector, or None if not found.
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-query-selector>
    pub async fn query_selector(
        &self,
        selector: &str,
    ) -> Result<Option<Arc<crate::ElementHandle>>> {
        let frame = self.main_frame().await?;
        frame.query_selector(selector).await
    }

    /// Returns all elements matching the selector.
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-query-selector-all>
    pub async fn query_selector_all(
        &self,
        selector: &str,
    ) -> Result<Vec<Arc<crate::ElementHandle>>> {
        let frame = self.main_frame().await?;
        frame.query_selector_all(selector).await
    }

    /// Takes a screenshot of the page and returns the image bytes.
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-screenshot>
    pub async fn screenshot(&self, options: Option<crate::ScreenshotOptions>) -> Result<Vec<u8>> {
        let params = if let Some(opts) = options {
            opts.to_json()
        } else {
            // Default to PNG with required timeout
            serde_json::json!({
                "type": "png",
                "timeout": pw_protocol::options::DEFAULT_TIMEOUT_MS
            })
        };

        #[derive(Deserialize)]
        struct ScreenshotResponse {
            binary: String,
        }

        let response: ScreenshotResponse = self.channel().send("screenshot", params).await?;

        let bytes = base64::prelude::BASE64_STANDARD
            .decode(&response.binary)
            .map_err(|e| {
                pw_runtime::Error::ProtocolError(format!("Failed to decode screenshot: {}", e))
            })?;

        Ok(bytes)
    }

    /// Takes a screenshot and saves it to a file, also returning the bytes.
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-screenshot>
    pub async fn screenshot_to_file(
        &self,
        path: &std::path::Path,
        options: Option<crate::ScreenshotOptions>,
    ) -> Result<Vec<u8>> {
        let bytes = self.screenshot(options).await?;

        tokio::fs::write(path, &bytes).await.map_err(|e| {
            pw_runtime::Error::ProtocolError(format!("Failed to write screenshot file: {}", e))
        })?;

        Ok(bytes)
    }

    /// Evaluates JavaScript in the page context.
    ///
    /// Executes the provided JavaScript expression or function within the page's
    /// context and returns the result. The return value must be JSON-serializable.
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-evaluate>
    pub async fn evaluate(&self, expression: &str) -> Result<()> {
        // Delegate to the main frame, matching playwright-python's behavior
        let frame = self.main_frame().await?;
        frame.frame_evaluate_expression(expression).await
    }

    /// Evaluates a JavaScript expression and returns the result as a String.
    ///
    /// # Arguments
    ///
    /// * `expression` - JavaScript code to evaluate
    ///
    /// # Returns
    ///
    /// The result converted to a String
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-evaluate>
    pub async fn evaluate_value(&self, expression: &str) -> Result<String> {
        let frame = self.main_frame().await?;
        frame.frame_evaluate_expression_value(expression).await
    }

    /// Evaluates a JavaScript expression and returns the result as [`serde_json::Value`].
    ///
    /// This method is useful when working with complex objects or arrays where you
    /// need access to the full JSON structure rather than a string representation.
    ///
    /// # Arguments
    ///
    /// * `expression` - JavaScript code to evaluate in the page context
    ///
    /// # Returns
    ///
    /// The evaluation result as a [`serde_json::Value`], which can be an object,
    /// array, string, number, boolean, or null.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProtocolError`] if:
    /// - The JavaScript expression throws an exception
    /// - The result contains non-serializable values (e.g., DOM elements, functions)
    /// - The page has been closed
    ///
    /// # Example
    ///
    /// ```ignore
    /// let value = page.evaluate_json("({ name: 'test', count: 42 })").await?;
    /// assert_eq!(value["name"], "test");
    /// assert_eq!(value["count"], 42);
    /// ```
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-evaluate>
    pub async fn evaluate_json(&self, expression: &str) -> Result<serde_json::Value> {
        let frame = self.main_frame().await?;
        frame.frame_evaluate_expression_json(expression).await
    }

    /// Evaluates a JavaScript expression and deserializes the result to a typed value.
    ///
    /// This is the most ergonomic way to evaluate JavaScript when you know the
    /// expected return type at compile time. The result is automatically deserialized
    /// using serde.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type to deserialize into. Must implement [`serde::de::DeserializeOwned`].
    ///
    /// # Arguments
    ///
    /// * `expression` - JavaScript code to evaluate in the page context
    ///
    /// # Returns
    ///
    /// The evaluation result deserialized as type `T`.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - The JavaScript expression throws an exception
    /// - The result cannot be deserialized to type `T`
    /// - The page has been closed
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[derive(Deserialize)]
    /// struct PageInfo {
    ///     title: String,
    ///     url: String,
    /// }
    ///
    /// let info: PageInfo = page.evaluate_typed(
    ///     "({ title: document.title, url: location.href })"
    /// ).await?;
    /// println!("Page: {} at {}", info.title, info.url);
    /// ```
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-evaluate>
    pub async fn evaluate_typed<T: serde::de::DeserializeOwned>(
        &self,
        expression: &str,
    ) -> Result<T> {
        let frame = self.main_frame().await?;
        frame.frame_evaluate_expression_typed(expression).await
    }

    /// Registers a route handler for network interception.
    ///
    /// When a request matches the specified pattern, the handler will be called
    /// with a Route object that can abort, continue, or fulfill the request.
    ///
    /// Returns a [`Subscription`] that will automatically unregister the handler
    /// when dropped. To keep the handler active, store the subscription.
    ///
    /// # Arguments
    ///
    /// * `pattern` - URL pattern to match (supports glob patterns like "**/*.png")
    /// * `handler` - Async closure that handles the route
    ///
    /// # Returns
    ///
    /// A [`Subscription`] that unregisters the handler when dropped.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Register a route handler
    /// let subscription = page.route("**/*.png", |route| async move {
    ///     route.abort(None).await
    /// }).await?;
    ///
    /// // Handler is active while subscription is held
    ///
    /// // To explicitly unregister:
    /// subscription.unsubscribe();
    /// // Or just drop it:
    /// // drop(subscription);
    /// ```
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-route>
    pub async fn route<F, Fut>(&self, pattern: &str, handler: F) -> Result<Subscription>
    where
        F: Fn(Route) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        // 1. Generate unique ID and wrap handler
        let id = next_handler_id();
        let handler =
            Arc::new(move |route: Route| -> RouteHandlerFuture { Box::pin(handler(route)) });

        // 2. Store in handlers list
        self.route_handlers.lock().push(RouteHandlerEntry {
            id,
            pattern: pattern.to_string(),
            handler,
        });

        // 3. Enable network interception via protocol
        self.enable_network_interception().await?;

        // 4. Return subscription handle
        Ok(Subscription::new_route(id, &self.route_handlers))
    }

    /// Updates network interception patterns for this page
    async fn enable_network_interception(&self) -> Result<()> {
        // Collect all patterns from registered handlers
        // Each pattern must be an object with "glob" field
        let patterns: Vec<serde_json::Value> = self
            .route_handlers
            .lock()
            .iter()
            .map(|entry| serde_json::json!({ "glob": entry.pattern }))
            .collect();

        // Send protocol command to update network interception patterns
        // Follows playwright-python's approach
        self.channel()
            .send_no_result(
                "setNetworkInterceptionPatterns",
                serde_json::json!({
                    "patterns": patterns
                }),
            )
            .await
    }

    /// Handles a route event from the protocol
    ///
    /// Called by on_event when a "route" event is received
    async fn on_route_event(&self, route: Route) {
        let handlers = self.route_handlers.lock().clone();
        let url = route.request().url().to_string();

        for entry in handlers.iter().rev() {
            if Self::matches_pattern(&entry.pattern, &url) {
                let handler = entry.handler.clone();
                // Ensure fulfill/continue/abort completes before browser continues
                if let Err(e) = handler(route).await {
                    tracing::error!(error = %e, "Route handler error");
                }
                break;
            }
        }
    }

    /// Checks if a URL matches a glob pattern
    ///
    /// Supports standard glob patterns:
    /// - `*` matches any characters except `/`
    /// - `**` matches any characters including `/`
    /// - `?` matches a single character
    fn matches_pattern(pattern: &str, url: &str) -> bool {
        use glob::Pattern;

        // Try to compile the glob pattern
        match Pattern::new(pattern) {
            Ok(glob_pattern) => glob_pattern.matches(url),
            Err(_) => pattern == url, // Fall back to exact string match on invalid pattern
        }
    }

    /// Registers a download event handler.
    ///
    /// The handler will be called when a download is triggered by the page.
    /// Downloads occur when the page initiates a file download (e.g., clicking a link
    /// with the download attribute, or a server response with Content-Disposition: attachment).
    ///
    /// Returns a [`Subscription`] that will automatically unregister the handler
    /// when dropped. To keep the handler active, store the subscription.
    ///
    /// # Arguments
    ///
    /// * `handler` - Async closure that receives the Download object
    ///
    /// # Returns
    ///
    /// A [`Subscription`] that unregisters the handler when dropped.
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-event-download>
    pub fn on_download<F, Fut>(&self, handler: F) -> Subscription
    where
        F: Fn(Download) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        let id = next_handler_id();
        let handler = Arc::new(move |download: Download| -> DownloadHandlerFuture {
            Box::pin(handler(download))
        });

        self.download_handlers
            .lock()
            .push(DownloadHandlerEntry { id, handler });

        Subscription::new_download(id, &self.download_handlers)
    }

    /// Registers a dialog event handler.
    ///
    /// The handler will be called when a JavaScript dialog is triggered (alert, confirm, prompt, or beforeunload).
    /// The dialog must be explicitly accepted or dismissed, otherwise the page will freeze.
    ///
    /// Returns a [`Subscription`] that will automatically unregister the handler
    /// when dropped. To keep the handler active, store the subscription.
    ///
    /// # Arguments
    ///
    /// * `handler` - Async closure that receives the Dialog object
    ///
    /// # Returns
    ///
    /// A [`Subscription`] that unregisters the handler when dropped.
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-event-dialog>
    pub fn on_dialog<F, Fut>(&self, handler: F) -> Subscription
    where
        F: Fn(Dialog) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        let id = next_handler_id();
        let handler =
            Arc::new(move |dialog: Dialog| -> DialogHandlerFuture { Box::pin(handler(dialog)) });

        self.dialog_handlers
            .lock()
            .push(DialogHandlerEntry { id, handler });

        Subscription::new_dialog(id, &self.dialog_handlers)
    }

    /// Returns a receiver for console messages from the page.
    ///
    /// Console messages are emitted when JavaScript code calls console API methods
    /// like `console.log()`, `console.error()`, etc.
    ///
    /// The returned receiver is a broadcast receiver. If the receiver falls behind
    /// (processing messages slower than they arrive), older messages may be dropped
    /// with a `RecvError::Lagged` error.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut rx = page.console_messages();
    ///
    /// // In a background task
    /// tokio::spawn(async move {
    ///     while let Ok(msg) = rx.recv().await {
    ///         println!("[{}] {}", msg.kind(), msg.text());
    ///     }
    /// });
    /// ```
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-event-console>
    pub fn console_messages(&self) -> broadcast::Receiver<ConsoleMessage> {
        self.console_tx.subscribe()
    }

    /// Waits for a console message matching the predicate.
    ///
    /// Returns the first [`ConsoleMessage`] for which `predicate` returns `true`.
    ///
    /// # Errors
    ///
    /// - [`Error::Timeout`] if no matching message arrives within `timeout`
    /// - [`Error::ChannelClosed`] if the page is closed
    ///
    /// [`Error::Timeout`]: pw_runtime::Error::Timeout
    /// [`Error::ChannelClosed`]: pw_runtime::Error::ChannelClosed
    ///
    /// # Example
    ///
    /// ```ignore
    /// let msg = page.wait_for_console(
    ///     |msg| msg.text().contains("ready"),
    ///     std::time::Duration::from_secs(10)
    /// ).await?;
    /// ```
    pub async fn wait_for_console<F>(
        &self,
        predicate: F,
        timeout: std::time::Duration,
    ) -> Result<ConsoleMessage>
    where
        F: Fn(&ConsoleMessage) -> bool,
    {
        let mut rx = self.console_messages();

        tokio::time::timeout(timeout, async move {
            loop {
                match rx.recv().await {
                    Ok(msg) if predicate(&msg) => return Ok(msg),
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(dropped = n, "Console message receiver lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        return Err(Error::ChannelClosed);
                    }
                }
            }
        })
        .await
        .map_err(|_| Error::Timeout("Timeout waiting for console message".to_string()))?
    }

    /// Registers a console message callback.
    ///
    /// The callback will be invoked for each console message emitted by the page.
    /// Unlike [`console_messages()`](Self::console_messages), which returns a receiver that you
    /// poll manually, this method spawns a background task that invokes your callback.
    ///
    /// Returns a [`ConsoleSubscription`] that cancels the background task when dropped.
    /// To keep receiving messages, store the subscription.
    ///
    /// # Arguments
    ///
    /// * `handler` - A synchronous callback that receives each console message
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Store the subscription to keep the handler active
    /// let _sub = page.on_console(|msg| {
    ///     println!("[{}] {}", msg.kind(), msg.text());
    /// });
    ///
    /// // Navigate and trigger console messages
    /// page.goto("https://example.com", None).await?;
    ///
    /// // When _sub is dropped, the handler stops receiving messages
    /// ```
    ///
    /// See: <https://playwright.dev/docs/api/class-page#page-event-console>
    ///
    /// [`ConsoleSubscription`]: crate::events::ConsoleSubscription
    pub fn on_console<F>(&self, handler: F) -> super::events::ConsoleSubscription
    where
        F: Fn(ConsoleMessage) + Send + Sync + 'static,
    {
        use tokio::sync::oneshot;

        let mut rx = self.console_messages();
        let (cancel_tx, mut cancel_rx) = oneshot::channel::<()>();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = rx.recv() => {
                        match result {
                            Ok(msg) => handler(msg),
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                tracing::warn!(dropped = n, "Console callback lagged");
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                    _ = &mut cancel_rx => break,
                }
            }
        });

        super::events::ConsoleSubscription::new(cancel_tx)
    }

    /// Handles a download event from the protocol
    async fn on_download_event(&self, download: Download) {
        let handlers = self.download_handlers.lock().clone();

        for entry in handlers {
            if let Err(e) = (entry.handler)(download.clone()).await {
                tracing::error!(error = %e, handler_id = entry.id, "Download handler error");
            }
        }
    }

    /// Handles a dialog event from the protocol
    async fn on_dialog_event(&self, dialog: Dialog) {
        let handlers = self.dialog_handlers.lock().clone();

        for entry in handlers {
            if let Err(e) = (entry.handler)(dialog.clone()).await {
                tracing::error!(error = %e, handler_id = entry.id, "Dialog handler error");
            }
        }
    }

    /// Triggers dialog event (called by BrowserContext when dialog events arrive)
    ///
    /// Dialog events are sent to BrowserContext and forwarded to the associated Page.
    /// This method is public so BrowserContext can forward dialog events.
    pub async fn trigger_dialog_event(&self, dialog: Dialog) {
        self.on_dialog_event(dialog).await;
    }
}

impl pw_runtime::channel_owner::private::Sealed for Page {}

impl ChannelOwner for Page {
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
            "navigated" => {
                // Update URL when page navigates
                if let Some(url_value) = params.get("url") {
                    if let Some(url_str) = url_value.as_str() {
                        if let Ok(mut url) = self.url.write() {
                            *url = url_str.to_string();
                        }
                    }
                }
            }
            "route" => {
                if let Some(route_guid) = params
                    .get("route")
                    .and_then(|v| v.get("guid"))
                    .and_then(|v| v.as_str())
                {
                    let connection = self.connection();
                    let route_guid_owned = route_guid.to_string();
                    let self_clone = self.clone();

                    tokio::spawn(async move {
                        let route_arc = match connection.get_object(&route_guid_owned).await {
                            Ok(obj) => obj,
                            Err(e) => {
                                tracing::error!(error = %e, guid = %route_guid_owned, "Failed to get route object");
                                return;
                            }
                        };

                        let route = match route_arc.downcast_ref::<Route>() {
                            Some(r) => r.clone(),
                            None => {
                                tracing::error!(guid = %route_guid_owned, "Failed to downcast to Route");
                                return;
                            }
                        };

                        self_clone.on_route_event(route).await;
                    });
                }
            }
            "download" => {
                // Event params: {url, suggestedFilename, artifact: {guid: "..."}}
                let url = params
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let suggested_filename = params
                    .get("suggestedFilename")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                if let Some(artifact_guid) = params
                    .get("artifact")
                    .and_then(|v| v.get("guid"))
                    .and_then(|v| v.as_str())
                {
                    let connection = self.connection();
                    let artifact_guid_owned = artifact_guid.to_string();
                    let self_clone = self.clone();

                    tokio::spawn(async move {
                        let artifact_arc = match connection.get_object(&artifact_guid_owned).await {
                            Ok(obj) => obj,
                            Err(e) => {
                                tracing::error!(error = %e, guid = %artifact_guid_owned, "Failed to get artifact object");
                                return;
                            }
                        };

                        let download =
                            Download::from_artifact(artifact_arc, url, suggested_filename);

                        self_clone.on_download_event(download).await;
                    });
                }
            }
            "dialog" => {
                // Handled by BrowserContext and forwarded to Page
            }
            "console" => {
                if let Some(message_obj) = params.get("message") {
                    let kind = message_obj
                        .get("type")
                        .and_then(|v| v.as_str())
                        .map(ConsoleMessageKind::from_str)
                        .unwrap_or(ConsoleMessageKind::Log);

                    let text = message_obj
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let location = message_obj.get("location").and_then(|loc| {
                        Some(ConsoleLocation {
                            url: loc.get("url")?.as_str()?.to_string(),
                            line_number: loc.get("lineNumber")?.as_u64()? as u32,
                            column_number: loc.get("columnNumber")?.as_u64()? as u32,
                        })
                    });

                    let _ = self.console_tx.send(ConsoleMessage {
                        kind,
                        text,
                        location,
                    });
                }
            }
            _ => {
                // TODO: Future events - load, domcontentloaded, close, crash, etc.
            }
        }
    }

    fn was_collected(&self) -> bool {
        self.base.was_collected()
    }
}

impl std::fmt::Debug for Page {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Page")
            .field("guid", &self.guid())
            .field("url", &self.url())
            .finish()
    }
}

/// Options for page.goto() and page.reload()
#[derive(Debug, Clone)]
pub struct GotoOptions {
    /// Maximum operation time in milliseconds
    pub timeout: Option<std::time::Duration>,
    /// When to consider operation succeeded
    pub wait_until: Option<WaitUntil>,
}

impl GotoOptions {
    /// Creates new GotoOptions with default values
    pub fn new() -> Self {
        Self {
            timeout: None,
            wait_until: None,
        }
    }

    /// Sets the timeout
    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets the wait_until option
    pub fn wait_until(mut self, wait_until: WaitUntil) -> Self {
        self.wait_until = Some(wait_until);
        self
    }
}

impl Default for GotoOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// When to consider navigation succeeded
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitUntil {
    /// Consider operation to be finished when the `load` event is fired
    Load,
    /// Consider operation to be finished when the `DOMContentLoaded` event is fired
    DomContentLoaded,
    /// Consider operation to be finished when there are no network connections for at least 500ms
    NetworkIdle,
    /// Consider operation to be finished when the commit event is fired
    Commit,
}

impl WaitUntil {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            WaitUntil::Load => "load",
            WaitUntil::DomContentLoaded => "domcontentloaded",
            WaitUntil::NetworkIdle => "networkidle",
            WaitUntil::Commit => "commit",
        }
    }
}

/// Response from navigation operations
#[derive(Debug, Clone)]
pub struct Response {
    /// URL of the response
    pub url: String,
    /// HTTP status code
    pub status: u16,
    /// HTTP status text
    pub status_text: String,
    /// Whether the response was successful (status 200-299)
    pub ok: bool,
    /// Response headers
    pub headers: std::collections::HashMap<String, String>,
}

impl Response {
    /// Returns the URL of the response
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Returns the HTTP status code
    pub fn status(&self) -> u16 {
        self.status
    }

    /// Returns the HTTP status text
    pub fn status_text(&self) -> &str {
        &self.status_text
    }

    /// Returns whether the response was successful (status 200-299)
    pub fn ok(&self) -> bool {
        self.ok
    }

    /// Returns the response headers
    pub fn headers(&self) -> &std::collections::HashMap<String, String> {
        &self.headers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_console_message_kind_from_str() {
        assert_eq!(ConsoleMessageKind::from_str("log"), ConsoleMessageKind::Log);
        assert_eq!(
            ConsoleMessageKind::from_str("error"),
            ConsoleMessageKind::Error
        );
        assert_eq!(
            ConsoleMessageKind::from_str("warning"),
            ConsoleMessageKind::Warning
        );
        assert_eq!(
            ConsoleMessageKind::from_str("info"),
            ConsoleMessageKind::Info
        );
        assert_eq!(
            ConsoleMessageKind::from_str("debug"),
            ConsoleMessageKind::Debug
        );
        assert_eq!(
            ConsoleMessageKind::from_str("unknown"),
            ConsoleMessageKind::Other
        );
    }

    #[test]
    fn test_console_message_kind_display() {
        assert_eq!(format!("{}", ConsoleMessageKind::Log), "log");
        assert_eq!(format!("{}", ConsoleMessageKind::Error), "error");
        assert_eq!(format!("{}", ConsoleMessageKind::Warning), "warning");
        assert_eq!(format!("{}", ConsoleMessageKind::Other), "other");
    }

    #[test]
    fn test_console_message_accessors() {
        let msg = ConsoleMessage {
            kind: ConsoleMessageKind::Log,
            text: "Hello, World!".to_string(),
            location: Some(ConsoleLocation {
                url: "http://example.com/script.js".to_string(),
                line_number: 42,
                column_number: 10,
            }),
        };

        assert_eq!(msg.kind(), ConsoleMessageKind::Log);
        assert_eq!(msg.text(), "Hello, World!");
        let loc = msg.location().unwrap();
        assert_eq!(loc.url, "http://example.com/script.js");
        assert_eq!(loc.line_number, 42);
        assert_eq!(loc.column_number, 10);
    }

    #[test]
    fn test_console_message_without_location() {
        let msg = ConsoleMessage {
            kind: ConsoleMessageKind::Error,
            text: "Something went wrong".to_string(),
            location: None,
        };

        assert_eq!(msg.kind(), ConsoleMessageKind::Error);
        assert_eq!(msg.text(), "Something went wrong");
        assert!(msg.location().is_none());
    }
}
