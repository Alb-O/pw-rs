use std::path::Path;

use crate::browser::BrowserSession;
use crate::error::Result;
use pw::WaitUntil;
use tracing::info;

pub async fn execute(url: &str, selector: &str, auth_file: Option<&Path>) -> Result<()> {
    info!(target = "pw", %url, %selector, "get text");

    let session = BrowserSession::with_auth(WaitUntil::NetworkIdle, auth_file).await?;
    session.goto(url).await?;

    let locator = session.page().locator(selector).await;
    let text = locator.text_content().await?.unwrap_or_default();

    println!("{}", text.trim());
    session.close().await
}
