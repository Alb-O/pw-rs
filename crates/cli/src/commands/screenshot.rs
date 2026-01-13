//! Screenshot capture command.

use std::path::PathBuf;
use std::time::Instant;

use crate::context::CommandContext;
use crate::error::Result;
use crate::output::{
    Artifact, ArtifactType, CommandInputs, OutputFormat, ResultBuilder, ScreenshotData,
    print_result,
};
use crate::session_broker::{SessionBroker, SessionRequest};
use crate::target::{Resolve, ResolveEnv, ResolvedTarget, TargetPolicy};
use pw::{ScreenshotOptions, WaitUntil};
use serde::{Deserialize, Serialize};
use tracing::info;

// ---------------------------------------------------------------------------
// Raw and Resolved Types
// ---------------------------------------------------------------------------

/// Raw inputs from CLI or batch JSON.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotRaw {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default, alias = "url_flag")]
    pub url_flag: Option<String>,
    #[serde(default)]
    pub output: Option<PathBuf>,
    #[serde(default, alias = "full_page")]
    pub full_page: Option<bool>,
}

impl ScreenshotRaw {
    pub fn from_cli(
        url: Option<String>,
        url_flag: Option<String>,
        output: Option<PathBuf>,
        full_page: bool,
    ) -> Self {
        Self {
            url,
            url_flag,
            output,
            full_page: Some(full_page),
        }
    }
}

/// Resolved inputs ready for execution.
#[derive(Debug, Clone)]
pub struct ScreenshotResolved {
    pub target: ResolvedTarget,
    pub output: PathBuf,
    pub full_page: bool,
}

impl ScreenshotResolved {
    pub fn preferred_url<'a>(&'a self, last_url: Option<&'a str>) -> Option<&'a str> {
        self.target.preferred_url(last_url)
    }
}

impl Resolve for ScreenshotRaw {
    type Output = ScreenshotResolved;

    fn resolve(self, env: &ResolveEnv<'_>) -> Result<ScreenshotResolved> {
        let url = self.url_flag.or(self.url);
        let target = env.resolve_target(url, TargetPolicy::AllowCurrentPage)?;

        // Output path resolution is handled by ContextState in the dispatcher
        // For now, use a default if not provided
        let output = self
            .output
            .unwrap_or_else(|| PathBuf::from("screenshot.png"));
        let full_page = self.full_page.unwrap_or(false);

        Ok(ScreenshotResolved {
            target,
            output,
            full_page,
        })
    }
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

/// Execute screenshot with resolved arguments.
pub async fn execute_resolved(
    args: &ScreenshotResolved,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    last_url: Option<&str>,
) -> Result<()> {
    let _start = Instant::now();
    let url_display = args.target.url_str().unwrap_or("<current page>");
    info!(target = "pw", url = %url_display, path = %args.output.display(), full_page = %args.full_page, browser = %ctx.browser, "screenshot");

    let preferred_url = args.preferred_url(last_url);
    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(preferred_url),
        )
        .await?;
    session.goto_target(&args.target.target).await?;

    if let Some(parent) = args.output.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let screenshot_opts = ScreenshotOptions {
        full_page: Some(args.full_page),
        ..Default::default()
    };

    session
        .page()
        .screenshot_to_file(&args.output, Some(screenshot_opts))
        .await?;

    let size_bytes = std::fs::metadata(&args.output).ok().map(|m| m.len());

    let result = ResultBuilder::new("screenshot")
        .inputs(CommandInputs {
            url: args.target.url_str().map(String::from),
            output_path: Some(args.output.clone()),
            ..Default::default()
        })
        .data(ScreenshotData {
            path: args.output.clone(),
            full_page: args.full_page,
            width: None,
            height: None,
        })
        .artifact(Artifact {
            artifact_type: ArtifactType::Screenshot,
            path: args.output.clone(),
            size_bytes,
        })
        .build();

    print_result(&result, format);
    session.close().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screenshot_raw_deserialize() {
        let json = r#"{"url": "https://example.com", "output": "test.png", "full_page": true}"#;
        let raw: ScreenshotRaw = serde_json::from_str(json).unwrap();
        assert_eq!(raw.url, Some("https://example.com".into()));
        assert_eq!(raw.output, Some(PathBuf::from("test.png")));
        assert_eq!(raw.full_page, Some(true));
    }
}
