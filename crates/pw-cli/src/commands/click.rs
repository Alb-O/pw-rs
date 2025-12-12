use std::path::Path;
use std::time::Duration;

use crate::browser::BrowserSession;
use crate::error::Result;
use pw::WaitUntil;
use tracing::info;

pub async fn execute(url: &str, selector: &str, auth_file: Option<&Path>) -> Result<()> {
    info!(target = "pw", %url, %selector, "click element");

    let session = BrowserSession::with_auth(WaitUntil::NetworkIdle, auth_file).await?;
    session.goto(url).await?;

    let locator = session.page().locator(selector).await;
    locator.click(None).await?;

    tokio::time::sleep(Duration::from_millis(1000)).await;

    let final_url = session.page().url();
    println!("Navigated to: {final_url}");

    session.close().await
}
