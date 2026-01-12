//! Core protocol types used across the wire.
//!
//! These types represent primitive values and enums used in the Playwright protocol.

use serde::{Deserialize, Serialize};

/// Mouse button for click actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    /// Left mouse button (default)
    Left,
    /// Right mouse button
    Right,
    /// Middle mouse button
    Middle,
}

/// Keyboard modifier keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyboardModifier {
    /// Alt key
    Alt,
    /// Control key
    Control,
    /// Meta key (Command on macOS, Windows key on Windows)
    Meta,
    /// Shift key
    Shift,
    /// Control on Windows/Linux, Meta on macOS
    ControlOrMeta,
}

/// Position for click actions.
///
/// Coordinates are relative to the top-left corner of the element's padding box.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    /// X coordinate
    pub x: f64,
    /// Y coordinate
    pub y: f64,
}

/// Screenshot image format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScreenshotType {
    /// PNG format (lossless, supports transparency)
    Png,
    /// JPEG format (lossy compression, smaller file size)
    Jpeg,
}

/// Clip region for screenshot.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScreenshotClip {
    /// X coordinate of clip region origin
    pub x: f64,
    /// Y coordinate of clip region origin
    pub y: f64,
    /// Width of clip region
    pub width: f64,
    /// Height of clip region
    pub height: f64,
}

/// Page load state for navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WaitUntil {
    /// Consider navigation finished after the `load` event fires
    #[default]
    Load,
    /// Consider navigation finished when the DOMContentLoaded event fires
    #[serde(rename = "domcontentloaded")]
    DomContentLoaded,
    /// Consider navigation finished when there are no network connections for at least 500ms
    #[serde(rename = "networkidle")]
    NetworkIdle,
    /// Consider navigation finished when document.readyState reaches 'complete'
    Commit,
}

/// Viewport dimensions for browser context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Viewport {
    /// Page width in pixels
    pub width: i32,
    /// Page height in pixels
    pub height: i32,
}

/// Geolocation coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Geolocation {
    /// Latitude between -90 and 90
    pub latitude: f64,
    /// Longitude between -180 and 180
    pub longitude: f64,
    /// Non-negative accuracy value (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accuracy: Option<f64>,
}

/// Select option variant.
///
/// Represents different ways to select an option in a `<select>` element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SelectOption {
    /// Select by option value attribute
    Value { value: String },
    /// Select by option label (visible text)
    Label { label: String },
    /// Select by option index (0-based)
    Index { index: usize },
}

impl SelectOption {
    /// Create a new value-based selection.
    pub fn value(v: impl Into<String>) -> Self {
        SelectOption::Value { value: v.into() }
    }

    /// Create a new label-based selection.
    pub fn label(l: impl Into<String>) -> Self {
        SelectOption::Label { label: l.into() }
    }

    /// Create a new index-based selection.
    pub fn index(i: usize) -> Self {
        SelectOption::Index { index: i }
    }
}

impl From<&str> for SelectOption {
    fn from(value: &str) -> Self {
        SelectOption::value(value)
    }
}

impl From<String> for SelectOption {
    fn from(value: String) -> Self {
        SelectOption::value(value)
    }
}

/// FilePayload represents a file for advanced file uploads.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePayload {
    /// File name
    pub name: String,
    /// MIME type
    pub mime_type: String,
    /// File contents as base64-encoded string
    pub buffer: String,
}

impl FilePayload {
    /// Creates a new FilePayload from raw bytes.
    pub fn new(name: impl Into<String>, mime_type: impl Into<String>, data: &[u8]) -> Self {
        use base64::Engine;
        Self {
            name: name.into(),
            mime_type: mime_type.into(),
            buffer: base64::engine::general_purpose::STANDARD.encode(data),
        }
    }
}

/// HAR content policy for recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HarContentPolicy {
    /// Attach content as base64-encoded data
    #[default]
    Attach,
    /// Embed content inline
    Embed,
    /// Omit content from HAR
    Omit,
}

/// HAR recording mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HarMode {
    /// Full recording mode
    #[default]
    Full,
    /// Minimal recording mode
    Minimal,
}

/// HAR not found behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HarNotFound {
    /// Abort on not found
    #[default]
    Abort,
    /// Fallback on not found
    Fallback,
}

/// Console message type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsoleMessageKind {
    /// console.log
    Log,
    /// console.debug
    Debug,
    /// console.info
    Info,
    /// console.warning
    Warning,
    /// console.error
    Error,
    /// console.dir
    Dir,
    /// console.dirxml
    #[serde(rename = "dirxml")]
    DirXml,
    /// console.table
    Table,
    /// console.trace
    Trace,
    /// console.clear
    Clear,
    /// console.group or console.groupCollapsed
    StartGroup,
    /// console.groupCollapsed
    StartGroupCollapsed,
    /// console.groupEnd
    EndGroup,
    /// console.assert
    Assert,
    /// console.profile
    Profile,
    /// console.profileEnd
    ProfileEnd,
    /// console.count
    Count,
    /// console.timeEnd
    TimeEnd,
}
