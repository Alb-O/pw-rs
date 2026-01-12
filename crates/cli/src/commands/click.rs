use std::path::Path;
use std::time::Duration;

use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::output::{
    ClickData, CommandInputs, FailureWithArtifacts, OutputFormat, ResultBuilder,
    print_failure_with_artifacts, print_result,
};
use crate::session_broker::{SessionBroker, SessionHandle, SessionRequest};
use pw::WaitUntil;
use tracing::info;

/// Execute click and return the actual browser URL after the click.
///
/// The returned URL is the page's location after the click (may differ if click triggered navigation).
pub async fn execute(
    url: &str,
    selector: &str,
    wait_ms: u64,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    artifacts_dir: Option<&Path>,
    preferred_url: Option<&str>,
) -> Result<String> {
    info!(target = "pw", %url, %selector, browser = %ctx.browser, "click element");

    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(preferred_url),
        )
        .await?;

    match execute_inner(&session, url, selector, wait_ms, format).await {
        Ok(after_url) => {
            session.close().await?;
            Ok(after_url)
        }
        Err(e) => {
            // Collect artifacts on failure if artifacts_dir is set
            let artifacts = session
                .collect_failure_artifacts(artifacts_dir, "click")
                .await;

            if !artifacts.is_empty() {
                // Print failure with artifacts and signal that output is complete
                let failure = FailureWithArtifacts::new(e.to_command_error())
                    .with_artifacts(artifacts.artifacts);
                print_failure_with_artifacts("click", &failure, format);
                let _ = session.close().await;
                return Err(PwError::OutputAlreadyPrinted);
            }

            // No artifacts collected, propagate original error
            let _ = session.close().await;
            Err(e)
        }
    }
}

async fn execute_inner(
    session: &SessionHandle,
    url: &str,
    selector: &str,
    wait_ms: u64,
    format: OutputFormat,
) -> Result<String> {
    session.goto_unless_current(url).await?;

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

    let result = ResultBuilder::new("click")
        .inputs(CommandInputs {
            url: Some(url.to_string()),
            selector: Some(selector.to_string()),
            ..Default::default()
        })
        .data(ClickData {
            before_url,
            after_url: after_url.clone(),
            navigated,
            selector: selector.to_string(),
        })
        .build();

    print_result(&result, format);
    Ok(after_url)
}
