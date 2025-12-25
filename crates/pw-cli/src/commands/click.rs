use std::time::{Duration, Instant};

use crate::context::CommandContext;
use crate::error::Result;
use crate::output::{
    ClickData, CommandInputs, OutputFormat, ResultBuilder, print_result,
};
use crate::session_broker::{SessionBroker, SessionRequest};
use pw::WaitUntil;
use tracing::info;

pub async fn execute(
    url: &str,
    selector: &str,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
) -> Result<()> {
    let _start = Instant::now();
    info!(target = "pw", %url, %selector, browser = %ctx.browser, "click element");

    let session = broker
        .session(SessionRequest::from_context(WaitUntil::NetworkIdle, ctx))
        .await?;
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

    session.close().await
}
