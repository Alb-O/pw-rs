use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, PwError>;

#[derive(Debug, Error)]
pub enum PwError {
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

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Playwright(#[from] pw::Error),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}
