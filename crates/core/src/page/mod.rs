//! [`Page`] protocol object representing a browser tab.

mod eval;
mod input;
mod page_events;
mod routing;
mod screenshot;

use std::sync::{Arc, RwLock};

use indexmap::IndexMap;
use parking_lot::Mutex;
use pw_runtime::channel::Channel;
use pw_runtime::channel_owner::{ChannelOwner, ChannelOwnerImpl, ParentOrConnection};
use pw_runtime::{Error, Result};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::broadcast;

pub use crate::handlers::Subscription;
use crate::handlers::{HandlerMap, RouteMeta};
use crate::{Dialog, Download, Route};

/// A browser tab or window within a [`BrowserContext`](crate::BrowserContext).
///
/// See <https://playwright.dev/docs/api/class-page>
#[derive(Clone)]
pub struct Page {
	base: ChannelOwnerImpl,
	/// Current URL of the page (wrapped in RwLock for event updates).
	url: Arc<RwLock<String>>,
	/// GUID of the main frame.
	main_frame_guid: Arc<str>,
	/// Route handlers for network interception (with compiled matchers).
	route_handlers: HandlerMap<Route, RouteMeta>,
	/// Download event handlers.
	download_handlers: HandlerMap<Download>,
	/// Dialog event handlers.
	dialog_handlers: HandlerMap<Dialog>,
	/// Console message broadcast channel.
	console_tx: broadcast::Sender<ConsoleMessage>,
}

