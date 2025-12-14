use std::time::Duration;

use crate::context::CommandContext;
use crate::error::Result;
use crate::session_broker::{SessionBroker, SessionRequest};
use pw::WaitUntil;
use tracing::info;

pub async fn execute(
    url: &str,
    selector: &str,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
) -> Result<()> {
    info!(target = "pw", %url, %selector, browser = %ctx.browser, "click element");

    let session = broker
        .session(SessionRequest::from_context(WaitUntil::NetworkIdle, ctx))
        .await?;
    session.goto(url).await?;

    let locator = session.page().locator(selector).await;
    locator.click(None).await?;

    tokio::time::sleep(Duration::from_millis(1000)).await;

    let final_url = session.page().url();
    println!("Navigated to: {final_url}");

    session.close().await
}
