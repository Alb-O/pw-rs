use crate::browser::BrowserSession;
use crate::error::Result;
use pw::WaitUntil;
use tracing::info;

pub async fn execute(url: &str, selector: &str) -> Result<()> {
    if selector == "html" {
        info!(target = "pw", %url, "get full page HTML");
    } else {
        info!(target = "pw", %url, %selector, "get HTML for selector");
    }

    let session = BrowserSession::new(WaitUntil::NetworkIdle).await?;
    session.goto(url).await?;

    let locator = session.page().locator(selector).await;
    let html = locator.inner_html().await?;

    println!("{html}");
    session.close().await
}
