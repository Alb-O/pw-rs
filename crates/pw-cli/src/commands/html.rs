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
    if selector == "html" {
        info!(target = "pw", %url, browser = %ctx.browser, "get full page HTML");
    } else {
        info!(target = "pw", %url, %selector, browser = %ctx.browser, "get HTML for selector");
    }

    let session = broker
        .session(SessionRequest::from_context(WaitUntil::NetworkIdle, ctx))
        .await?;
    session.goto(url).await?;

    let locator = session.page().locator(selector).await;
    let html = locator.inner_html().await?;

    println!("{html}");
    session.close().await
}
