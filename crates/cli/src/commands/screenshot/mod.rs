//! Screenshot capture command.

use std::path::PathBuf;

use clap::Args;
use pw_rs::{ScreenshotOptions, WaitUntil};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::commands::contract::{resolve_target_from_url_pair, standard_delta, standard_inputs};
use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ExecCtx};
use crate::commands::exec_flow::navigation_plan;
use crate::error::Result;
use crate::output::ScreenshotData;
use crate::session_helpers::{ArtifactsPolicy, with_session};
use crate::target::{ResolveEnv, ResolvedTarget, TargetPolicy};

/// Raw inputs from CLI or batch JSON.
#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotRaw {
	/// Target URL (positional, uses context when omitted)
	#[serde(default)]
	pub url: Option<String>,

	/// Output file path (uses context or defaults when omitted)
	#[arg(short, long, value_name = "FILE")]
	#[serde(default)]
	pub output: Option<PathBuf>,

	/// Capture the full scrollable page instead of just the viewport
	#[arg(long)]
	#[serde(default, alias = "full_page")]
	pub full_page: Option<bool>,

	/// Target URL (named alternative)
	#[arg(long = "url", short = 'u', value_name = "URL")]
	#[serde(default, alias = "url_flag")]
	pub url_flag: Option<String>,
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
		let target = resolve_target_from_url_pair(raw.url, raw.url_flag, env, TargetPolicy::AllowCurrentPage)?;

		let output = raw.output.unwrap_or_else(|| PathBuf::from("screenshot.png"));
		let full_page = raw.full_page.unwrap_or(false);

		Ok(ScreenshotResolved { target, output, full_page })
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, mut exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
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

			let plan = navigation_plan(exec.ctx, exec.last_url, &args.target, WaitUntil::NetworkIdle);
			let timeout_ms = plan.timeout_ms;
			let target = plan.target;
			let output = args.output.clone();
			let full_page = args.full_page;

			with_session(&mut exec, plan.request, ArtifactsPolicy::Never, move |session| {
				let output = output.clone();
				Box::pin(async move {
					session.goto_target(&target, timeout_ms).await?;

					let screenshot_opts = ScreenshotOptions {
						full_page: Some(full_page),
						..Default::default()
					};

					session.page().screenshot_to_file(&output, Some(screenshot_opts)).await?;

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

			let inputs = standard_inputs(&args.target, None, None, Some(&args.output), None);

			Ok(CommandOutcome {
				inputs,
				data,
				delta: standard_delta(&args.target, None, Some(&args.output)),
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
