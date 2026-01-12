//! Testing infrastructure for pw-cli.
//!
//! Provides traits and mock implementations for testing CLI commands without
//! spawning actual browsers.
//!
//! The testing infrastructure follows trait-based dependency injection:
//! - [`PageLike`]: Abstracts page operations (click, text, screenshot, eval)
//! - [`SessionLike`]: Abstracts session lifecycle (goto, page access, close)
//! - [`LocatorLike`]: Abstracts locator operations (click, text_content, count)
//!
//! # Example
//!
//! ```ignore
//! use pw_cli::testing::{MockSession, MockPage};
//!
//! #[tokio::test]
//! async fn test_click_command() {
//!     let page = MockPage::new();
//!     page.set_url("https://example.com");
//!     page.set_text_for_selector("h1", "Hello World");
//!
//!     let session = MockSession::new(page);
//!     // ... test command with session
//! }
//! ```

use crate::error::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Abstracts page operations for testing.
///
/// Mirrors the subset of [`pw::Page`] methods used by CLI commands.
/// Implement this trait to create custom test doubles, or use the provided
/// [`MockPage`] implementation.
#[async_trait]
pub trait PageLike: Send + Sync {
    /// Returns the current URL of the page.
    fn url(&self) -> String;

    /// Evaluates a JavaScript `expression` and returns the result as a string.
    async fn evaluate_value(&self, expression: &str) -> Result<String>;

    /// Evaluates a JavaScript `expression` and returns the result as JSON.
    async fn evaluate_json(&self, expression: &str) -> Result<serde_json::Value>;

    /// Creates a [`LocatorLike`] for the given CSS `selector`.
    fn locator(&self, selector: &str) -> Box<dyn LocatorLike + '_>;

    /// Takes a screenshot, capturing the full scrollable page if `full_page` is true.
    async fn screenshot(&self, full_page: bool) -> Result<Vec<u8>>;

    /// Takes a screenshot, saves to `path`, and returns the bytes.
    async fn screenshot_to_file(&self, path: &Path, full_page: bool) -> Result<Vec<u8>>;

    /// Returns the page title from the `<title>` element.
    async fn title(&self) -> Result<String>;

    /// Returns HTML content for the `selector`, or the full page if [`None`].
    async fn html(&self, selector: Option<&str>) -> Result<String>;
}

/// Abstracts locator operations for testing.
///
/// Mirrors the subset of [`pw::Locator`] methods used by CLI commands.
/// Locators represent a way to find element(s) on the page at any moment.
#[async_trait]
pub trait LocatorLike: Send + Sync {
    /// Returns the CSS selector string used to create this locator.
    fn selector(&self) -> &str;

    /// Clicks the first matching element.
    async fn click(&self) -> Result<()>;

    /// Returns the `textContent` of the first matching element, or [`None`] if not found.
    async fn text_content(&self) -> Result<Option<String>>;

    /// Returns the `innerText` of the first matching element.
    async fn inner_text(&self) -> Result<String>;

    /// Returns the `innerHTML` of the first matching element.
    async fn inner_html(&self) -> Result<String>;

    /// Returns the `outerHTML` of the first matching element.
    async fn outer_html(&self) -> Result<String>;

    /// Returns the number of elements matching the selector.
    async fn count(&self) -> Result<usize>;

    /// Types `text` into an input element, replacing any existing value.
    async fn fill(&self, text: &str) -> Result<()>;

    /// Returns the value of the `name` attribute, or [`None`] if not present.
    async fn get_attribute(&self, name: &str) -> Result<Option<String>>;

    /// Returns the element's [`BoundingBox`], or [`None`] if not visible.
    async fn bounding_box(&self) -> Result<Option<BoundingBox>>;
}

/// Element bounding box in CSS pixels, relative to the viewport.
#[derive(Debug, Clone, Copy, Default)]
pub struct BoundingBox {
    /// X coordinate of the top-left corner.
    pub x: f64,
    /// Y coordinate of the top-left corner.
    pub y: f64,
    /// Width of the element.
    pub width: f64,
    /// Height of the element.
    pub height: f64,
}

