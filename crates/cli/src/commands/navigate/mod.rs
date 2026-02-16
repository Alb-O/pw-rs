//! Navigation command.

use clap::Args;
use pw_rs::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::commands::contract::{resolve_target_from_url_pair, standard_delta_with_url, standard_inputs};
use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ExecCtx};
use crate::commands::exec_flow::navigation_plan;
use crate::commands::page::snapshot::{EXTRACT_ELEMENTS_JS, EXTRACT_META_JS, EXTRACT_TEXT_JS, PageMeta, RawElement};
use crate::error::Result;
use crate::output::{InteractiveElement, SnapshotData};
use crate::target::{ResolveEnv, ResolvedTarget, Target, TargetPolicy};

const DEFAULT_MAX_TEXT_LENGTH: usize = 5000;

/// Raw inputs from CLI or batch JSON.
#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigateRaw {
	/// Target URL (positional, uses context when omitted)
	#[serde(default)]
	pub url: Option<String>,

	/// Target URL (named alternative)
	#[arg(long = "url", short = 'u', value_name = "URL")]
	#[serde(default, alias = "url_flag")]
	pub url_flag: Option<String>,
}

/// Resolved inputs ready for execution.
#[derive(Debug, Clone)]
pub struct NavigateResolved {
	pub target: ResolvedTarget,
}

pub struct NavigateCommand;

impl CommandDef for NavigateCommand {
	const NAME: &'static str = "navigate";

	type Raw = NavigateRaw;
	type Resolved = NavigateResolved;
	type Data = SnapshotData;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		let target = resolve_target_from_url_pair(raw.url, raw.url_flag, env, TargetPolicy::AllowCurrentPage)?;
		Ok(NavigateResolved { target })
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let url_display = args.target.url_str().unwrap_or("<current page>");
			info!(target = "pw", url = %url_display, browser = %exec.ctx.browser, "navigate");

			let plan = navigation_plan(exec.ctx, exec.last_url, &args.target, WaitUntil::Load);
			let session = exec.broker.session(plan.request).await?;

			match &plan.target {
				Target::Navigate(url) => {
					session.goto_if_needed(url.as_str(), exec.ctx.timeout_ms()).await?;
				}
				Target::CurrentPage => {}
			}

			session.page().bring_to_front().await?;

			let meta_js = format!("JSON.stringify({})", EXTRACT_META_JS);
			let meta: PageMeta = serde_json::from_str(&session.page().evaluate_value(&meta_js).await?)?;

			let text_js = format!("JSON.stringify({}({}, {}))", EXTRACT_TEXT_JS, DEFAULT_MAX_TEXT_LENGTH, false);
			let text: String = serde_json::from_str(&session.page().evaluate_value(&text_js).await?)?;

			let elements_js = format!("JSON.stringify({})", EXTRACT_ELEMENTS_JS);
			let raw_elements: Vec<RawElement> = serde_json::from_str(&session.page().evaluate_value(&elements_js).await?)?;

			let elements: Vec<InteractiveElement> = raw_elements.into_iter().map(Into::into).collect();
			let element_count = elements.len();

			let data = SnapshotData {
				url: meta.url.clone(),
				title: meta.title,
				viewport_width: meta.viewport_width,
				viewport_height: meta.viewport_height,
				text,
				elements,
				element_count,
			};

			let inputs = standard_inputs(&args.target, None, None, None, None);

			session.close().await?;

			Ok(CommandOutcome {
				inputs,
				data,
				delta: standard_delta_with_url(Some(meta.url), None, None),
			})
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn navigate_raw_deserialize() {
		let json = r#"{"url": "https://example.com"}"#;
		let raw: NavigateRaw = serde_json::from_str(json).unwrap();
		assert_eq!(raw.url, Some("https://example.com".into()));
	}
}
