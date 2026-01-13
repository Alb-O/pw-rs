//! Structured output envelope for all CLI commands.
//!
//! Provides a consistent JSON output format for machine consumption (agent/API usage).
//!
//! ## Output Contract
//!
//! Every command produces a result envelope on stdout:
//!
//! ```json
//! {
//!   "ok": true,
//!   "command": "navigate",
//!   "data": { ... },
//!   "timings": { "duration_ms": 1234 },
//!   "artifacts": []
//! }
//! ```
//!
//! On failure:
//!
//! ```json
//! {
//!   "ok": false,
//!   "command": "navigate",
//!   "error": {
//!     "code": "NAVIGATION_FAILED",
//!     "message": "Navigation to https://example.com failed: timeout",
//!     "details": { ... }
//!   }
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Current schema version for command output.
///
/// Increment this when making breaking changes to the output structure.
/// Agents can use this to detect incompatible CLI versions.
pub const SCHEMA_VERSION: u32 = 1;

/// Output format for CLI results.
///
/// Used both for clap argument parsing and internal formatting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    /// TOON output (default, token-efficient for LLMs)
    #[default]
    Toon,
    /// JSON output
    Json,
    /// Newline-delimited JSON (streaming)
    Ndjson,
    /// Human-readable text
    Text,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "toon" => Ok(OutputFormat::Toon),
            "json" => Ok(OutputFormat::Json),
            "ndjson" => Ok(OutputFormat::Ndjson),
            "text" => Ok(OutputFormat::Text),
            _ => Err(format!("unknown format: {s}")),
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Toon => write!(f, "toon"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Ndjson => write!(f, "ndjson"),
            OutputFormat::Text => write!(f, "text"),
        }
    }
}

/// The main result envelope returned by all commands.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResult<T: Serialize> {
    /// Schema version for output format compatibility.
    ///
    /// Agents can use this to detect incompatible CLI versions.
    /// Currently always [`SCHEMA_VERSION`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<u32>,

    /// Whether the command succeeded
    pub ok: bool,

    /// Command name (e.g., "navigate", "click", "screenshot")
    pub command: String,

    /// Inputs used for this command (for traceability)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<CommandInputs>,

    /// Command-specific result data (only present on success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,

    /// Error information (only present on failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CommandError>,

    /// Timing information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timings: Option<Timings>,

    /// Artifacts produced (screenshots, files, etc.)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<Artifact>,

    /// Diagnostic information (warnings, console messages, etc.)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,

    /// Effective configuration used for this command
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<EffectiveConfig>,
}

/// Inputs that were used for the command (for traceability)
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CommandInputs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<PathBuf>,

    /// Additional command-specific inputs
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

/// Error information for failed commands
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    /// Error code (e.g., "NAVIGATION_FAILED", "SELECTOR_NOT_FOUND", "TIMEOUT")
    pub code: ErrorCode,

    /// Human-readable error message
    pub message: String,

    /// Additional error details (stack trace, context, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// Standardized error codes for programmatic handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// Browser failed to launch
    BrowserLaunchFailed,
    /// Navigation to URL failed
    NavigationFailed,
    /// Selector did not match any elements
    SelectorNotFound,
    /// Multiple elements matched when one was expected
    SelectorAmbiguous,
    /// Operation timed out
    Timeout,
    /// JavaScript evaluation failed
    JsEvalFailed,
    /// Screenshot capture failed
    ScreenshotFailed,
    /// File I/O error
    IoError,
    /// Session/connection error
    SessionError,
    /// Invalid input provided
    InvalidInput,
    /// Authentication required or failed
    AuthError,
    /// Unknown/internal error
    InternalError,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCode::BrowserLaunchFailed => write!(f, "BROWSER_LAUNCH_FAILED"),
            ErrorCode::NavigationFailed => write!(f, "NAVIGATION_FAILED"),
            ErrorCode::SelectorNotFound => write!(f, "SELECTOR_NOT_FOUND"),
            ErrorCode::SelectorAmbiguous => write!(f, "SELECTOR_AMBIGUOUS"),
            ErrorCode::Timeout => write!(f, "TIMEOUT"),
            ErrorCode::JsEvalFailed => write!(f, "JS_EVAL_FAILED"),
            ErrorCode::ScreenshotFailed => write!(f, "SCREENSHOT_FAILED"),
            ErrorCode::IoError => write!(f, "IO_ERROR"),
            ErrorCode::SessionError => write!(f, "SESSION_ERROR"),
            ErrorCode::InvalidInput => write!(f, "INVALID_INPUT"),
            ErrorCode::AuthError => write!(f, "AUTH_ERROR"),
            ErrorCode::InternalError => write!(f, "INTERNAL_ERROR"),
        }
    }
}

