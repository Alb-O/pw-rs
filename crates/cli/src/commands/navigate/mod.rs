//! Navigation command.

use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ContextDelta, ExecCtx};
use crate::commands::page::snapshot::{
	EXTRACT_ELEMENTS_JS, EXTRACT_META_JS, EXTRACT_TEXT_JS, PageMeta, RawElement,
};
use crate::error::Result;
use crate::output::{CommandInputs, InteractiveElement, SnapshotData};
use crate::session_broker::SessionRequest;
use crate::target::{ResolveEnv, ResolvedTarget, Target, TargetPolicy};

const DEFAULT_MAX_TEXT_LENGTH: usize = 5000;

/// Raw inputs from CLI or batch JSON.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigateRaw {
	#[serde(default)]
	pub url: Option<String>,
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
		let url = raw.url_flag.or(raw.url);
		let target = env.resolve_target(url, TargetPolicy::AllowCurrentPage)?;
		Ok(NavigateResolved { target })
	}

	fn execute<'exec, 'ctx>(
		args: &'exec Self::Resolved,
		exec: ExecCtx<'exec, 'ctx>,
	) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let url_display = args.target.url_str().unwrap_or("<current page>");
			info!(target = "pw", url = %url_display, browser = %exec.ctx.browser, "navigate");

			let preferred_url = args.target.preferred_url(exec.last_url);

			let req = SessionRequest::from_context(WaitUntil::Load, exec.ctx)
				.with_preferred_url(preferred_url);

			let session = exec.broker.session(req).await?;

			match &args.target.target {
				Target::Navigate(url) => {
					session
						.goto_if_needed(url.as_str(), exec.ctx.timeout_ms())
						.await?;
				}
				Target::CurrentPage => {}
			}

			session.page().bring_to_front().await?;

			let meta_js = format!("JSON.stringify({})", EXTRACT_META_JS);
			let meta: PageMeta =
				serde_json::from_str(&session.page().evaluate_value(&meta_js).await?)?;

			let text_js = format!(
				"JSON.stringify({}({}, {}))",
				EXTRACT_TEXT_JS, DEFAULT_MAX_TEXT_LENGTH, false
			);
			let text: String =
				serde_json::from_str(&session.page().evaluate_value(&text_js).await?)?;

			let elements_js = format!("JSON.stringify({})", EXTRACT_ELEMENTS_JS);
			let raw_elements: Vec<RawElement> =
				serde_json::from_str(&session.page().evaluate_value(&elements_js).await?)?;

			let elements: Vec<InteractiveElement> =
				raw_elements.into_iter().map(Into::into).collect();
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

			let inputs = CommandInputs {
				url: args.target.url_str().map(String::from),
				..Default::default()
			};

			session.close().await?;

			Ok(CommandOutcome {
				inputs,
				data,
				delta: ContextDelta {
					url: Some(meta.url),
					selector: None,
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
	fn navigate_raw_deserialize() {
		let json = r#"{"url": "https://example.com"}"#;
		let raw: NavigateRaw = serde_json::from_str(json).unwrap();
		assert_eq!(raw.url, Some("https://example.com".into()));
	}
}
