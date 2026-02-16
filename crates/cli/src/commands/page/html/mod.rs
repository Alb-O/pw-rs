//! HTML content extraction command.

use clap::Args;
use pw_rs::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::commands::contract::{resolve_target_and_selector, standard_delta, standard_inputs};
use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ExecCtx};
use crate::commands::exec_flow::navigation_plan;
use crate::error::Result;
use crate::session_helpers::{ArtifactsPolicy, with_session};
use crate::target::{ResolveEnv, ResolvedTarget};

/// Raw inputs from CLI or batch JSON.
#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HtmlRaw {
	/// Target URL (positional, uses context when omitted)
	#[serde(default)]
	pub url: Option<String>,

	/// CSS selector (positional, uses last selector or defaults to html)
	#[serde(default)]
	pub selector: Option<String>,

	/// Target URL (named alternative)
	#[arg(long = "url", short = 'u', value_name = "URL")]
	#[serde(default, alias = "url_flag")]
	pub url_flag: Option<String>,

	/// CSS selector (named alternative)
	#[arg(long = "selector", short = 's', value_name = "SELECTOR")]
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
	const NAME: &'static str = "page.html";

	type Raw = HtmlRaw;
	type Resolved = HtmlResolved;
	type Data = HtmlData;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		let (target, selector) = resolve_target_and_selector(raw.url, raw.selector, raw.url_flag, raw.selector_flag, env, Some("html"))?;

		Ok(HtmlResolved { target, selector })
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, mut exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
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

			let plan = navigation_plan(exec.ctx, exec.last_url, &args.target, WaitUntil::NetworkIdle);
			let timeout_ms = plan.timeout_ms;
			let target = plan.target;
			let selector = args.selector.clone();

			let data = with_session(&mut exec, plan.request, ArtifactsPolicy::Never, move |session| {
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
