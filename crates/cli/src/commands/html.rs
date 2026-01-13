//! HTML content extraction command.

use crate::args;
use crate::context::CommandContext;
use crate::error::Result;
use crate::output::{CommandInputs, OutputFormat, ResultBuilder, print_result};
use crate::session_broker::{SessionBroker, SessionRequest};
use crate::target::{Resolve, ResolveEnv, ResolvedTarget, TargetPolicy};
use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

// ---------------------------------------------------------------------------
// Raw and Resolved Types
// ---------------------------------------------------------------------------

/// Raw inputs from CLI or batch JSON.
///
/// This struct captures the unprocessed arguments as they come from the user,
/// whether via CLI flags or batch mode JSON. Use [`Resolve`] to convert to
/// [`HtmlResolved`] for execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HtmlRaw {
    /// URL (positional argument).
    #[serde(default)]
    pub url: Option<String>,
    /// Selector (positional argument, may be detected from URL position).
    #[serde(default)]
    pub selector: Option<String>,
    /// URL via --url flag.
    #[serde(default, alias = "url_flag")]
    pub url_flag: Option<String>,
    /// Selector via --selector flag.
    #[serde(default, alias = "selector_flag")]
    pub selector_flag: Option<String>,
}

impl HtmlRaw {
    /// Create from CLI arguments.
    pub fn from_cli(
        url: Option<String>,
        selector: Option<String>,
        url_flag: Option<String>,
        selector_flag: Option<String>,
    ) -> Self {
        Self {
            url,
            selector,
            url_flag,
            selector_flag,
        }
    }
}

/// Resolved inputs ready for execution.
///
/// All arguments have been validated and resolved against context.
#[derive(Debug, Clone)]
pub struct HtmlResolved {
    /// Resolved navigation target.
    pub target: ResolvedTarget,
    /// Resolved CSS selector.
    pub selector: String,
}

impl HtmlResolved {
    /// Returns the URL for page preference matching.
    pub fn preferred_url<'a>(&'a self, last_url: Option<&'a str>) -> Option<&'a str> {
        self.target.preferred_url(last_url)
    }
}

impl Resolve for HtmlRaw {
    type Output = HtmlResolved;

    fn resolve(self, env: &ResolveEnv<'_>) -> Result<HtmlResolved> {
        // Smart detection: resolve positional vs flags
        let resolved = args::resolve_url_and_selector(
            self.url.clone(),
            self.url_flag,
            self.selector_flag.or(self.selector),
        );

        // Resolve target using typed target system
        let target = env.resolve_target(resolved.url, TargetPolicy::AllowCurrentPage)?;

        // Resolve selector with "html" as default (full page)
        let selector = env.resolve_selector(resolved.selector, Some("html"))?;

        Ok(HtmlResolved { target, selector })
    }
}

// ---------------------------------------------------------------------------
// Output Data
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HtmlData {
    html: String,
    selector: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    length: Option<usize>,
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

/// Execute the html command with resolved arguments.
pub async fn execute_resolved(
    args: &HtmlResolved,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    last_url: Option<&str>,
) -> Result<()> {
    let url_display = args.target.url_str().unwrap_or("<current page>");

    if args.selector == "html" {
        info!(target = "pw", url = %url_display, browser = %ctx.browser, "get full page HTML");
    } else {
        info!(target = "pw", url = %url_display, selector = %args.selector, browser = %ctx.browser, "get HTML for selector");
    }

    let preferred_url = args.preferred_url(last_url);
    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(preferred_url),
        )
        .await?;

    // Use typed target navigation
    session
        .goto_target(&args.target.target, ctx.timeout_ms())
        .await?;

    let locator = session.page().locator(&args.selector).await;
    let html = locator.inner_html().await?;

    let result = ResultBuilder::new("html")
        .inputs(CommandInputs {
            url: args.target.url_str().map(String::from),
            selector: Some(args.selector.clone()),
            ..Default::default()
        })
        .data(HtmlData {
            length: Some(html.len()),
            html,
            selector: args.selector.clone(),
        })
        .build();

    print_result(&result, format);
    session.close().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_raw_deserialize_from_json() {
        let json = r#"{"url": "https://example.com", "selector": "main"}"#;
        let raw: HtmlRaw = serde_json::from_str(json).unwrap();
        assert_eq!(raw.url, Some("https://example.com".into()));
        assert_eq!(raw.selector, Some("main".into()));
    }

    #[test]
    fn html_raw_deserialize_with_flags() {
        let json = r#"{"url_flag": "https://example.com", "selector_flag": ".content"}"#;
        let raw: HtmlRaw = serde_json::from_str(json).unwrap();
        assert_eq!(raw.url_flag, Some("https://example.com".into()));
        assert_eq!(raw.selector_flag, Some(".content".into()));
    }

    #[test]
    fn html_raw_deserialize_camel_case() {
        let json = r#"{"urlFlag": "https://example.com", "selectorFlag": ".content"}"#;
        let raw: HtmlRaw = serde_json::from_str(json).unwrap();
        assert_eq!(raw.url_flag, Some("https://example.com".into()));
        assert_eq!(raw.selector_flag, Some(".content".into()));
    }

    #[test]
    fn html_raw_from_cli() {
        let raw = HtmlRaw::from_cli(
            Some("https://example.com".into()),
            None,
            None,
            Some(".content".into()),
        );
        assert_eq!(raw.url, Some("https://example.com".into()));
        assert_eq!(raw.selector_flag, Some(".content".into()));
    }
}
