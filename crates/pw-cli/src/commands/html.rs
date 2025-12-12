use std::path::Path;

use crate::browser::BrowserSession;
use crate::error::Result;
use pw::WaitUntil;
use tracing::info;

pub async fn execute(url: &str, selector: &str, auth_file: Option<&Path>) -> Result<()> {
    if selector == "html" {
        info!(target = "pw", %url, "get full page HTML");
    } else {
        info!(target = "pw", %url, %selector, "get HTML for selector");
    }

    let session = BrowserSession::with_auth(WaitUntil::NetworkIdle, auth_file).await?;
    session.goto(url).await?;

    let locator = session.page().locator(selector).await;
    let html = locator.inner_html().await?;

    println!("{html}");
    session.close().await
}
