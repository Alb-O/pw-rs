use std::path::Path;

use crate::browser::BrowserSession;
use crate::error::Result;
use pw::{ScreenshotOptions, WaitUntil};
use tracing::info;

pub async fn execute(url: &str, output: &Path) -> Result<()> {
    info!(target = "pw", %url, path = %output.display(), "screenshot");

    let session = BrowserSession::new(WaitUntil::NetworkIdle).await?;
    session.goto(url).await?;

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let screenshot_opts = ScreenshotOptions {
        full_page: Some(true),
        ..Default::default()
    };

    session
        .page()
        .screenshot_to_file(output, Some(screenshot_opts))
        .await?;

    info!(target = "pw", path = %output.display(), "screenshot saved");
    session.close().await
}
