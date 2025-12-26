use std::path::PathBuf;

use thiserror::Error;

use crate::output::{CommandError, ErrorCode};

pub type Result<T> = std::result::Result<T, PwError>;

#[derive(Debug, Error)]
pub enum PwError {
    /// Command failed but output has already been printed (e.g., with artifacts).
    /// Used to signal exit code 1 without additional output.
    #[error("")]
    OutputAlreadyPrinted,

    #[error("initialization failed: {0}")]
    Init(String),

    #[error("browser launch failed: {0}")]
    BrowserLaunch(String),

    #[error("navigation failed: {url}")]
    Navigation {
        url: String,
        #[source]
        source: anyhow::Error,
    },

    #[error("element not found: {selector}")]
    ElementNotFound { selector: String },

    #[error("javascript evaluation failed: {0}")]
    JsEval(String),

    #[error("screenshot failed: {path}")]
    Screenshot {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("timeout after {ms}ms waiting for: {condition}")]
    Timeout { ms: u64, condition: String },

    #[error("context resolution failed: {0}")]
    Context(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Playwright(#[from] pw::Error),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

impl PwError {
    /// Check if this error indicates output has already been printed.
    /// When true, the caller should exit with code 1 without printing additional output.
    pub fn is_output_already_printed(&self) -> bool {
        matches!(self, PwError::OutputAlreadyPrinted)
    }

    /// Convert this error to a CommandError for structured output
    pub fn to_command_error(&self) -> CommandError {
        let (code, message, details) = match self {
            PwError::OutputAlreadyPrinted => {
                // This should never be called - caller should check is_output_already_printed()
                (ErrorCode::InternalError, String::new(), None)
            }
            PwError::Init(msg) => (ErrorCode::BrowserLaunchFailed, msg.clone(), None),
            PwError::BrowserLaunch(msg) => (ErrorCode::BrowserLaunchFailed, msg.clone(), None),
            PwError::Navigation { url, source } => (
                ErrorCode::NavigationFailed,
                format!("Navigation to {url} failed: {source}"),
                Some(serde_json::json!({ "url": url })),
            ),
            PwError::ElementNotFound { selector } => (
                ErrorCode::SelectorNotFound,
                format!("No elements matched selector: {selector}"),
                Some(serde_json::json!({ "selector": selector })),
            ),
            PwError::JsEval(msg) => (
                ErrorCode::JsEvalFailed,
                msg.clone(),
                None,
            ),
            PwError::Screenshot { path, source } => (
                ErrorCode::ScreenshotFailed,
                format!("Screenshot failed at {}: {source}", path.display()),
                Some(serde_json::json!({ "path": path })),
            ),
            PwError::Timeout { ms, condition } => (
                ErrorCode::Timeout,
                format!("Timeout after {ms}ms waiting for: {condition}"),
                Some(serde_json::json!({ "timeout_ms": ms, "condition": condition })),
            ),
            PwError::Context(msg) => (ErrorCode::InvalidInput, msg.clone(), None),
            PwError::Io(err) => (
                ErrorCode::IoError,
                err.to_string(),
                None,
            ),
            PwError::Json(err) => (
                ErrorCode::InternalError,
                format!("JSON error: {err}"),
                None,
            ),
            PwError::Playwright(err) => {
                // Map Playwright errors to appropriate codes
                let msg = err.to_string();
                let code = if msg.contains("Timeout") {
                    ErrorCode::Timeout
                } else if msg.contains("not found") || msg.contains("no element") {
                    ErrorCode::SelectorNotFound
                } else if msg.contains("navigation") {
                    ErrorCode::NavigationFailed
                } else {
                    ErrorCode::InternalError
                };
                (code, msg, None)
            }
            PwError::Anyhow(err) => (
                ErrorCode::InternalError,
                err.to_string(),
                None,
            ),
        };

        CommandError {
            code,
            message,
            details,
        }
    }
}
