use std::path::PathBuf;

use thiserror::Error;

use crate::args::looks_like_selector;
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

/// Classify a Playwright error and clean up verbose messages.
///
/// Playwright's strict mode violations dump every matching element, which is
/// overwhelming. This extracts the useful info (selector, count) into a concise message.
fn classify_and_clean_playwright_error(msg: &str) -> (ErrorCode, String) {
    // Handle strict mode violations - extremely verbose, need cleanup
    if msg.contains("strict mode violation") {
        if let Some(clean) = clean_strict_mode_error(msg) {
            return (ErrorCode::SelectorNotFound, clean);
        }
    }

    // Map to appropriate error codes
    let code = if msg.contains("Timeout") {
        ErrorCode::Timeout
    } else if msg.contains("not found") || msg.contains("no element") {
        ErrorCode::SelectorNotFound
    } else if msg.contains("navigation") {
        ErrorCode::NavigationFailed
    } else {
        ErrorCode::InternalError
    };

    (code, msg.to_string())
}

/// Clean up verbose strict mode violation errors.
///
/// Input like:
/// ```text
/// Error: strict mode violation: locator("button") resolved to 55 elements:
///     1) <button class="...">...</button> aka get_by_role("button")
///     2) <button class="...">...</button> aka get_by_role("button")
///     ... (53 more elements)
/// ```
///
/// Becomes:
/// ```text
/// Selector "button" matched 55 elements (strict mode requires exactly 1). Use a more specific selector or `>> nth=0` to select the first match.
/// ```
fn clean_strict_mode_error(msg: &str) -> Option<String> {
    // Extract selector and count from the error message
    // Pattern: locator("...") resolved to N elements
    let selector_start = msg.find("locator(\"")?;
    let selector_content_start = selector_start + 9; // len of 'locator("'
    let selector_end = msg[selector_content_start..].find("\")")?;
    let selector = &msg[selector_content_start..selector_content_start + selector_end];

    // Find element count
    let resolved_idx = msg.find("resolved to ")?;
    let count_start = resolved_idx + 12; // len of "resolved to "
    let count_end = msg[count_start..].find(' ')?;
    let count: u32 = msg[count_start..count_start + count_end].parse().ok()?;

    Some(format!(
        "Selector \"{}\" matched {} elements (strict mode requires exactly 1). \
         Use a more specific selector or `>> nth=0` to select the first match.",
        selector, count
    ))
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
            PwError::Navigation { url, source } => {
                let mut msg = format!("Navigation to {url} failed: {source}");
                // If the URL looks like a CSS selector, add a helpful hint
                if looks_like_selector(url) {
                    msg.push_str(&format!(
                        ". Did you mean to use `-s {}` for a CSS selector?",
                        url
                    ));
                }
                (
                    ErrorCode::NavigationFailed,
                    msg,
                    Some(serde_json::json!({ "url": url })),
                )
            }
            PwError::ElementNotFound { selector } => (
                ErrorCode::SelectorNotFound,
                format!("No elements matched selector: {selector}"),
                Some(serde_json::json!({ "selector": selector })),
            ),
            PwError::JsEval(msg) => (ErrorCode::JsEvalFailed, msg.clone(), None),
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
            PwError::Io(err) => (ErrorCode::IoError, err.to_string(), None),
            PwError::Json(err) => (ErrorCode::InternalError, format!("JSON error: {err}"), None),
            PwError::Playwright(err) => {
                let msg = err.to_string();
                let (code, clean_msg) = classify_and_clean_playwright_error(&msg);
                (code, clean_msg, None)
            }
            PwError::Anyhow(err) => (ErrorCode::InternalError, err.to_string(), None),
        };

        CommandError {
            code,
            message,
            details,
        }
    }
}
