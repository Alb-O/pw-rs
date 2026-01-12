//! Error types for the Playwright runtime.

use thiserror::Error;

/// Result type alias for runtime operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in the Playwright runtime.
#[derive(Debug, Error)]
pub enum Error {
    /// Playwright server binary was not found.
    #[error("Playwright server not found. Install with: npm install playwright")]
    ServerNotFound,

    /// Failed to launch the Playwright server process.
    #[error("Failed to launch Playwright server: {0}. Check that Node.js is installed.")]
    LaunchFailed(String),

    /// Server error (runtime issue with Playwright server).
    #[error("Server error: {0}")]
    ServerError(String),

    /// Failed to establish connection with the server.
    #[error("Failed to connect to Playwright server: {0}")]
    ConnectionFailed(String),

    /// Transport-level error (stdio communication).
    #[error("Transport error: {0}")]
    TransportError(String),

    /// Protocol-level error (JSON-RPC).
    #[error("Protocol error: {0}")]
    ProtocolError(String),

    /// Remote Playwright server error with full context.
    #[error("{name}: {message}")]
    Remote {
        /// Error type name (e.g., "TimeoutError", "Error", "TargetClosedError")
        name: String,
        /// Human-readable error message
        message: String,
        /// JavaScript stack trace from the server (if available)
        stack: Option<String>,
    },

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Timeout waiting for operation.
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Navigation timeout.
    #[error("Navigation timeout after {duration_ms}ms navigating to '{url}'")]
    NavigationTimeout { url: String, duration_ms: u64 },

    /// Target was closed (browser, context, or page).
    #[error("Target closed: Cannot perform operation on closed {target_type}. {context}")]
    TargetClosed {
        target_type: String,
        context: String,
    },

    /// Object not found in the connection registry.
    #[error("Object not found: {guid}{}", expected.map(|t| format!(" (expected {})", t)).unwrap_or_default())]
    ObjectNotFound {
        guid: String,
        expected: Option<&'static str>,
    },

    /// Unknown protocol object type.
    #[error("Unknown protocol object type: {0}")]
    UnknownObjectType(String),

    /// Channel closed unexpectedly.
    #[error("Channel closed unexpectedly")]
    ChannelClosed,

    /// Invalid argument provided to method.
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// Element not found by selector.
    #[error("Element not found: selector '{0}'")]
    ElementNotFound(String),

    /// Assertion timeout (expect API).
    #[error("Assertion timeout: {0}")]
    AssertionTimeout(String),
}

impl Error {
    /// Returns the error name if this is a Remote error.
    pub fn error_name(&self) -> Option<&str> {
        match self {
            Error::Remote { name, .. } => Some(name),
            _ => None,
        }
    }

    /// Returns the stack trace if this is a Remote error with a stack.
    pub fn stack_trace(&self) -> Option<&str> {
        match self {
            Error::Remote { stack, .. } => stack.as_deref(),
            _ => None,
        }
    }

    /// Returns true if this is a timeout error.
    pub fn is_timeout(&self) -> bool {
        match self {
            Error::Timeout(_) | Error::NavigationTimeout { .. } | Error::AssertionTimeout(_) => {
                true
            }
            Error::Remote { name, .. } => name == "TimeoutError",
            _ => false,
        }
    }

    /// Returns true if this is a target closed error.
    pub fn is_target_closed(&self) -> bool {
        match self {
            Error::TargetClosed { .. } => true,
            Error::Remote { name, .. } => name == "TargetClosedError",
            _ => false,
        }
    }
}
