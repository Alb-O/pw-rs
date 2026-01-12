use std::path::Path;
use std::time::Instant;

use crate::context::CommandContext;
use crate::error::Result;
use crate::output::{
    Artifact, ArtifactType, CommandInputs, OutputFormat, ResultBuilder, ScreenshotData,
    print_result,
};
use crate::session_broker::{SessionBroker, SessionRequest};
use pw::{ScreenshotOptions, WaitUntil};
use tracing::info;

pub async fn execute(
    url: &str,
    output: &Path,
    full_page: bool,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    preferred_url: Option<&str>,
) -> Result<()> {
    let _start = Instant::now();
    let output = output.to_path_buf();

    info!(target = "pw", %url, path = %output.display(), full_page, browser = %ctx.browser, "screenshot");

    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(preferred_url),
        )
        .await?;
    session.goto_unless_current(url).await?;

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

    // Get file size for artifact info
    let size_bytes = std::fs::metadata(&output).ok().map(|m| m.len());

    let result = ResultBuilder::new("screenshot")
        .inputs(CommandInputs {
            url: Some(url.to_string()),
            output_path: Some(output.clone()),
            ..Default::default()
        })
        .data(ScreenshotData {
            path: output.clone(),
            full_page,
            width: None, // TODO: Could extract from screenshot metadata
            height: None,
        })
        .artifact(Artifact {
            artifact_type: ArtifactType::Screenshot,
            path: output,
            size_bytes,
        })
        .build();

    print_result(&result, format);

    session.close().await
}
