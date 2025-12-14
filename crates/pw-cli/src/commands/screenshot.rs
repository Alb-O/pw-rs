use std::path::Path;

use crate::context::CommandContext;
use crate::error::Result;
use crate::session_broker::{SessionBroker, SessionRequest};
use pw::{ScreenshotOptions, WaitUntil};
use tracing::info;

pub async fn execute(
    url: &str,
    output: &Path,
    full_page: bool,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
) -> Result<()> {
    let output = output.to_path_buf();

    info!(target = "pw", %url, path = %output.display(), full_page, browser = %ctx.browser, "screenshot");

    let session = broker
        .session(SessionRequest::from_context(WaitUntil::NetworkIdle, ctx))
        .await?;
    session.goto(url).await?;

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let screenshot_opts = ScreenshotOptions {
        full_page: Some(full_page),
        ..Default::default()
    };

    session
        .page()
        .screenshot_to_file(&output, Some(screenshot_opts))
        .await?;

    info!(target = "pw", path = %output.display(), "screenshot saved");
    session.close().await
}
