//! Fill form element command.
//!
//! Fills a form input element with text. Supports text inputs, textareas,
//! and contenteditable elements.
//!
//! # Example
//!
//! ```bash
//! pw fill --selector "input[name=email]" --text "user@example.com"
//! ```

use clap::Args;
use pw_rs::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::commands::contract::{resolve_target_from_url_pair, standard_delta, standard_inputs};
use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ExecCtx};
use crate::commands::exec_flow::navigation_plan;
use crate::error::Result;
use crate::output::FillData;
use crate::session_helpers::{ArtifactsPolicy, with_session};
use crate::target::{ResolveEnv, ResolvedTarget, TargetPolicy};

/// Raw inputs from CLI or batch JSON before resolution.
#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FillRaw {
	/// Text to fill into the input
	pub text: Option<String>,

	/// CSS selector for the input element
	#[arg(long = "selector", short = 's', value_name = "SELECTOR")]
	#[serde(default)]
	pub selector: Option<String>,

	/// Target URL (named alternative)
	#[arg(long = "url", short = 'u', value_name = "URL")]
	#[serde(default)]
	pub url: Option<String>,
}

/// Resolved inputs ready for execution.
#[derive(Debug, Clone)]
pub struct FillResolved {
	/// Navigation target (URL or current page).
	pub target: ResolvedTarget,

	/// CSS selector for the target element.
	pub selector: String,

	/// Text to fill into the element.
	pub text: String,
}

pub struct FillCommand;

impl CommandDef for FillCommand {
	const NAME: &'static str = "fill";

	type Raw = FillRaw;
	type Resolved = FillResolved;
	type Data = FillData;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		let target = resolve_target_from_url_pair(raw.url, None, env, TargetPolicy::AllowCurrentPage)?;
		let selector = env.resolve_selector(raw.selector, None)?;
		let text = raw.text.unwrap_or_default();

		Ok(FillResolved { target, selector, text })
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, mut exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let url_display = args.target.url_str().unwrap_or("<current page>");
			info!(target = "pw", url = %url_display, selector = %args.selector, "fill");

			let plan = navigation_plan(exec.ctx, exec.last_url, &args.target, WaitUntil::Load);
			let timeout_ms = plan.timeout_ms;
			let target = plan.target;
			let selector = args.selector.clone();
			let text = args.text.clone();

			let data = with_session(&mut exec, plan.request, ArtifactsPolicy::OnError { command: "fill" }, move |session| {
				let selector = selector.clone();
				let text = text.clone();
				Box::pin(async move {
					session.goto_target(&target, timeout_ms).await?;

					let locator = session.page().locator(&selector).await;
					locator.fill(&text, None).await?;

					Ok(FillData { selector, text })
				})
			})
			.await?;

			let inputs = standard_inputs(&args.target, Some(&args.selector), None, None, None);

			Ok(CommandOutcome {
				inputs,
				data,
				delta: standard_delta(&args.target, Some(&args.selector), None),
			})
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn fill_raw_deserialize_from_json() {
		let json = r#"{"url": "https://example.com", "selector": "input", "text": "hello"}"#;
		let raw: FillRaw = serde_json::from_str(json).unwrap();
		assert_eq!(raw.url, Some("https://example.com".into()));
		assert_eq!(raw.selector, Some("input".into()));
		assert_eq!(raw.text, Some("hello".into()));
	}
}
