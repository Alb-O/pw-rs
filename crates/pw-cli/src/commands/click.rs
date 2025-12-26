use std::path::Path;
use std::time::{Duration, Instant};

use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::output::{
    ClickData, CommandInputs, FailureWithArtifacts, OutputFormat, ResultBuilder,
    print_failure_with_artifacts, print_result,
};
use crate::session_broker::{SessionBroker, SessionHandle, SessionRequest};
use pw::WaitUntil;
use tracing::info;

pub async fn execute(
    url: &str,
    selector: &str,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    artifacts_dir: Option<&Path>,
) -> Result<()> {
    let _start = Instant::now();
    info!(target = "pw", %url, %selector, browser = %ctx.browser, "click element");

    let session = broker
        .session(SessionRequest::from_context(WaitUntil::NetworkIdle, ctx))
        .await?;

    match execute_inner(&session, url, selector, format).await {
        Ok(()) => session.close().await,
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
    format: OutputFormat,
) -> Result<()> {
    session.goto(url).await?;

    // Record URL before click
    let before_url = session.page().url();

    // Get the locator and click
    let locator = session.page().locator(selector).await;
    locator.click(None).await?;

    // Wait briefly for potential navigation
    // TODO: Use proper wait_for_navigation when available in pw-core
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Record URL after click
    let after_url = session.page().url();
    let navigated = before_url != after_url;

    let result = ResultBuilder::new("click")
        .inputs(CommandInputs {
            url: Some(url.to_string()),
            selector: Some(selector.to_string()),
            ..Default::default()
        })
        .data(ClickData {
            before_url,
            after_url,
            navigated,
            selector: selector.to_string(),
        })
        .build();

    print_result(&result, format);
    Ok(())
}
