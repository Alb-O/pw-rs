//! Wait command for various conditions.
//!
//! Waits for a specified condition before continuing. Supports:
//! - **Timeout**: numeric milliseconds (e.g., `"1000"`)
//! - **Load state**: `"load"`, `"domcontentloaded"`, `"networkidle"`
//! - **Selector**: CSS selector to wait for element presence
//!
//! # Examples
//!
//! ```bash
//! pw wait --condition 2000           # wait 2 seconds
//! pw wait --condition networkidle    # wait for network idle
//! pw wait --condition ".loaded"      # wait for element
//! ```

use std::time::Duration;

use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::output::{CommandInputs, ErrorCode, OutputFormat, ResultBuilder, print_result};
use crate::session_broker::{SessionBroker, SessionRequest};
use crate::target::{Resolve, ResolveEnv, ResolvedTarget, TargetPolicy};
use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Raw inputs from CLI or batch JSON before resolution.
///
/// Use [`Resolve::resolve`] to convert to [`WaitResolved`] for execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitRaw {
    /// Target URL, resolved from context if not provided.
    #[serde(default)]
    pub url: Option<String>,

    /// Wait condition: milliseconds, load state, or CSS selector.
    #[serde(default)]
    pub condition: Option<String>,
}

impl WaitRaw {
    /// Creates a [`WaitRaw`] from CLI arguments.
    pub fn from_cli(url: Option<String>, condition: Option<String>) -> Self {
        Self { url, condition }
    }
}

/// Resolved inputs ready for execution.
///
/// The [`condition`](Self::condition) has been validated as present.
#[derive(Debug, Clone)]
pub struct WaitResolved {
    /// Navigation target (URL or current page).
    pub target: ResolvedTarget,

    /// Wait condition (timeout ms, load state, or CSS selector).
    pub condition: String,
}

impl WaitResolved {
    /// Returns the URL for page preference matching.
    ///
    /// For [`Navigate`](crate::target::Target::Navigate) targets, returns the URL.
    /// For [`CurrentPage`](crate::target::Target::CurrentPage), returns `last_url` as a hint.
    pub fn preferred_url<'a>(&'a self, last_url: Option<&'a str>) -> Option<&'a str> {
        self.target.preferred_url(last_url)
    }
}

impl Resolve for WaitRaw {
    type Output = WaitResolved;

    fn resolve(self, env: &ResolveEnv<'_>) -> Result<WaitResolved> {
        let target = env.resolve_target(self.url, TargetPolicy::AllowCurrentPage)?;
        let condition = self
            .condition
            .ok_or_else(|| PwError::Context("No condition provided for wait command".into()))?;

        Ok(WaitResolved { target, condition })
    }
}

/// Output data for the wait command result.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WaitData {
    condition: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    waited_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selector_found: Option<bool>,
}

/// Executes the wait command with resolved arguments.
///
/// Interprets the condition and waits accordingly:
/// - Numeric string: sleep for that many milliseconds
/// - `"load"`, `"domcontentloaded"`, `"networkidle"`: wait for page load state
/// - Other strings: treated as CSS selector, polls until element exists
///
/// # Errors
///
/// Returns an error if:
/// - Navigation fails
/// - Selector wait times out (30 second limit)
pub async fn execute_resolved(
    args: &WaitResolved,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    last_url: Option<&str>,
) -> Result<()> {
    let url_display = args.target.url_str().unwrap_or("<current page>");
    info!(target = "pw", url = %url_display, condition = %args.condition, browser = %ctx.browser, "wait");

    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(args.preferred_url(last_url)),
        )
        .await?;
    session
        .goto_target(&args.target.target, ctx.timeout_ms())
        .await?;

    let condition = &args.condition;
    let url_str = args.target.url_str();

    if let Ok(ms) = condition.parse::<u64>() {
        tokio::time::sleep(Duration::from_millis(ms)).await;

        let result = ResultBuilder::new("wait")
            .inputs(CommandInputs {
                url: url_str.map(String::from),
                extra: Some(serde_json::json!({ "condition": condition })),
                ..Default::default()
            })
            .data(WaitData {
                condition: format!("timeout:{ms}ms"),
                waited_ms: Some(ms),
                selector_found: None,
            })
            .build();

        print_result(&result, format);
    } else if matches!(
        condition.as_str(),
        "load" | "domcontentloaded" | "networkidle"
    ) {
        let result = ResultBuilder::new("wait")
            .inputs(CommandInputs {
                url: url_str.map(String::from),
                extra: Some(serde_json::json!({ "condition": condition })),
                ..Default::default()
            })
            .data(WaitData {
                condition: format!("loadstate:{condition}"),
                waited_ms: None,
                selector_found: None,
            })
            .build();

        print_result(&result, format);
    } else {
        wait_for_selector(&session, condition, url_str, format).await?;
    }

    session.close().await
}

/// Polls for a CSS selector until it appears or times out.
async fn wait_for_selector(
    session: &crate::session_broker::SessionHandle,
    selector: &str,
    url_str: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");
    let max_attempts = 30u64;

    for attempt in 0..max_attempts {
        let visible = session
            .page()
            .evaluate_value(&format!("document.querySelector('{escaped}') !== null"))
            .await
            .unwrap_or_else(|_| "false".to_string());

        if visible == "true" {
            let result = ResultBuilder::new("wait")
                .inputs(CommandInputs {
                    url: url_str.map(String::from),
                    selector: Some(selector.to_string()),
                    ..Default::default()
                })
                .data(WaitData {
                    condition: format!("selector:{selector}"),
                    waited_ms: Some(attempt * 1000),
                    selector_found: Some(true),
                })
                .build();

            print_result(&result, format);
            return Ok(());
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    let result = ResultBuilder::<WaitData>::new("wait")
        .inputs(CommandInputs {
            url: url_str.map(String::from),
            selector: Some(selector.to_string()),
            ..Default::default()
        })
        .error(
            ErrorCode::Timeout,
            format!(
                "Timeout after {}ms waiting for selector: {selector}",
                max_attempts * 1000
            ),
        )
        .build();

    print_result(&result, format);

    Err(PwError::Timeout {
        ms: max_attempts * 1000,
        condition: selector.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_raw_deserialize_from_json() {
        let json = r#"{"url": "https://example.com", "condition": "1000"}"#;
        let raw: WaitRaw = serde_json::from_str(json).unwrap();
        assert_eq!(raw.url, Some("https://example.com".into()));
        assert_eq!(raw.condition, Some("1000".into()));
    }

    #[test]
    fn wait_raw_deserialize_selector_condition() {
        let json = r#"{"condition": ".loaded"}"#;
        let raw: WaitRaw = serde_json::from_str(json).unwrap();
        assert_eq!(raw.condition, Some(".loaded".into()));
    }
}
