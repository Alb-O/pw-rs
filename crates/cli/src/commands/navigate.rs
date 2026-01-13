//! Navigation command.

use crate::context::CommandContext;
use crate::error::Result;
use crate::output::{
    CommandInputs, DiagnosticLevel, NavigateData, OutputFormat, ResultBuilder, print_result,
};
use crate::session_broker::{SessionBroker, SessionRequest};
use crate::target::{Resolve, ResolveEnv, ResolvedTarget, Target, TargetPolicy};
use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// Raw and Resolved Types
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

/// Execute navigation and return the actual browser URL after navigation.
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

    // Navigate based on target type
    match &args.target.target {
        Target::Navigate(url) => {
            session
                .goto_if_needed(url.as_str(), ctx.timeout_ms())
                .await?;
        }
        Target::CurrentPage => {
            // No navigation needed
        }
    }

    let title = session.page().title().await.unwrap_or_default();
    let actual_url = session
        .page()
        .evaluate_value("window.location.href")
        .await
        .unwrap_or_else(|_| session.page().url());

    let errors_json = session
        .page()
        .evaluate_value("JSON.stringify(window.__playwrightErrors || [])")
        .await
        .unwrap_or_else(|_| "[]".to_string());
    let errors: Vec<String> = serde_json::from_str(&errors_json).unwrap_or_default();

    if !errors.is_empty() {
        warn!(
            target = "pw.browser",
            count = errors.len(),
            "page reported errors"
        );
    }

    let input_url = args.target.url_str().unwrap_or(&actual_url);
    let actual_url_field = if actual_url != input_url {
        Some(actual_url.clone())
    } else {
        None
    };

    let mut builder = ResultBuilder::new("navigate")
        .inputs(CommandInputs {
            url: args.target.url_str().map(String::from),
            ..Default::default()
        })
        .data(NavigateData {
            url: input_url.to_string(),
            actual_url: actual_url_field,
            title,
            errors: errors.clone(),
            warnings: vec![],
        });

    for error in &errors {
        builder = builder.diagnostic_with_source(DiagnosticLevel::Error, error, "browser");
    }

    let result = builder.build();
    print_result(&result, format);

    session.close().await?;
    Ok(actual_url)
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
