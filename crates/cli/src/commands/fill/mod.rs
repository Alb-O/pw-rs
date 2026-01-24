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

use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ContextDelta, ExecCtx};
use crate::error::Result;
use crate::output::{CommandInputs, FillData};
use crate::session_broker::SessionRequest;
use crate::session_helpers::{ArtifactsPolicy, with_session};
use crate::target::{ResolveEnv, ResolvedTarget, TargetPolicy};

/// Raw inputs from CLI or batch JSON before resolution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FillRaw {
	/// Target URL, resolved from context if not provided.
	#[serde(default)]
	pub url: Option<String>,

	/// CSS selector for the element to fill.
	#[serde(default)]
	pub selector: Option<String>,

	/// Text to fill into the element.
	#[serde(default)]
	pub text: Option<String>,
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
		let target = env.resolve_target(raw.url, TargetPolicy::AllowCurrentPage)?;
		let selector = env.resolve_selector(raw.selector, None)?;
		let text = raw.text.unwrap_or_default();

		Ok(FillResolved {
			target,
			selector,
			text,
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
			info!(target = "pw", url = %url_display, selector = %args.selector, "fill");

			let preferred_url = args.target.preferred_url(exec.last_url);
			let timeout_ms = exec.ctx.timeout_ms();
			let target = args.target.target.clone();
			let selector = args.selector.clone();
			let text = args.text.clone();

			let req = SessionRequest::from_context(WaitUntil::Load, exec.ctx)
				.with_preferred_url(preferred_url);

			let data = with_session(
				&mut exec,
				req,
				ArtifactsPolicy::OnError { command: "fill" },
				move |session| {
					let selector = selector.clone();
					let text = text.clone();
					Box::pin(async move {
						session.goto_target(&target, timeout_ms).await?;

						let locator = session.page().locator(&selector).await;
						locator.fill(&text, None).await?;

						Ok(FillData { selector, text })
					})
				},
			)
			.await?;

			let inputs = CommandInputs {
				url: args.target.url_str().map(String::from),
				selector: Some(args.selector.clone()),
				..Default::default()
			};

			Ok(CommandOutcome {
				inputs,
				data,
				delta: ContextDelta {
					url: args.target.url_str().map(String::from),
					selector: Some(args.selector.clone()),
					output: None,
				},
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