/// Timing information for the command
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timings {
    /// Total duration in milliseconds
    pub duration_ms: u64,

    /// Time spent on navigation (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub navigation_ms: Option<u64>,

    /// Time spent waiting for condition (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_ms: Option<u64>,
}

impl From<Duration> for Timings {
    fn from(duration: Duration) -> Self {
        Timings {
            duration_ms: duration.as_millis() as u64,
            navigation_ms: None,
            wait_ms: None,
        }
    }
}

/// Artifact produced by a command (file, screenshot, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    /// Type of artifact
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,

    /// Path to the artifact
    pub path: PathBuf,

    /// Size in bytes (if known)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

/// Types of artifacts that can be produced
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactType {
    Screenshot,
    Html,
    Auth,
    Trace,
    Video,
}

/// Diagnostic messages (warnings, info, etc.)
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    /// Severity level
    pub level: DiagnosticLevel,

    /// Diagnostic message
    pub message: String,

    /// Source of the diagnostic (e.g., "browser", "network", "js")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Diagnostic severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticLevel {
    Info,
    Warning,
    Error,
}

/// Effective configuration used for the command
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveConfig {
    /// Browser type used
    pub browser: String,

    /// Whether running headless
    pub headless: bool,

    /// Wait condition used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_until: Option<String>,

    /// Timeout used (ms)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,

    /// CDP/WS endpoint connected to (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

/// Builder for constructing command results
pub struct ResultBuilder<T: Serialize> {
    schema_version: Option<u32>,
    command: String,
    inputs: Option<CommandInputs>,
    data: Option<T>,
    error: Option<CommandError>,
    start_time: Option<Instant>,
    timings: Option<Timings>,
    artifacts: Vec<Artifact>,
    diagnostics: Vec<Diagnostic>,
    config: Option<EffectiveConfig>,
}

impl<T: Serialize> ResultBuilder<T> {
    /// Create a new result builder for the given command.
    ///
    /// The schema version is automatically set to [`SCHEMA_VERSION`].
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            schema_version: Some(SCHEMA_VERSION),
            command: command.into(),
            inputs: None,
            data: None,
            error: None,
            start_time: Some(Instant::now()),
            timings: None,
            artifacts: Vec::new(),
            diagnostics: Vec::new(),
            config: None,
        }
    }

    /// Override the schema version (useful for testing or compatibility).
    pub fn schema_version(mut self, version: u32) -> Self {
        self.schema_version = Some(version);
        self
    }

    /// Disable schema version in output (for backwards compatibility).
    pub fn no_schema_version(mut self) -> Self {
        self.schema_version = None;
        self
    }

    /// Set the inputs used for this command
    pub fn inputs(mut self, inputs: CommandInputs) -> Self {
        self.inputs = Some(inputs);
        self
    }

    /// Set the successful result data
    pub fn data(mut self, data: T) -> Self {
        self.data = Some(data);
        self
    }

    /// Set an error
    pub fn error(mut self, code: ErrorCode, message: impl Into<String>) -> Self {
        self.error = Some(CommandError {
            code,
            message: message.into(),
            details: None,
        });
        self
    }

    /// Set an error with details
    pub fn error_with_details(
        mut self,
        code: ErrorCode,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        self.error = Some(CommandError {
            code,
            message: message.into(),
            details: Some(details),
        });
        self
    }

    /// Add an artifact
    pub fn artifact(mut self, artifact: Artifact) -> Self {
        self.artifacts.push(artifact);
        self
    }

    /// Add a diagnostic
    pub fn diagnostic(mut self, level: DiagnosticLevel, message: impl Into<String>) -> Self {
        self.diagnostics.push(Diagnostic {
            level,
            message: message.into(),
            source: None,
        });
        self
    }

    /// Add a diagnostic with source
    pub fn diagnostic_with_source(
        mut self,
        level: DiagnosticLevel,
        message: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        self.diagnostics.push(Diagnostic {
            level,
            message: message.into(),
            source: Some(source.into()),
        });
        self
    }

    /// Set the effective configuration
    pub fn config(mut self, config: EffectiveConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Override timings (if not using automatic timing from start_time)
    pub fn timings(mut self, timings: Timings) -> Self {
        self.timings = Some(timings);
        self
    }

    /// Build the final result
    pub fn build(self) -> CommandResult<T> {
        let ok = self.error.is_none() && self.data.is_some();

        let timings = self
            .timings
            .or_else(|| self.start_time.map(|start| Timings::from(start.elapsed())));

        CommandResult {
            schema_version: self.schema_version,
            ok,
            command: self.command,
            inputs: self.inputs,
            data: self.data,
            error: self.error,
            timings,
            artifacts: self.artifacts,
            diagnostics: self.diagnostics,
            config: self.config,
        }
    }
}