/// Console message from JavaScript `console.*` calls.
///
/// See <https://playwright.dev/docs/api/class-consolemessage>
#[derive(Debug, Clone)]
pub struct ConsoleMessage {
	/// The type of console message (log, error, warning, etc.).
	kind: ConsoleMessageKind,
	/// The text content of the message.
	text: String,
	/// Source location where the message was logged.
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

impl Page {
	/// Creates a new Page from protocol initialization.
	pub fn new(parent: Arc<dyn ChannelOwner>, type_name: String, guid: Arc<str>, initializer: Value) -> Result<Self> {
		let main_frame_guid: Arc<str> = Arc::from(
			initializer["mainFrame"]["guid"]
				.as_str()
				.ok_or_else(|| pw_runtime::Error::ProtocolError("Page initializer missing 'mainFrame.guid' field".to_string()))?,
		);

		let base = ChannelOwnerImpl::new(ParentOrConnection::Parent(parent), type_name, guid, initializer);

		let url = Arc::new(RwLock::new("about:blank".to_string()));
		let route_handlers = Arc::new(Mutex::new(IndexMap::new()));
		let download_handlers = Arc::new(Mutex::new(IndexMap::new()));
		let dialog_handlers = Arc::new(Mutex::new(IndexMap::new()));
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

	pub(crate) fn channel(&self) -> &Channel {
		self.base.channel()
	}

	pub(crate) async fn main_frame(&self) -> Result<crate::Frame> {
		let frame_arc = self.connection().get_object(&self.main_frame_guid).await?;

		let frame = frame_arc
			.downcast_ref::<crate::Frame>()
			.ok_or_else(|| pw_runtime::Error::ProtocolError(format!("Expected Frame object, got {}", frame_arc.type_name())))?;

		Ok(frame.clone())
	}

	/// Returns the current URL (initially "about:blank").
	///
	/// See <https://playwright.dev/docs/api/class-page#page-url>
	pub fn url(&self) -> String {
		self.url.read().unwrap_or_else(|e| e.into_inner()).clone()
	}

	/// Closes the page.
	///
	/// See <https://playwright.dev/docs/api/class-page#page-close>
	pub async fn close(&self) -> Result<()> {
		self.channel().send_no_result("close", serde_json::json!({})).await
	}

	/// Brings the page to the front (activates the tab).
	///
	/// See <https://playwright.dev/docs/api/class-page#page-bring-to-front>
	pub async fn bring_to_front(&self) -> Result<()> {
		self.channel().send_no_result("bringToFront", serde_json::json!({})).await
	}

	/// Navigates to the specified URL.
	///
	/// Returns `None` for URLs without responses (data URLs, about:blank).
	///
	/// See <https://playwright.dev/docs/api/class-page#page-goto>
	pub async fn goto(&self, url: &str, options: Option<GotoOptions>) -> Result<Option<Response>> {
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
	/// See <https://playwright.dev/docs/api/class-page#page-title>
	pub async fn title(&self) -> Result<String> {
		let frame = self.main_frame().await?;
		frame.title().await
	}

	/// Creates a locator for finding elements.
	///
	/// See <https://playwright.dev/docs/api/class-page#page-locator>
	pub async fn locator(&self, selector: &str) -> crate::Locator {
		let frame = self.main_frame().await.expect("Main frame should exist");

		crate::Locator::new(Arc::new(frame), selector.to_string())
	}

	/// Returns the keyboard for low-level control.
	///
	/// See <https://playwright.dev/docs/api/class-page#page-keyboard>
	pub fn keyboard(&self) -> crate::Keyboard {
		crate::Keyboard::new(self.clone())
	}

	/// Returns the mouse for low-level control.
	///
	/// See <https://playwright.dev/docs/api/class-page#page-mouse>
	pub fn mouse(&self) -> crate::Mouse {
		crate::Mouse::new(self.clone())
	}

	/// Returns the accessibility handle for inspecting the accessibility tree.
	///
	/// See <https://playwright.dev/docs/api/class-page#page-accessibility>
	pub fn accessibility(&self) -> crate::Accessibility {
		crate::Accessibility::new(self.clone())
	}

	/// Returns the video handle if recording is enabled, or `None`.
	///
	/// See <https://playwright.dev/docs/api/class-page#page-video>
	pub fn video(&self) -> Option<crate::Video> {
		let video_guid = self.base.initializer().get("video").and_then(|v| v.get("guid")).and_then(|v| v.as_str())?;

		self.base
			.children()
			.into_iter()
			.find(|child| child.guid() == video_guid)
			.and_then(|child| child.downcast_ref::<crate::Video>().cloned())
	}

	/// Reloads the current page.
	///
	/// Returns `None` for URLs without responses (data URLs, about:blank).
	///
	/// See <https://playwright.dev/docs/api/class-page#page-reload>
	pub async fn reload(&self, options: Option<GotoOptions>) -> Result<Option<Response>> {
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
			let response_arc = self.connection().wait_for_object(&response_ref.guid, std::time::Duration::from_secs(1)).await?;

			let initializer = response_arc.initializer();

			let status = initializer["status"]
				.as_u64()
				.ok_or_else(|| pw_runtime::Error::ProtocolError("Response missing status".to_string()))? as u16;

			let headers = initializer["headers"]
				.as_array()
				.ok_or_else(|| pw_runtime::Error::ProtocolError("Response missing headers".to_string()))?
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
					.ok_or_else(|| pw_runtime::Error::ProtocolError("Response missing url".to_string()))?
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

	/// Returns the first element matching the selector, or `None`.
	///
	/// See <https://playwright.dev/docs/api/class-page#page-query-selector>
	pub async fn query_selector(&self, selector: &str) -> Result<Option<Arc<crate::ElementHandle>>> {
		let frame = self.main_frame().await?;
		frame.query_selector(selector).await
	}

	/// Returns all elements matching the selector.
	///
	/// See <https://playwright.dev/docs/api/class-page#page-query-selector-all>
	pub async fn query_selector_all(&self, selector: &str) -> Result<Vec<Arc<crate::ElementHandle>>> {
		let frame = self.main_frame().await?;
		frame.query_selector_all(selector).await
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
				if let Some(url_str) = params.get("url").and_then(|v| v.as_str()) {
					if let Ok(mut url) = self.url.write() {
						*url = url_str.to_string();
					}
				}
			}
			"route" => {
				let Some(route_guid) = params.get("route").and_then(|v| v.get("guid")).and_then(|v| v.as_str()) else {
					return;
				};

				let connection = self.connection();
				let route_guid_owned = route_guid.to_string();
				let self_clone = self.clone();

				tokio::spawn(async move {
					let Ok(route_arc) = connection.get_object(&route_guid_owned).await else {
						tracing::error!(guid = %route_guid_owned, "Failed to get route object");
						return;
					};

					let Some(route) = route_arc.downcast_ref::<Route>().cloned() else {
						tracing::error!(guid = %route_guid_owned, "Failed to downcast to Route");
						return;
					};

					self_clone.on_route_event(route).await;
				});
			}
			"download" => {
				let url = params.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();

				let suggested_filename = params.get("suggestedFilename").and_then(|v| v.as_str()).unwrap_or("").to_string();

				let Some(artifact_guid) = params.get("artifact").and_then(|v| v.get("guid")).and_then(|v| v.as_str()) else {
					return;
				};

				let connection = self.connection();
				let artifact_guid_owned = artifact_guid.to_string();
				let self_clone = self.clone();

				tokio::spawn(async move {
					let Ok(artifact_arc) = connection.get_object(&artifact_guid_owned).await else {
						tracing::error!(guid = %artifact_guid_owned, "Failed to get artifact object");
						return;
					};

					let download = Download::from_artifact(artifact_arc, url, suggested_filename);
					self_clone.on_download_event(download).await;
				});
			}
			"dialog" => {}
			"console" => {
				let Some(message_obj) = params.get("message") else {
					return;
				};

				let kind = message_obj
					.get("type")
					.and_then(|v| v.as_str())
					.map(ConsoleMessageKind::from_str)
					.unwrap_or(ConsoleMessageKind::Log);

				let text = message_obj.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();

				let location = message_obj.get("location").and_then(|loc| {
					Some(ConsoleLocation {
						url: loc.get("url")?.as_str()?.to_string(),
						line_number: loc.get("lineNumber")?.as_u64()? as u32,
						column_number: loc.get("columnNumber")?.as_u64()? as u32,
					})
				});

				let _ = self.console_tx.send(ConsoleMessage { kind, text, location });
			}
			_ => {}
		}
	}

	fn was_collected(&self) -> bool {
		self.base.was_collected()
	}
}

impl std::fmt::Debug for Page {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Page").field("guid", &self.guid()).field("url", &self.url()).finish()
	}
}

/// Options for [`Page::goto`] and [`Page::reload`].
#[derive(Debug, Clone, Default)]
pub struct GotoOptions {
	/// Maximum operation time.
	pub timeout: Option<std::time::Duration>,
	/// When to consider the operation succeeded.
	pub wait_until: Option<WaitUntil>,
}

impl GotoOptions {
	/// Creates new GotoOptions with default values.
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets the timeout.
	pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
		self.timeout = Some(timeout);
		self
	}