/// Abstracts session operations for testing.
///
/// Mirrors the subset of [`crate::session_broker::SessionHandle`] methods used by CLI commands.
/// A session represents a browser connection with an active page.
#[async_trait]
pub trait SessionLike: Send + Sync {
    /// Navigates the page to `url`.
    async fn goto(&self, url: &str) -> Result<()>;

    /// Navigates to `url` only if not already there. Returns `true` if navigation occurred.
    async fn goto_if_needed(&self, url: &str) -> Result<bool>;

    /// Navigates to `url` unless it equals `__CURRENT_PAGE__`. Returns `true` if navigation occurred.
    async fn goto_unless_current(&self, url: &str) -> Result<bool>;

    /// Returns a reference to the underlying [`PageLike`].
    fn page(&self) -> &dyn PageLike;

    /// Closes the session and releases browser resources.
    async fn close(self: Box<Self>) -> Result<()>;
}

/// Mock page for testing CLI commands without a browser.
///
/// Provides configurable responses for page operations and records all actions
/// for later assertion. Configure expected responses with `set_*` methods,
/// then retrieve recorded actions with [`actions()`](Self::actions).
///
/// # Example
///
/// ```
/// use pw_cli::testing::{MockPage, MockAction};
///
/// let page = MockPage::new();
/// page.set_url("https://example.com");
/// page.set_text_for_selector("h1", "Welcome");
///
/// // After running commands...
/// let actions = page.actions();
/// assert!(actions.iter().any(|a| matches!(a, MockAction::Click { .. })));
/// ```
#[derive(Default)]
pub struct MockPage {
    url: Mutex<String>,
    title: Mutex<String>,
    html: Mutex<String>,
    text_by_selector: Mutex<HashMap<String, String>>,
    html_by_selector: Mutex<HashMap<String, String>>,
    count_by_selector: Mutex<HashMap<String, usize>>,
    bbox_by_selector: Mutex<HashMap<String, BoundingBox>>,
    eval_results: Mutex<HashMap<String, serde_json::Value>>,
    screenshot_bytes: Mutex<Vec<u8>>,
    actions: Mutex<Vec<MockAction>>,
}

/// Action recorded by [`MockPage`] for test assertions.
///
/// Use [`MockPage::actions()`] to retrieve the list of actions performed
/// during a test, then assert on the expected sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockAction {
    /// A click was performed on an element.
    Click { selector: String },
    /// Text was filled into an input element.
    Fill { selector: String, text: String },
    /// JavaScript was evaluated.
    Evaluate { expression: String },
    /// A screenshot was taken.
    Screenshot { full_page: bool },
    /// Navigation was performed.
    Goto { url: String },
}

impl MockPage {
    /// Creates a new mock page at `about:blank`.
    pub fn new() -> Self {
        Self {
            url: Mutex::new("about:blank".to_string()),
            title: Mutex::new(String::new()),
            html: Mutex::new("<html><body></body></html>".to_string()),
            text_by_selector: Mutex::new(HashMap::new()),
            html_by_selector: Mutex::new(HashMap::new()),
            count_by_selector: Mutex::new(HashMap::new()),
            bbox_by_selector: Mutex::new(HashMap::new()),
            eval_results: Mutex::new(HashMap::new()),
            screenshot_bytes: Mutex::new(vec![0x89, 0x50, 0x4E, 0x47]),
            actions: Mutex::new(Vec::new()),
        }
    }

    /// Sets the current URL.
    pub fn set_url(&self, url: &str) {
        *self.url.lock().unwrap() = url.to_string();
    }

    /// Sets the page title.
    pub fn set_title(&self, title: &str) {
        *self.title.lock().unwrap() = title.to_string();
    }

    /// Sets the full page HTML.
    pub fn set_html(&self, html: &str) {
        *self.html.lock().unwrap() = html.to_string();
    }