/// Print a command result to stdout in the specified format
pub fn print_result<T: Serialize>(result: &CommandResult<T>, format: OutputFormat) {
    match format {
        OutputFormat::Toon => {
            if let Ok(json_value) = serde_json::to_value(result) {
                println!("{}", toon::encode(&json_value, None));
            }
        }
        OutputFormat::Json => {
            if let Ok(json) = serde_json::to_string_pretty(result) {
                println!("{json}");
            }
        }
        OutputFormat::Ndjson => {
            if let Ok(json) = serde_json::to_string(result) {
                println!("{json}");
            }
        }
        OutputFormat::Text => {
            print_result_text(result);
        }
    }
}

/// Print a command result in human-readable text format
fn print_result_text<T: Serialize>(result: &CommandResult<T>) {
    let mut stdout = io::stdout().lock();

    if result.ok {
        if let Some(ref data) = result.data {
            // Try to pretty-print JSON data
            if let Ok(json) = serde_json::to_string_pretty(data) {
                let _ = writeln!(stdout, "{json}");
            }
        }
    } else if let Some(ref error) = result.error {
        let _ = writeln!(stdout, "Error [{}]: {}", error.code, error.message);
        if let Some(ref details) = error.details {
            if let Ok(json) = serde_json::to_string_pretty(details) {
                let _ = writeln!(stdout, "Details: {json}");
            }
        }
    }

    // Print diagnostics
    for diag in &result.diagnostics {
        let prefix = match diag.level {
            DiagnosticLevel::Info => "info",
            DiagnosticLevel::Warning => "warning",
            DiagnosticLevel::Error => "error",
        };
        if let Some(ref source) = diag.source {
            let _ = writeln!(stdout, "[{prefix}:{source}] {}", diag.message);
        } else {
            let _ = writeln!(stdout, "[{prefix}] {}", diag.message);
        }
    }

    // Print artifacts
    for artifact in &result.artifacts {
        let _ = writeln!(
            stdout,
            "Saved {:?}: {}",
            artifact.artifact_type,
            artifact.path.display()
        );
    }

    // Print timing in verbose/debug scenarios
    if let Some(ref timings) = result.timings {
        let _ = writeln!(stdout, "Completed in {}ms", timings.duration_ms);
    }
}

/// Print an error to stderr in human-readable format
pub fn print_error_stderr(error: &CommandError) {
    eprintln!("Error [{}]: {}", error.code, error.message);
}

/// A command result with no data (for commands that only produce side effects)
pub type EmptyResult = CommandResult<()>;

/// A command result with a simple string value
pub type StringResult = CommandResult<String>;

/// A command failure that includes collected artifacts.
///
/// This is used when a command fails but we want to include diagnostic artifacts
/// (screenshot, HTML) in the error response.
#[derive(Debug)]
pub struct FailureWithArtifacts {
    pub error: CommandError,
    pub artifacts: Vec<Artifact>,
}

impl FailureWithArtifacts {
    pub fn new(error: CommandError) -> Self {
        Self {
            error,
            artifacts: Vec::new(),
        }
    }

    pub fn with_artifacts(mut self, artifacts: Vec<Artifact>) -> Self {
        self.artifacts = artifacts;
        self
    }
}

/// Print a failure result with artifacts to stdout
pub fn print_failure_with_artifacts(
    command: &str,
    failure: &FailureWithArtifacts,
    format: OutputFormat,
) {
    let result: CommandResult<()> = ResultBuilder::new(command)
        .error(failure.error.code, &failure.error.message)
        .build();

    // We need to manually add artifacts since ResultBuilder doesn't support
    // adding artifacts to error results. Create a modified result.
    let result_with_artifacts = CommandResult {
        schema_version: result.schema_version,
        ok: false,
        command: result.command,
        inputs: result.inputs,
        data: None::<()>,
        error: Some(failure.error.clone()),
        timings: result.timings,
        artifacts: failure.artifacts.clone(),
        diagnostics: result.diagnostics,
        config: result.config,
    };

    print_result(&result_with_artifacts, format);
}

