// Copyright 2024 Paul Adamson
// Licensed under the Apache License, Version 2.0

//! Playwright tracing API for recording browser sessions.
//!
//! Tracing captures a trace of browser operations that can be viewed in the
//! [Playwright Trace Viewer](https://playwright.dev/docs/trace-viewer). This is
//! invaluable for debugging test failures.
//!
//! # Example
//!
//! ```ignore
//! // Start tracing before performing actions
//! context.tracing().unwrap().start(TracingStartOptions {
//!     screenshots: Some(true),
//!     snapshots: Some(true),
//!     ..Default::default()
//! }).await?;
//!
//! // Perform browser operations
//! page.goto("https://example.com", None).await?;
//! page.click("button", None).await?;
//!
//! // Stop and save the trace
//! context.tracing().stop(TracingStopOptions {
//!     path: Some("trace.zip".into()),
//! }).await?;
//!
//! // View with: npx playwright show-trace trace.zip
//! ```
//!
//! See: <https://playwright.dev/docs/api/class-tracing>

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use pw_runtime::Result;
use pw_runtime::channel::Channel;
use pw_runtime::channel_owner::{ChannelOwner, ChannelOwnerImpl, ParentOrConnection};

/// Handle for managing Playwright traces on a [`BrowserContext`].
///
/// Tracing records browser operations including screenshots, DOM snapshots,
/// and source code locations. The resulting trace file can be opened with
/// Playwright's trace viewer for debugging.
///
/// Obtain a [`Tracing`] handle via [`BrowserContext::tracing()`].
///
/// [`BrowserContext`]: crate::BrowserContext
/// [`BrowserContext::tracing()`]: crate::BrowserContext::tracing
#[derive(Clone)]
pub struct Tracing {
    base: ChannelOwnerImpl,
}

impl Tracing {
    /// Creates a new Tracing from protocol initialization.
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
        Ok(Self { base })
    }

    fn channel(&self) -> &Channel {
        self.base.channel()
    }

    /// Starts recording a trace.
    ///
    /// Only one trace can be recording at a time per context. Call [`stop`](Self::stop)
    /// to finish recording and save the trace.
    ///
    /// # Arguments
    ///
    /// * `options` - Configuration for what to capture in the trace
    ///
    /// # Example
    ///
    /// ```ignore
    /// context.tracing().unwrap().start(TracingStartOptions {
    ///     screenshots: Some(true),
    ///     snapshots: Some(true),
    ///     sources: Some(true),
    ///     title: Some("My Test".into()),
    /// }).await?;
    /// ```
    ///
    /// # Errors
    ///
    /// - [`Error::ProtocolError`] if tracing is already active
    /// - [`Error::ProtocolError`] if the context has been closed
    ///
    /// [`Error::ProtocolError`]: pw_runtime::Error::ProtocolError
    ///
    /// See: <https://playwright.dev/docs/api/class-tracing#tracing-start>
    pub async fn start(&self, options: TracingStartOptions) -> Result<()> {
        let params = serde_json::to_value(&options).unwrap_or_default();
        self.channel().send_no_result("tracingStart", params).await
    }

    /// Starts a new trace chunk.
    ///
    /// If a trace is already recording, this creates a new chunk that can be
    /// saved separately. Useful for splitting long sessions into multiple files.
    ///
    /// # Arguments
    ///
    /// * `options` - Optional title for the new chunk
    ///
    /// # Errors
    ///
    /// - [`Error::ProtocolError`] if no trace is currently active
    ///
    /// [`Error::ProtocolError`]: pw_runtime::Error::ProtocolError
    ///
    /// See: <https://playwright.dev/docs/api/class-tracing#tracing-start-chunk>
    pub async fn start_chunk(&self, options: Option<TracingStartChunkOptions>) -> Result<()> {
        let params = options
            .map(|o| serde_json::to_value(&o).unwrap_or_default())
            .unwrap_or_else(|| serde_json::json!({}));
        self.channel()
            .send_no_result("tracingStartChunk", params)
            .await
    }

    /// Stops recording the trace and optionally saves it.
    ///
    /// If `path` is provided in options, the trace is saved to that file.
    /// The trace can be viewed with `npx playwright show-trace <path>`.
    ///
    /// # Arguments
    ///
    /// * `options` - Where to save the trace file
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Stop and save to file
    /// context.tracing().stop(TracingStopOptions {
    ///     path: Some("trace.zip".into()),
    /// }).await?;
    ///
    /// // Stop without saving
    /// context.tracing().stop(TracingStopOptions::default()).await?;
    /// ```
    ///
    /// # Errors
    ///
    /// - [`Error::ProtocolError`] if no trace is currently active
    /// - [`Error::IoError`] if the trace file cannot be written
    ///
    /// [`Error::ProtocolError`]: pw_runtime::Error::ProtocolError
    /// [`Error::IoError`]: pw_runtime::Error::IoError
    ///
    /// See: <https://playwright.dev/docs/api/class-tracing#tracing-stop>
    pub async fn stop(&self, options: TracingStopOptions) -> Result<()> {
        let params = serde_json::to_value(&options).unwrap_or_default();
        self.channel().send_no_result("tracingStop", params).await
    }

    /// Stops the current trace chunk and optionally saves it.
    ///
    /// Use this after [`start_chunk`](Self::start_chunk) to save intermediate
    /// trace segments without stopping the entire trace.
    ///
    /// # Errors
    ///
    /// - [`Error::ProtocolError`] if no chunk is currently active
    ///
    /// [`Error::ProtocolError`]: pw_runtime::Error::ProtocolError
    ///
    /// See: <https://playwright.dev/docs/api/class-tracing#tracing-stop-chunk>
    pub async fn stop_chunk(&self, options: TracingStopOptions) -> Result<()> {
        let params = serde_json::to_value(&options).unwrap_or_default();
        self.channel()
            .send_no_result("tracingStopChunk", params)
            .await
    }
}

