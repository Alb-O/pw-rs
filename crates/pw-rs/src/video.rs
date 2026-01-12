// Copyright 2024 Paul Adamson
// Licensed under the Apache License, Version 2.0

//! Video recording API for capturing page sessions.
//!
//! Videos are recorded when [`BrowserContextOptions::record_video_dir`] is set.
//! Each page in the context will have video automatically recorded.
//!
//! # Example
//!
//! ```ignore
//! use pw::protocol::{BrowserContextOptions, Viewport};
//!
//! // Create context with video recording enabled
//! let options = BrowserContextOptions::builder()
//!     .record_video_dir("/tmp/videos")
//!     .record_video_size(Viewport { width: 1280, height: 720 })
//!     .build();
//!
//! let context = browser.new_context_with_options(options).await?;
//! let page = context.new_page().await?;
//!
//! // Navigate and perform actions - video is recording
//! page.goto("https://example.com", None).await?;
//!
//! // Get the video path after page closes
//! let video = page.video().expect("video should exist");
//! page.close().await?;
//!
//! let path = video.path().await?;
//! println!("Video saved to: {}", path.display());
//! ```
//!
//! See: <https://playwright.dev/docs/api/class-video>

use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use pw_runtime::Result;
use pw_runtime::channel::Channel;
use pw_runtime::channel_owner::{ChannelOwner, ChannelOwnerImpl, ParentOrConnection};

/// Handle for a recorded video file.
///
/// Videos are automatically recorded when video recording is enabled on the
/// browser context. The video file is saved when the page closes.
///
/// # Example
///
/// ```ignore
/// let video = page.video().expect("video recording enabled");
///
/// // Close page to finish recording
/// page.close().await?;
///
/// // Get path to the recorded video
/// let path = video.path().await?;
/// ```
///
/// See: <https://playwright.dev/docs/api/class-video>
#[derive(Clone)]
pub struct Video {
    base: ChannelOwnerImpl,
}

impl Video {
    /// Creates a new Video from protocol initialization.
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

    /// Returns the path to the video file.
    ///
    /// The video is not guaranteed to be fully written until the page has
    /// closed. Call this after [`Page::close()`] completes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProtocolError`] if video recording failed or
    /// the page hasn't been closed yet.
    ///
    /// [`Page::close()`]: crate::Page::close
    /// [`Error::ProtocolError`]: pw_runtime::Error::ProtocolError
    ///
    /// See: <https://playwright.dev/docs/api/class-video#video-path>
    pub async fn path(&self) -> Result<PathBuf> {
        #[derive(Deserialize)]
        struct PathResponse {
            value: String,
        }

        let response: PathResponse = self.channel().send("path", serde_json::json!({})).await?;
        Ok(PathBuf::from(response.value))
    }

    /// Saves the video to the specified `path`.
    ///
    /// If the video is still being recorded, waits for recording to finish
    /// before saving. The video will be copied to the destination.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProtocolError`] if video recording failed, or
    /// [`Error::IoError`] if the file cannot be written.
    ///
    /// [`Error::ProtocolError`]: pw_runtime::Error::ProtocolError
    /// [`Error::IoError`]: pw_runtime::Error::IoError
    ///
    /// See: <https://playwright.dev/docs/api/class-video#video-save-as>
    pub async fn save_as(&self, path: impl Into<PathBuf>) -> Result<()> {
        let path: PathBuf = path.into();
        self.channel()
            .send_no_result(
                "saveAs",
                serde_json::json!({
                    "path": path.to_string_lossy()
                }),
            )
            .await
    }

    /// Deletes the video file.
    ///
    /// This will wait for the video to finish recording if necessary, then
    /// delete the file from disk.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProtocolError`] if video recording failed, or
    /// [`Error::IoError`] if the file cannot be deleted.
    ///
    /// [`Error::ProtocolError`]: pw_runtime::Error::ProtocolError
    /// [`Error::IoError`]: pw_runtime::Error::IoError
    ///
    /// See: <https://playwright.dev/docs/api/class-video#video-delete>
    pub async fn delete(&self) -> Result<()> {
        self.channel()
            .send_no_result("delete", serde_json::json!({}))
            .await
    }
}

impl pw_runtime::channel_owner::private::Sealed for Video {}

impl ChannelOwner for Video {
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

impl std::fmt::Debug for Video {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Video").field("guid", &self.guid()).finish()
    }
}
