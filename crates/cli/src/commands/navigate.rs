//! Navigation command.

use crate::commands::snapshot::{
    EXTRACT_ELEMENTS_JS, EXTRACT_META_JS, EXTRACT_TEXT_JS, PageMeta, RawElement,
};
use crate::context::CommandContext;
use crate::error::Result;
use crate::output::{
    CommandInputs, InteractiveElement, OutputFormat, ResultBuilder, SnapshotData, print_result,
};
use crate::session_broker::{SessionBroker, SessionRequest};
use crate::target::{Resolve, ResolveEnv, ResolvedTarget, Target, TargetPolicy};
use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Raw inputs from CLI or batch JSON.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigateRaw {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default, alias = "url_flag")]
    pub url_flag: Option<String>,
}

impl NavigateRaw {
    pub fn from_cli(url: Option<String>, url_flag: Option<String>) -> Self {
        Self { url, url_flag }
    }
}

/// Resolved inputs ready for execution.
#[derive(Debug, Clone)]
pub struct NavigateResolved {
    pub target: ResolvedTarget,
}

impl NavigateResolved {
    pub fn preferred_url<'a>(&'a self, last_url: Option<&'a str>) -> Option<&'a str> {
        self.target.preferred_url(last_url)
    }
}

impl Resolve for NavigateRaw {
    type Output = NavigateResolved;

    fn resolve(self, env: &ResolveEnv<'_>) -> Result<NavigateResolved> {
        let url = self.url_flag.or(self.url);
        let target = env.resolve_target(url, TargetPolicy::AllowCurrentPage)?;
        Ok(NavigateResolved { target })
    }
}

const DEFAULT_MAX_TEXT_LENGTH: usize = 5000;

/// Execute navigation and return the actual browser URL after navigation.
///
/// After navigation completes, extracts a page snapshot (metadata, text content,
/// and interactive elements) matching the output of `pw page snapshot`.
pub async fn execute_resolved(
    args: &NavigateResolved,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    last_url: Option<&str>,
) -> Result<String> {
    let url_display = args.target.url_str().unwrap_or("<current page>");
    info!(target = "pw", url = %url_display, browser = %ctx.browser, "navigate");

    let preferred_url = args.preferred_url(last_url);
    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::Load, ctx).with_preferred_url(preferred_url),
        )
        .await?;

    match &args.target.target {
        Target::Navigate(url) => {
            session
                .goto_if_needed(url.as_str(), ctx.timeout_ms())
                .await?;
        }
        Target::CurrentPage => {}
    }

    let meta_js = format!("JSON.stringify({})", EXTRACT_META_JS);
    let meta: PageMeta = serde_json::from_str(&session.page().evaluate_value(&meta_js).await?)?;

    let text_js = format!(
        "JSON.stringify({}({}, {}))",
        EXTRACT_TEXT_JS, DEFAULT_MAX_TEXT_LENGTH, false
    );
    let text: String = serde_json::from_str(&session.page().evaluate_value(&text_js).await?)?;

    let elements_js = format!("JSON.stringify({})", EXTRACT_ELEMENTS_JS);
    let raw_elements: Vec<RawElement> =
        serde_json::from_str(&session.page().evaluate_value(&elements_js).await?)?;
    let elements: Vec<InteractiveElement> = raw_elements.into_iter().map(Into::into).collect();
    let element_count = elements.len();

    let result = ResultBuilder::new("navigate")
        .inputs(CommandInputs {
            url: args.target.url_str().map(String::from),
            ..Default::default()
        })
        .data(SnapshotData {
            url: meta.url.clone(),
            title: meta.title,
            viewport_width: meta.viewport_width,
            viewport_height: meta.viewport_height,
            text,
            elements,
            element_count,
        })
        .build();

    print_result(&result, format);

    session.close().await?;
    Ok(meta.url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigate_raw_deserialize() {
        let json = r#"{"url": "https://example.com"}"#;
        let raw: NavigateRaw = serde_json::from_str(json).unwrap();
        assert_eq!(raw.url, Some("https://example.com".into()));
    }
}