    /// Sets text content for a selector (also sets count to 1 if unset).
    pub fn set_text_for_selector(&self, selector: &str, text: &str) {
        self.text_by_selector
            .lock()
            .unwrap()
            .insert(selector.to_string(), text.to_string());
        self.count_by_selector
            .lock()
            .unwrap()
            .entry(selector.to_string())
            .or_insert(1);
    }

    /// Sets inner HTML for a selector (also sets count to 1 if unset).
    pub fn set_html_for_selector(&self, selector: &str, html: &str) {
        self.html_by_selector
            .lock()
            .unwrap()
            .insert(selector.to_string(), html.to_string());
        self.count_by_selector
            .lock()
            .unwrap()
            .entry(selector.to_string())
            .or_insert(1);
    }

    /// Sets element count for a selector.
    pub fn set_count_for_selector(&self, selector: &str, count: usize) {
        self.count_by_selector
            .lock()
            .unwrap()
            .insert(selector.to_string(), count);
    }

    /// Sets bounding box for a selector (also sets count to 1 if unset).
    pub fn set_bbox_for_selector(&self, selector: &str, bbox: BoundingBox) {
        self.bbox_by_selector
            .lock()
            .unwrap()
            .insert(selector.to_string(), bbox);
        self.count_by_selector
            .lock()
            .unwrap()
            .entry(selector.to_string())
            .or_insert(1);
    }

    /// Sets the result for an eval expression.
    pub fn set_eval_result(&self, expression: &str, result: serde_json::Value) {
        self.eval_results
            .lock()
            .unwrap()
            .insert(expression.to_string(), result);
    }

    /// Sets screenshot bytes to return.
    pub fn set_screenshot_bytes(&self, bytes: Vec<u8>) {
        *self.screenshot_bytes.lock().unwrap() = bytes;
    }

    /// Returns all recorded actions (for test assertions).
    pub fn actions(&self) -> Vec<MockAction> {
        self.actions.lock().unwrap().clone()
    }

    /// Clears recorded actions.
    pub fn clear_actions(&self) {
        self.actions.lock().unwrap().clear();
    }

    fn record_action(&self, action: MockAction) {
        self.actions.lock().unwrap().push(action);
    }

    fn get_text_for_selector(&self, selector: &str) -> Option<String> {
        self.text_by_selector.lock().unwrap().get(selector).cloned()
    }

    fn get_html_for_selector(&self, selector: &str) -> Option<String> {
        self.html_by_selector.lock().unwrap().get(selector).cloned()
    }

    fn get_count_for_selector(&self, selector: &str) -> usize {
        self.count_by_selector
            .lock()
            .unwrap()
            .get(selector)
            .copied()
            .unwrap_or(0)
    }

    fn get_bbox_for_selector(&self, selector: &str) -> Option<BoundingBox> {
        self.bbox_by_selector.lock().unwrap().get(selector).copied()
    }
}

#[async_trait]
impl PageLike for MockPage {
    fn url(&self) -> String {
        self.url.lock().unwrap().clone()
    }

    async fn evaluate_value(&self, expression: &str) -> Result<String> {
        self.record_action(MockAction::Evaluate {
            expression: expression.to_string(),
        });

        if expression == "window.location.href" {
            return Ok(self.url());
        }

        let results = self.eval_results.lock().unwrap();
        match results.get(expression) {
            Some(serde_json::Value::String(s)) => Ok(s.clone()),
            Some(other) => Ok(other.to_string()),
            None => Ok("undefined".to_string()),
        }
    }

    async fn evaluate_json(&self, expression: &str) -> Result<serde_json::Value> {
        self.record_action(MockAction::Evaluate {
            expression: expression.to_string(),
        });

        let results = self.eval_results.lock().unwrap();
        Ok(results
            .get(expression)
            .cloned()
            .unwrap_or(serde_json::Value::Null))
    }