impl pw_runtime::channel_owner::private::Sealed for Tracing {}

impl ChannelOwner for Tracing {
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

    fn on_event(&self, _method: &str, _params: Value) {}

    fn was_collected(&self) -> bool {
        self.base.was_collected()
    }
}

impl std::fmt::Debug for Tracing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tracing")
            .field("guid", &self.guid())
            .finish()
    }
}

/// Options for starting a trace recording.
///
/// See: <https://playwright.dev/docs/api/class-tracing#tracing-start>
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracingStartOptions {
    /// Whether to capture screenshots during tracing.
    ///
    /// Screenshots are used to build a timeline preview in the trace viewer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshots: Option<bool>,

    /// Whether to capture DOM snapshots for each action.
    ///
    /// Snapshots allow inspecting the page state at each step in the trace viewer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshots: Option<bool>,

    /// Whether to include source files for actions.
    ///
    /// When enabled, clicking an action in the trace viewer shows the source code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<bool>,

    /// Trace name displayed in the trace viewer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

impl TracingStartOptions {
    /// Creates a new builder for configuring trace start options.
    pub fn builder() -> TracingStartOptionsBuilder {
        TracingStartOptionsBuilder::default()
    }
}

/// Builder for [`TracingStartOptions`].
#[derive(Debug, Clone, Default)]
pub struct TracingStartOptionsBuilder {
    screenshots: Option<bool>,
    snapshots: Option<bool>,
    sources: Option<bool>,
    title: Option<String>,
}

impl TracingStartOptionsBuilder {
    /// Enables or disables screenshot capture.
    pub fn screenshots(mut self, value: bool) -> Self {
        self.screenshots = Some(value);
        self
    }

    /// Enables or disables DOM snapshots.
    pub fn snapshots(mut self, value: bool) -> Self {
        self.snapshots = Some(value);
        self
    }

    /// Enables or disables source file inclusion.
    pub fn sources(mut self, value: bool) -> Self {
        self.sources = Some(value);
        self
    }

    /// Sets the trace title.
    pub fn title(mut self, value: impl Into<String>) -> Self {
        self.title = Some(value.into());
        self
    }

    /// Builds the options.
    pub fn build(self) -> TracingStartOptions {
        TracingStartOptions {
            screenshots: self.screenshots,
            snapshots: self.snapshots,
            sources: self.sources,
            title: self.title,
        }
    }
}

/// Options for starting a new trace chunk.
///
/// See: <https://playwright.dev/docs/api/class-tracing#tracing-start-chunk>
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracingStartChunkOptions {
    /// Trace name displayed in the trace viewer for this chunk.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Trace name displayed in the trace viewer for this chunk.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Options for stopping trace recording.
///
/// See: <https://playwright.dev/docs/api/class-tracing#tracing-stop>
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracingStopOptions {
    /// Path to save the trace file.
    ///
    /// If not specified, the trace is discarded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

impl TracingStopOptions {
    /// Creates stop options that save to the given path.
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self {
            path: Some(path.into()),
        }
    }
}
