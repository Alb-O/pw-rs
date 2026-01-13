//! Click element command.

use std::path::Path;
use std::time::Duration;

use crate::args;
use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::output::{
    ClickData, CommandInputs, DownloadedFile, FailureWithArtifacts, OutputFormat, ResultBuilder,
    print_failure_with_artifacts, print_result,
};
use crate::session_broker::{SessionBroker, SessionHandle, SessionRequest};
use crate::target::{Resolve, ResolveEnv, ResolvedTarget, TargetPolicy};
use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

// ---------------------------------------------------------------------------
// Raw and Resolved Types
// ---------------------------------------------------------------------------

/// Raw inputs from CLI or batch JSON.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClickRaw {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default, alias = "url_flag")]
    pub url_flag: Option<String>,
    #[serde(default, alias = "selector_flag")]
    pub selector_flag: Option<String>,
    #[serde(default, alias = "wait_ms")]
    pub wait_ms: Option<u64>,
}

impl ClickRaw {
    pub fn from_cli(
        url: Option<String>,
        selector: Option<String>,
        url_flag: Option<String>,
        selector_flag: Option<String>,
        wait_ms: Option<u64>,
    ) -> Self {
        Self {
            url,
            selector,
            url_flag,
            selector_flag,
            wait_ms,
        }
    }
}

/// Resolved inputs ready for execution.
#[derive(Debug, Clone)]
pub struct ClickResolved {
    pub target: ResolvedTarget,
    pub selector: String,
    pub wait_ms: u64,
}

impl ClickResolved {
    pub fn preferred_url<'a>(&'a self, last_url: Option<&'a str>) -> Option<&'a str> {
        self.target.preferred_url(last_url)
    }
}

impl Resolve for ClickRaw {
    type Output = ClickResolved;

    fn resolve(self, env: &ResolveEnv<'_>) -> Result<ClickResolved> {
        let resolved = args::resolve_url_and_selector(
            self.url.clone(),
            self.url_flag,
            self.selector_flag.or(self.selector),
        );

        let target = env.resolve_target(resolved.url, TargetPolicy::AllowCurrentPage)?;
        let selector = env.resolve_selector(resolved.selector, None)?;
        let wait_ms = self.wait_ms.unwrap_or(500);

        Ok(ClickResolved {
            target,
            selector,
            wait_ms,
        })
    }
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

/// Execute click and return the actual browser URL after the click.
pub async fn execute_resolved(
    args: &ClickResolved,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    artifacts_dir: Option<&Path>,
    last_url: Option<&str>,
) -> Result<String> {
    let url_display = args.target.url_str().unwrap_or("<current page>");
    info!(target = "pw", url = %url_display, selector = %args.selector, browser = %ctx.browser, "click element");

    let preferred_url = args.preferred_url(last_url);
    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(preferred_url),
        )
        .await?;

    match execute_inner(&session, &args.target, &args.selector, args.wait_ms, format).await {
        Ok(after_url) => {
            session.close().await?;
            Ok(after_url)
        }
        Err(e) => {
            let artifacts = session
                .collect_failure_artifacts(artifacts_dir, "click")
                .await;

            if !artifacts.is_empty() {
                let failure = FailureWithArtifacts::new(e.to_command_error())
                    .with_artifacts(artifacts.artifacts);
                print_failure_with_artifacts("click", &failure, format);
                let _ = session.close().await;
                return Err(PwError::OutputAlreadyPrinted);
            }

            let _ = session.close().await;
            Err(e)
        }
    }
}

async fn execute_inner(
    session: &SessionHandle,
    target: &ResolvedTarget,
    selector: &str,
    wait_ms: u64,
    format: OutputFormat,
) -> Result<String> {
    session.goto_target(&target.target).await?;

    let before_url = session
        .page()
        .evaluate_value("window.location.href")
        .await
        .unwrap_or_else(|_| session.page().url());

    let locator = session.page().locator(selector).await;
    locator.click(None).await?;

    if wait_ms > 0 {
        tokio::time::sleep(Duration::from_millis(wait_ms)).await;
    }

    let after_url = session
        .page()
        .evaluate_value("window.location.href")
        .await
        .unwrap_or_else(|_| session.page().url());
    let navigated = before_url != after_url;

    // Collect any downloads that occurred during the click
    let downloads: Vec<DownloadedFile> = session
        .downloads()
        .into_iter()
        .map(|d| DownloadedFile {
            url: d.url,
            suggested_filename: d.suggested_filename,
            path: d.path,
        })
        .collect();

    let result = ResultBuilder::new("click")
        .inputs(CommandInputs {
            url: target.url_str().map(String::from),
            selector: Some(selector.to_string()),
            ..Default::default()
        })
        .data(ClickData {
            before_url,
            after_url: after_url.clone(),
            navigated,
            selector: selector.to_string(),
            downloads,
        })
        .build();

    print_result(&result, format);
    Ok(after_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_raw_deserialize() {
        let json = r#"{"url": "https://example.com", "selector": "button", "wait_ms": 1000}"#;
        let raw: ClickRaw = serde_json::from_str(json).unwrap();
        assert_eq!(raw.url, Some("https://example.com".into()));
        assert_eq!(raw.selector, Some("button".into()));
        assert_eq!(raw.wait_ms, Some(1000));
    }

    #[test]
    fn click_raw_default_wait_ms() {
        let json = r#"{"selector": "button"}"#;
        let raw: ClickRaw = serde_json::from_str(json).unwrap();
        assert_eq!(raw.wait_ms, None);
    }
}