    fn locator(&self, selector: &str) -> Box<dyn LocatorLike + '_> {
        Box::new(MockLocator {
            selector: selector.to_string(),
            page: self,
        })
    }

    async fn screenshot(&self, full_page: bool) -> Result<Vec<u8>> {
        self.record_action(MockAction::Screenshot { full_page });
        Ok(self.screenshot_bytes.lock().unwrap().clone())
    }

    async fn screenshot_to_file(&self, _path: &Path, full_page: bool) -> Result<Vec<u8>> {
        self.screenshot(full_page).await
    }

    async fn title(&self) -> Result<String> {
        Ok(self.title.lock().unwrap().clone())
    }

    async fn html(&self, selector: Option<&str>) -> Result<String> {
        match selector {
            Some(sel) => Ok(self.get_html_for_selector(sel).unwrap_or_default()),
            None => Ok(self.html.lock().unwrap().clone()),
        }
    }
}

/// Mock locator returned by [`MockPage::locator()`].
///
/// Records click and fill actions to the parent [`MockPage`] and returns
/// configured responses for text/HTML/count queries.
pub struct MockLocator<'a> {
    selector: String,
    page: &'a MockPage,
}

#[async_trait]
impl<'a> LocatorLike for MockLocator<'a> {
    fn selector(&self) -> &str {
        &self.selector
    }

    async fn click(&self) -> Result<()> {
        self.page.record_action(MockAction::Click {
            selector: self.selector.clone(),
        });
        Ok(())
    }

    async fn text_content(&self) -> Result<Option<String>> {
        Ok(self.page.get_text_for_selector(&self.selector))
    }

    async fn inner_text(&self) -> Result<String> {
        Ok(self
            .page
            .get_text_for_selector(&self.selector)
            .unwrap_or_default())
    }

    async fn inner_html(&self) -> Result<String> {
        Ok(self
            .page
            .get_html_for_selector(&self.selector)
            .unwrap_or_default())
    }

    async fn outer_html(&self) -> Result<String> {
        self.inner_html().await
    }

    async fn count(&self) -> Result<usize> {
        Ok(self.page.get_count_for_selector(&self.selector))
    }

    async fn fill(&self, text: &str) -> Result<()> {
        self.page.record_action(MockAction::Fill {
            selector: self.selector.clone(),
            text: text.to_string(),
        });
        Ok(())
    }

    async fn get_attribute(&self, _name: &str) -> Result<Option<String>> {
        Ok(None)
    }

    async fn bounding_box(&self) -> Result<Option<BoundingBox>> {
        Ok(self.page.get_bbox_for_selector(&self.selector))
    }
}

/// Mock session for testing CLI commands without a browser.
///
/// Wraps a [`MockPage`] and tracks navigation state. Use [`default_session()`](Self::default_session)
/// for a quick setup, or [`new()`](Self::new) with a pre-configured [`MockPage`].
///
/// # Example
///
/// ```
/// use pw_cli::testing::{MockSession, MockPage};
///
/// // Quick setup with defaults
/// let session = MockSession::default_session();
///
/// // Or with a configured page
/// let page = MockPage::new();
/// page.set_text_for_selector("h1", "Hello");
/// let session = MockSession::new(page);
/// ```
pub struct MockSession {
    page: Arc<MockPage>,
    current_url: Mutex<String>,
    closed: Mutex<bool>,
}

impl MockSession {
    /// Creates a mock session wrapping the given `page`.
    pub fn new(page: MockPage) -> Self {
        let url = page.url();
        Self {
            page: Arc::new(page),
            current_url: Mutex::new(url),
            closed: Mutex::new(false),
        }
    }

    /// Creates a mock session with a default [`MockPage`] at `about:blank`.
    pub fn default_session() -> Self {
        Self::new(MockPage::new())
    }

    /// Returns the underlying [`MockPage`] for additional configuration or assertions.
    pub fn mock_page(&self) -> &MockPage {
        &self.page
    }

    /// Returns `true` if [`close()`](SessionLike::close) was called.
    pub fn is_closed(&self) -> bool {
        *self.closed.lock().unwrap()
    }
}

