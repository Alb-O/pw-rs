use crate::browser::BrowserSession;
use crate::error::Result;
use pw::WaitUntil;
use tracing::info;

pub async fn execute(url: &str, selector: &str) -> Result<()> {
    info!(target = "pw", %url, %selector, "get text");

    let session = BrowserSession::new(WaitUntil::NetworkIdle).await?;
    session.goto(url).await?;

    let locator = session.page().locator(selector).await;
    let text = locator.text_content().await?.unwrap_or_default();

    println!("{}", text.trim());
    session.close().await
}
