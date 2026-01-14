//! Fill form element command.
//!
//! Fills a form input element with text. Supports text inputs, textareas,
//! and contenteditable elements.
//!
//! # Example
//!
//! ```bash
//! pw fill --selector "input[name=email]" --text "user@example.com"
//! ```

use crate::context::CommandContext;
use crate::error::Result;
use crate::output::{CommandInputs, FillData, OutputFormat, ResultBuilder, print_result};
use crate::session_broker::{SessionBroker, SessionRequest};
use crate::target::{Resolve, ResolveEnv, ResolvedTarget, TargetPolicy};
use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Raw inputs from CLI or batch JSON before resolution.
///
/// This struct captures unprocessed arguments as provided by the user.
/// Use [`Resolve::resolve`] to convert to [`FillResolved`] for execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FillRaw {
    /// Target URL, resolved from context if not provided.
    #[serde(default)]
    pub url: Option<String>,

    /// CSS selector for the element to fill.
    #[serde(default)]
    pub selector: Option<String>,

    /// Text to fill into the element.
    #[serde(default)]
    pub text: Option<String>,
}

impl FillRaw {
    /// Creates a [`FillRaw`] from CLI arguments.
    pub fn from_cli(url: Option<String>, selector: Option<String>, text: Option<String>) -> Self {
        Self {
            url,
            selector,
            text,
        }
    }
}

/// Resolved inputs ready for execution.
///
/// All arguments have been validated and resolved against context state.
/// The [`target`](Self::target) contains either a concrete URL or indicates
/// the current page should be used.
#[derive(Debug, Clone)]
pub struct FillResolved {
    /// Navigation target (URL or current page).
    pub target: ResolvedTarget,

    /// CSS selector for the target element.
    pub selector: String,

    /// Text to fill into the element.
    pub text: String,
}

impl FillResolved {
    /// Returns the URL for page preference matching.
    ///
    /// For [`Navigate`](crate::target::Target::Navigate) targets, returns the URL.
    /// For [`CurrentPage`](crate::target::Target::CurrentPage), returns `last_url` as a hint.
    pub fn preferred_url<'a>(&'a self, last_url: Option<&'a str>) -> Option<&'a str> {
        self.target.preferred_url(last_url)
    }
}

impl Resolve for FillRaw {
    type Output = FillResolved;

    fn resolve(self, env: &ResolveEnv<'_>) -> Result<FillResolved> {
        let target = env.resolve_target(self.url, TargetPolicy::AllowCurrentPage)?;
        let selector = env.resolve_selector(self.selector, None)?;
        let text = self.text.unwrap_or_default();

        Ok(FillResolved {
            target,
            selector,
            text,
        })
    }
}

/// Executes the fill command with resolved arguments.
///
/// Navigates to the target page (if not already there), locates the element
/// matching `selector`, and fills it with the specified text.
///
/// # Errors
///
/// Returns an error if:
/// - Navigation fails
/// - The selector matches no elements
/// - The element is not fillable (e.g., not an input)
pub async fn execute_resolved(
    args: &FillResolved,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    last_url: Option<&str>,
) -> Result<()> {
    let url_display = args.target.url_str().unwrap_or("<current page>");
    info!(target = "pw", url = %url_display, selector = %args.selector, "fill");

    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::Load, ctx)
                .with_preferred_url(args.preferred_url(last_url)),
        )
        .await?;

    session
        .goto_target(&args.target.target, ctx.timeout_ms())
        .await?;

    let locator = session.page().locator(&args.selector).await;
    locator.fill(&args.text, None).await?;

    let result = ResultBuilder::new("fill")
        .inputs(CommandInputs {
            url: args.target.url_str().map(String::from),
            selector: Some(args.selector.clone()),
            ..Default::default()
        })
        .data(FillData {
            selector: args.selector.clone(),
            text: args.text.clone(),
        })
        .build();

    print_result(&result, format);
    session.close().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_raw_deserialize_from_json() {
        let json = r#"{"url": "https://example.com", "selector": "input", "text": "hello"}"#;
        let raw: FillRaw = serde_json::from_str(json).unwrap();
        assert_eq!(raw.url, Some("https://example.com".into()));
        assert_eq!(raw.selector, Some("input".into()));
        assert_eq!(raw.text, Some("hello".into()));
    }
}