#[async_trait]
impl SessionLike for MockSession {
    async fn goto(&self, url: &str) -> Result<()> {
        self.page.record_action(MockAction::Goto {
            url: url.to_string(),
        });
        *self.current_url.lock().unwrap() = url.to_string();
        self.page.set_url(url);
        Ok(())
    }

    async fn goto_if_needed(&self, url: &str) -> Result<bool> {
        let current = self.current_url.lock().unwrap().clone();
        if current == url {
            Ok(false)
        } else {
            self.goto(url).await?;
            Ok(true)
        }
    }

    async fn goto_unless_current(&self, url: &str) -> Result<bool> {
        if url == "__CURRENT_PAGE__" {
            return Ok(false);
        }
        self.goto_if_needed(url).await
    }

    fn page(&self) -> &dyn PageLike {
        self.page.as_ref()
    }

    async fn close(self: Box<Self>) -> Result<()> {
        *self.closed.lock().unwrap() = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_page_url() {
        let page = MockPage::new();
        assert_eq!(page.url(), "about:blank");

        page.set_url("https://example.com");
        assert_eq!(page.url(), "https://example.com");
    }

    #[tokio::test]
    async fn mock_page_locator_text() {
        let page = MockPage::new();
        page.set_text_for_selector("h1", "Hello World");

        let locator = page.locator("h1");
        assert_eq!(locator.count().await.unwrap(), 1);
        assert_eq!(
            locator.text_content().await.unwrap(),
            Some("Hello World".to_string())
        );
    }

    #[tokio::test]
    async fn mock_page_locator_not_found() {
        let page = MockPage::new();

        let locator = page.locator(".missing");
        assert_eq!(locator.count().await.unwrap(), 0);
        assert_eq!(locator.text_content().await.unwrap(), None);
    }

    #[tokio::test]
    async fn mock_page_records_actions() {
        let page = MockPage::new();
        page.set_count_for_selector("button", 1);

        let locator = page.locator("button");
        locator.click().await.unwrap();

        let actions = page.actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            MockAction::Click {
                selector: "button".to_string()
            }
        );
    }

    #[tokio::test]
    async fn mock_session_navigation() {
        let session = MockSession::default_session();
        assert_eq!(session.page().url(), "about:blank");

        session.goto("https://example.com").await.unwrap();
        assert_eq!(session.page().url(), "https://example.com");

        assert!(!session.goto_if_needed("https://example.com").await.unwrap());
        assert!(session.goto_if_needed("https://other.com").await.unwrap());
    }

    #[tokio::test]
    async fn mock_session_current_page_sentinel() {
        let session = MockSession::default_session();
        session.goto("https://example.com").await.unwrap();

        assert!(
            !session
                .goto_unless_current("__CURRENT_PAGE__")
                .await
                .unwrap()
        );
        assert_eq!(session.page().url(), "https://example.com");
    }

    #[tokio::test]
    async fn mock_page_eval() {
        let page = MockPage::new();
        page.set_url("https://example.com");
        page.set_eval_result("document.title", serde_json::json!("Test Page"));

        assert_eq!(
            page.evaluate_value("window.location.href").await.unwrap(),
            "https://example.com"
        );
        assert_eq!(
            page.evaluate_value("document.title").await.unwrap(),
            "Test Page"
        );
    }

    #[tokio::test]
    async fn mock_page_bounding_box() {
        let page = MockPage::new();
        page.set_bbox_for_selector(
            "#button",
            BoundingBox {
                x: 100.0,
                y: 200.0,
                width: 50.0,
                height: 30.0,
            },
        );

        let locator = page.locator("#button");
        let bbox = locator.bounding_box().await.unwrap().unwrap();
        assert_eq!(bbox.x, 100.0);
        assert_eq!(bbox.y, 200.0);
        assert_eq!(bbox.width, 50.0);
        assert_eq!(bbox.height, 30.0);
    }
}
