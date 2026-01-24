//! HTML content extraction command.

use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::args;
use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ContextDelta, ExecCtx};
use crate::error::Result;
use crate::output::CommandInputs;
use crate::session_broker::SessionRequest;
use crate::session_helpers::{ArtifactsPolicy, with_session};
use crate::target::{ResolveEnv, ResolvedTarget, TargetPolicy};

/// Raw inputs from CLI or batch JSON.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HtmlRaw {
	/// URL (positional argument).
	#[serde(default)]
	pub url: Option<String>,
	/// Selector (positional argument, may be detected from URL position).
	#[serde(default)]
	pub selector: Option<String>,
	/// URL via --url flag.
	#[serde(default, alias = "url_flag")]
	pub url_flag: Option<String>,
	/// Selector via --selector flag.
	#[serde(default, alias = "selector_flag")]
	pub selector_flag: Option<String>,
}

/// Resolved inputs ready for execution.
#[derive(Debug, Clone)]
pub struct HtmlResolved {
	/// Resolved navigation target.
	pub target: ResolvedTarget,
	/// Resolved CSS selector.
	pub selector: String,
}

/// Output data for HTML command.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HtmlData {
	pub html: String,
	pub selector: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub length: Option<usize>,
}

pub struct HtmlCommand;

impl CommandDef for HtmlCommand {
	const NAME: &'static str = "html";

	type Raw = HtmlRaw;
	type Resolved = HtmlResolved;
	type Data = HtmlData;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		// Smart detection: resolve positional vs flags
		let resolved = args::resolve_url_and_selector(
			raw.url.clone(),
			raw.url_flag,
			raw.selector_flag.or(raw.selector),
		);

		// Resolve target using typed target system
		let target = env.resolve_target(resolved.url, TargetPolicy::AllowCurrentPage)?;

		// Resolve selector with "html" as default (full page)
		let selector = env.resolve_selector(resolved.selector, Some("html"))?;

		Ok(HtmlResolved { target, selector })
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

			if args.selector == "html" {
				info!(target = "pw", url = %url_display, browser = %exec.ctx.browser, "get full page HTML");
			} else {
				info!(target = "pw", url = %url_display, selector = %args.selector, browser = %exec.ctx.browser, "get HTML for selector");
			}

			let preferred_url = args.target.preferred_url(exec.last_url);
			let timeout_ms = exec.ctx.timeout_ms();
			let target = args.target.target.clone();
			let selector = args.selector.clone();

			let req = SessionRequest::from_context(WaitUntil::NetworkIdle, exec.ctx)
				.with_preferred_url(preferred_url);

			let data = with_session(&mut exec, req, ArtifactsPolicy::Never, move |session| {
				let selector = selector.clone();
				Box::pin(async move {
					session.goto_target(&target, timeout_ms).await?;

					let locator = session.page().locator(&selector).await;
					let html = locator.inner_html().await?;

					Ok(HtmlData {
						length: Some(html.len()),
						html,
						selector,
					})
				})
			})
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
	fn html_raw_deserialize_from_json() {
		let json = r#"{"url": "https://example.com", "selector": "main"}"#;
		let raw: HtmlRaw = serde_json::from_str(json).unwrap();
		assert_eq!(raw.url, Some("https://example.com".into()));
		assert_eq!(raw.selector, Some("main".into()));
	}

	#[test]
	fn html_raw_deserialize_with_flags() {
		let json = r#"{"url_flag": "https://example.com", "selector_flag": ".content"}"#;
		let raw: HtmlRaw = serde_json::from_str(json).unwrap();
		assert_eq!(raw.url_flag, Some("https://example.com".into()));
		assert_eq!(raw.selector_flag, Some(".content".into()));
	}

	#[test]
	fn html_raw_deserialize_camel_case() {
		let json = r#"{"urlFlag": "https://example.com", "selectorFlag": ".content"}"#;
		let raw: HtmlRaw = serde_json::from_str(json).unwrap();
		assert_eq!(raw.url_flag, Some("https://example.com".into()));
		assert_eq!(raw.selector_flag, Some(".content".into()));
	}

}
