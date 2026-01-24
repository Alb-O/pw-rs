//! Screenshot capture command.

use std::path::PathBuf;

use pw::{ScreenshotOptions, WaitUntil};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ContextDelta, ExecCtx};
use crate::error::Result;
use crate::output::{CommandInputs, ScreenshotData};
use crate::session_broker::SessionRequest;
use crate::session_helpers::{ArtifactsPolicy, with_session};
use crate::target::{ResolveEnv, ResolvedTarget, TargetPolicy};

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

/// Resolved inputs ready for execution.
#[derive(Debug, Clone)]
pub struct ScreenshotResolved {
	pub target: ResolvedTarget,
	pub output: PathBuf,
	pub full_page: bool,
}

pub struct ScreenshotCommand;

impl CommandDef for ScreenshotCommand {
	const NAME: &'static str = "screenshot";

	type Raw = ScreenshotRaw;
	type Resolved = ScreenshotResolved;
	type Data = ScreenshotData;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		let url = raw.url_flag.or(raw.url);
		let target = env.resolve_target(url, TargetPolicy::AllowCurrentPage)?;

		// Output path resolution is handled by ContextState in the dispatcher
		let output = raw
			.output
			.unwrap_or_else(|| PathBuf::from("screenshot.png"));
		let full_page = raw.full_page.unwrap_or(false);

		Ok(ScreenshotResolved {
			target,
			output,
			full_page,
		})
	}

	fn execute<'exec, 'ctx>(
		args: &'exec Self::Resolved,
		mut exec: ExecCtx<'exec, 'ctx>,
	) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let url_display = args.target.url_str().unwrap_or("<current page>");
			info!(
				target = "pw",
				url = %url_display,
				path = %args.output.display(),
				full_page = %args.full_page,
				browser = %exec.ctx.browser,
				"screenshot"
			);

			if let Some(parent) = args.output.parent() {
				if !parent.as_os_str().is_empty() && !parent.exists() {
					std::fs::create_dir_all(parent)?;
				}
			}

			let preferred_url = args.target.preferred_url(exec.last_url);
			let timeout_ms = exec.ctx.timeout_ms();
			let target = args.target.target.clone();
			let output = args.output.clone();
			let full_page = args.full_page;

			let req = SessionRequest::from_context(WaitUntil::NetworkIdle, exec.ctx)
				.with_preferred_url(preferred_url);

			with_session(&mut exec, req, ArtifactsPolicy::Never, move |session| {
				let output = output.clone();
				Box::pin(async move {
					session.goto_target(&target, timeout_ms).await?;

					let screenshot_opts = ScreenshotOptions {
						full_page: Some(full_page),
						..Default::default()
					};

					session
						.page()
						.screenshot_to_file(&output, Some(screenshot_opts))
						.await?;

					Ok(())
				})
			})
			.await?;

			let data = ScreenshotData {
				path: args.output.clone(),
				full_page: args.full_page,
				width: None,
				height: None,
			};

			let inputs = CommandInputs {
				url: args.target.url_str().map(String::from),
				output_path: Some(args.output.clone()),
				..Default::default()
			};

			Ok(CommandOutcome {
				inputs,
				data,
				delta: ContextDelta {
					url: args.target.url_str().map(String::from),
					selector: None,
					output: Some(args.output.clone()),
				},
			})
		})
	}
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