	/// Sets the wait_until option.
	pub fn wait_until(mut self, wait_until: WaitUntil) -> Self {
		self.wait_until = Some(wait_until);
		self
	}
}

/// When to consider navigation succeeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitUntil {
	/// `load` event fired.
	Load,
	/// `DOMContentLoaded` event fired.
	DomContentLoaded,
	/// No network connections for 500ms.
	NetworkIdle,
	/// Navigation committed.
	Commit,
}

impl WaitUntil {
	pub(crate) fn as_str(&self) -> &'static str {
		match self {
			Self::Load => "load",
			Self::DomContentLoaded => "domcontentloaded",
			Self::NetworkIdle => "networkidle",
			Self::Commit => "commit",
		}
	}
}

/// Response from navigation operations.
#[derive(Debug, Clone)]
pub struct Response {
	/// URL of the response.
	pub url: String,
	/// HTTP status code.
	pub status: u16,
	/// HTTP status text.
	pub status_text: String,
	/// Whether the response was successful (status 200-299).
	pub ok: bool,
	/// Response headers.
	pub headers: std::collections::HashMap<String, String>,
}

impl Response {
	/// Returns the URL of the response.
	pub fn url(&self) -> &str {
		&self.url
	}

	/// Returns the HTTP status code.
	pub fn status(&self) -> u16 {
		self.status
	}

	/// Returns the HTTP status text.
	pub fn status_text(&self) -> &str {
		&self.status_text
	}

	/// Returns whether the response was successful (status 200-299).
	pub fn ok(&self) -> bool {
		self.ok
	}

	/// Returns the response headers.
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
		assert_eq!(ConsoleMessageKind::from_str("error"), ConsoleMessageKind::Error);
		assert_eq!(ConsoleMessageKind::from_str("warning"), ConsoleMessageKind::Warning);
		assert_eq!(ConsoleMessageKind::from_str("info"), ConsoleMessageKind::Info);
		assert_eq!(ConsoleMessageKind::from_str("debug"), ConsoleMessageKind::Debug);
		assert_eq!(ConsoleMessageKind::from_str("unknown"), ConsoleMessageKind::Other);
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
