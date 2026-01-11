// Error types for playwright-core

use thiserror::Error;

/// Result type alias for playwright-core operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur when using playwright-core
#[derive(Debug, Error)]
pub enum Error {
    /// Playwright server binary was not found
    ///
    /// The Playwright Node.js driver could not be located.
    /// To resolve this, install Playwright using: `npm install playwright`
    /// Or ensure the PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD environment variable is not set.
    #[error("Playwright server not found. Install with: npm install playwright")]
    ServerNotFound,

    /// Failed to launch the Playwright server process
    ///
    /// The Playwright server process could not be started.
    /// Common causes: Node.js not installed, insufficient permissions, or port already in use.
    /// Details: {0}
    #[error("Failed to launch Playwright server: {0}. Check that Node.js is installed.")]
    LaunchFailed(String),

    /// Server error (runtime issue with Playwright server)
    #[error("Server error: {0}")]
    ServerError(String),

    /// Failed to establish connection with the server
    #[error("Failed to connect to Playwright server: {0}")]
    ConnectionFailed(String),

    /// Transport-level error (stdio communication)
    #[error("Transport error: {0}")]
    TransportError(String),

    /// Protocol-level error (JSON-RPC)
    ///
    /// Generic protocol error - prefer using `Remote` for errors from the Playwright server
    /// as it preserves full error context including name and stack trace.
    #[error("Protocol error: {0}")]
    ProtocolError(String),

    /// Remote Playwright server error with full context
    ///
    /// Preserves the complete error information from the Playwright server:
    /// - `name`: Error type (e.g., "TimeoutError", "Error")
    /// - `message`: Human-readable error description
    /// - `stack`: JavaScript stack trace (when available)
    ///
    /// This variant should be preferred over `ProtocolError` when handling
    /// errors from the Playwright server as it enables better debugging.
    #[error("{name}: {message}")]
    Remote {
        /// Error type name (e.g., "TimeoutError", "Error", "TargetClosedError")
        name: String,
        /// Human-readable error message
        message: String,
        /// JavaScript stack trace from the server (if available)
        stack: Option<String>,
    },

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Timeout waiting for operation
    ///
    /// Contains context about what operation timed out and the timeout duration.
    /// Common causes include slow network, server not responding, or element not becoming actionable.
    /// Consider increasing the timeout or checking if the target is accessible.
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Navigation timeout
    ///
    /// Occurs when page navigation exceeds the specified timeout.
    /// Includes the URL being navigated to and timeout duration.
    #[error("Navigation timeout after {duration_ms}ms navigating to '{url}'")]
    NavigationTimeout { url: String, duration_ms: u64 },

    /// Target was closed (browser, context, or page)
    ///
    /// Occurs when attempting to perform an operation on a closed target.
    /// The target must be recreated before it can be used again.
    #[error("Target closed: Cannot perform operation on closed {target_type}. {context}")]
    TargetClosed {
        target_type: String,
        context: String,
    },

    /// Object not found in the connection registry
    ///
    /// Occurs when looking up an object by GUID that doesn't exist in the registry.
    /// This can happen if:
    /// - The object was disposed/garbage collected
    /// - The GUID is invalid or refers to an object that was never created
    /// - There's a race condition between object creation and lookup
    #[error("Object not found: {guid}{}", expected.map(|t| format!(" (expected {})", t)).unwrap_or_default())]
    ObjectNotFound {
        /// The GUID that was looked up
        guid: String,
        /// Expected object type (if known from GUID prefix)
        expected: Option<&'static str>,
    },

    /// Unknown protocol object type
    #[error("Unknown protocol object type: {0}")]
    UnknownObjectType(String),

    /// Channel closed unexpectedly
    #[error("Channel closed unexpectedly")]
    ChannelClosed,

    /// Invalid argument provided to method
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// Element not found by selector
    ///
    /// Includes the selector that was used to locate the element.
    /// This error typically occurs when waiting for an element times out.
    #[error("Element not found: selector '{0}'")]
    ElementNotFound(String),

    /// Assertion timeout (expect API)
    #[error("Assertion timeout: {0}")]
    AssertionTimeout(String),
}

impl Error {
    /// Returns the error name if this is a Remote error
    pub fn error_name(&self) -> Option<&str> {
        match self {
            Error::Remote { name, .. } => Some(name),
            _ => None,
        }
    }

    /// Returns the stack trace if this is a Remote error with a stack
    pub fn stack_trace(&self) -> Option<&str> {
        match self {
            Error::Remote { stack, .. } => stack.as_deref(),
            _ => None,
        }
    }

    /// Returns true if this is a timeout error
    pub fn is_timeout(&self) -> bool {
        match self {
            Error::Timeout(_) | Error::NavigationTimeout { .. } | Error::AssertionTimeout(_) => {
                true
            }
            Error::Remote { name, .. } => name == "TimeoutError",
            _ => false,
        }
    }

    /// Returns true if this is a target closed error
    pub fn is_target_closed(&self) -> bool {
        match self {
            Error::TargetClosed { .. } => true,
            Error::Remote { name, .. } => name == "TargetClosedError",
            _ => false,
        }
    }
}
