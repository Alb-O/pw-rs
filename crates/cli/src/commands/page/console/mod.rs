//! Console message capture command.
//!
//! Captures JavaScript console output (log, warn, error, etc.) from a page.
//! Injects a capture script before navigation, then collects messages after
//! a configurable timeout.
//!
//! # Example
//!
//! ```bash
//! pw console https://example.com --timeout-ms 5000
//! ```

use std::time::Duration;

use crate::browser::js::console_capture_injection_js;
use crate::context::CommandContext;
use crate::error::Result;
use crate::output::{CommandInputs, OutputFormat, ResultBuilder, print_result};
use crate::session_broker::{SessionBroker, SessionRequest};
use crate::target::{Resolve, ResolveEnv, ResolvedTarget, TargetPolicy};
use crate::types::ConsoleMessage;
use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// Raw inputs from CLI or batch JSON before resolution.
///
/// Use [`Resolve::resolve`] to convert to [`ConsoleResolved`] for execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleRaw {
    /// Target URL, resolved from context if not provided.
    #[serde(default)]
    pub url: Option<String>,

    /// How long to capture messages before returning (default: 3000ms).
    #[serde(default, alias = "timeout_ms")]
    pub timeout_ms: Option<u64>,
}

impl ConsoleRaw {
    /// Creates a [`ConsoleRaw`] from CLI arguments.
    pub fn from_cli(url: Option<String>, timeout_ms: u64) -> Self {
        Self {
            url,
            timeout_ms: Some(timeout_ms),
        }
    }
}

/// Resolved inputs ready for execution.
///
/// The [`timeout_ms`](Self::timeout_ms) defaults to 3000 if not specified.
#[derive(Debug, Clone)]
pub struct ConsoleResolved {
    /// Navigation target (URL or current page).
    pub target: ResolvedTarget,

    /// Capture duration in milliseconds.
    pub timeout_ms: u64,
}

impl ConsoleResolved {
    /// Returns the URL for page preference matching.
    ///
    /// For [`Navigate`](crate::target::Target::Navigate) targets, returns the URL.
    /// For [`CurrentPage`](crate::target::Target::CurrentPage), returns `last_url` as a hint.
    pub fn preferred_url<'a>(&'a self, last_url: Option<&'a str>) -> Option<&'a str> {
        self.target.preferred_url(last_url)
    }
}

impl Resolve for ConsoleRaw {
    type Output = ConsoleResolved;

    fn resolve(self, env: &ResolveEnv<'_>) -> Result<ConsoleResolved> {
        let target = env.resolve_target(self.url, TargetPolicy::AllowCurrentPage)?;

        Ok(ConsoleResolved {
            target,
            timeout_ms: self.timeout_ms.unwrap_or(3000),
        })
    }
}

/// Captured console messages with summary counts.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConsoleData {
    messages: Vec<ConsoleMessage>,
    count: usize,
    error_count: usize,
    warning_count: usize,
}

/// Executes the console command with resolved arguments.
///
/// Injects a console capture script, navigates to the page, waits for
/// [`timeout_ms`](ConsoleResolved::timeout_ms), then collects all captured
/// console messages.
///
/// Messages are also emitted to tracing at the `pw.browser.console` target
/// for logging visibility.
///
/// # Errors
///
/// Returns an error if:
/// - Navigation fails
/// - Session creation fails
pub async fn execute_resolved(
    args: &ConsoleResolved,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    last_url: Option<&str>,
) -> Result<()> {
    let url_display = args.target.url_str().unwrap_or("<current page>");
    info!(target = "pw", url = %url_display, timeout_ms = args.timeout_ms, browser = %ctx.browser, "capture console");

    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(args.preferred_url(last_url)),
        )
        .await?;

    if let Err(err) = session
        .page()
        .evaluate(console_capture_injection_js())
        .await
    {
        warn!(target = "pw.browser.console", error = %err, "failed to inject console capture");
    }

    session
        .goto_target(&args.target.target, ctx.timeout_ms())
        .await?;

    tokio::time::sleep(Duration::from_millis(args.timeout_ms)).await;

    let messages_json = session
        .page()
        .evaluate_value("JSON.stringify(window.__consoleMessages || [])")
        .await
        .unwrap_or_else(|_| "[]".to_string());

    let messages: Vec<ConsoleMessage> = serde_json::from_str(&messages_json).unwrap_or_default();

    for msg in &messages {
        info!(
            target = "pw.browser.console",
            kind = %msg.msg_type,
            text = %msg.text,
            stack = ?msg.stack,
            "browser console"
        );
    }

    let error_count = messages.iter().filter(|m| m.msg_type == "error").count();
    let warning_count = messages.iter().filter(|m| m.msg_type == "warning").count();
    let count = messages.len();

    let result = ResultBuilder::new("console")
        .inputs(CommandInputs {
            url: args.target.url_str().map(String::from),
            extra: Some(serde_json::json!({ "timeout_ms": args.timeout_ms })),
            ..Default::default()
        })
        .data(ConsoleData {
            messages,
            count,
            error_count,
            warning_count,
        })
        .build();

    print_result(&result, format);
    session.close().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_raw_deserialize_from_json() {
        let json = r#"{"url": "https://example.com", "timeout_ms": 5000}"#;
        let raw: ConsoleRaw = serde_json::from_str(json).unwrap();
        assert_eq!(raw.url, Some("https://example.com".into()));
        assert_eq!(raw.timeout_ms, Some(5000));
    }

    #[test]
    fn console_raw_defaults() {
        let json = r#"{}"#;
        let raw: ConsoleRaw = serde_json::from_str(json).unwrap();
        assert_eq!(raw.timeout_ms, None);
    }
}