/// Result data for navigate command
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigateData {
    /// The input URL that was requested
    pub url: String,
    /// The actual browser URL after navigation (may differ due to redirects)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_url: Option<String>,
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Result data for click command
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClickData {
    pub before_url: String,
    pub after_url: String,
    pub navigated: bool,
    pub selector: String,
}

/// Result data for screenshot command
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotData {
    pub path: PathBuf,
    pub full_page: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

/// Result data for text command
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextData {
    pub text: String,
    pub selector: String,
    pub match_count: usize,
}

/// Result data for fill command
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FillData {
    pub selector: String,
    pub text: String,
}

/// Result data for eval command
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalData {
    pub result: serde_json::Value,
    pub expression: String,
}

/// Result data for session start command
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartData {
    pub ws_endpoint: Option<String>,
    pub cdp_endpoint: Option<String>,
    pub browser: String,
    pub headless: bool,
}

/// Result data for elements command
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementsData {
    pub elements: Vec<InteractiveElement>,
    pub count: usize,
}

/// An interactive element found on the page
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveElement {
    pub tag: String,
    pub selector: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Result data for snapshot command (page model for agents)
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotData {
    /// Current page URL (may differ from requested due to redirects).
    pub url: String,
    /// Page title.
    pub title: String,
    /// Viewport width in pixels.
    pub viewport_width: i32,
    /// Viewport height in pixels.
    pub viewport_height: i32,
    /// Visible text content (truncated to max_text_length).
    pub text: String,
    /// Interactive elements (buttons, links, inputs, etc.).
    pub elements: Vec<InteractiveElement>,
    /// Number of interactive elements found.
    pub element_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_builder_success() {
        let result: CommandResult<NavigateData> = ResultBuilder::new("navigate")
            .inputs(CommandInputs {
                url: Some("https://example.com".into()),
                ..Default::default()
            })
            .data(NavigateData {
                url: "https://example.com".into(),
                actual_url: None,
                title: "Example".into(),
                errors: vec![],
                warnings: vec![],
            })
            .build();

        assert!(result.ok);
        assert_eq!(result.command, "navigate");
        assert!(result.data.is_some());
        assert!(result.error.is_none());
    }

    #[test]
    fn result_builder_error() {
        let result: CommandResult<NavigateData> = ResultBuilder::new("navigate")
            .inputs(CommandInputs {
                url: Some("https://blocked.com".into()),
                ..Default::default()
            })
            .error(ErrorCode::NavigationFailed, "Connection refused")
            .build();

        assert!(!result.ok);
        assert!(result.data.is_none());
        assert!(result.error.is_some());
        assert_eq!(
            result.error.as_ref().unwrap().code,
            ErrorCode::NavigationFailed
        );
    }

    #[test]
    fn error_code_display() {
        assert_eq!(ErrorCode::NavigationFailed.to_string(), "NAVIGATION_FAILED");
        assert_eq!(
            ErrorCode::SelectorNotFound.to_string(),
            "SELECTOR_NOT_FOUND"
        );
    }

    #[test]
    fn output_format_parse() {
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!("text".parse::<OutputFormat>().unwrap(), OutputFormat::Text);
        assert!("invalid".parse::<OutputFormat>().is_err());
    }

    #[test]
    fn serialize_command_result() {
        let result: CommandResult<ClickData> = ResultBuilder::new("click")
            .data(ClickData {
                before_url: "https://example.com".into(),
                after_url: "https://example.com/page".into(),
                navigated: true,
                selector: "a.link".into(),
            })
            .build();

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"ok\":true"));
        assert!(json.contains("\"navigated\":true"));
    }

    #[test]
    fn artifacts_included() {
        let result: CommandResult<ScreenshotData> = ResultBuilder::new("screenshot")
            .data(ScreenshotData {
                path: "/tmp/screenshot.png".into(),
                full_page: false,
                width: Some(1920),
                height: Some(1080),
            })
            .artifact(Artifact {
                artifact_type: ArtifactType::Screenshot,
                path: "/tmp/screenshot.png".into(),
                size_bytes: Some(12345),
            })
            .build();

        assert_eq!(result.artifacts.len(), 1);
        assert_eq!(result.artifacts[0].artifact_type, ArtifactType::Screenshot);
    }

    #[test]
    fn diagnostics_included() {
        let result: CommandResult<NavigateData> = ResultBuilder::new("navigate")
            .data(NavigateData {
                url: "https://example.com".into(),
                actual_url: None,
                title: "Example".into(),
                errors: vec![],
                warnings: vec![],
            })
            .diagnostic(DiagnosticLevel::Warning, "Page loaded slowly")
            .diagnostic_with_source(DiagnosticLevel::Error, "JS error occurred", "browser")
            .build();

        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].level, DiagnosticLevel::Warning);
        assert_eq!(result.diagnostics[1].source, Some("browser".into()));
    }
}
