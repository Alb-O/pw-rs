//! Click element command.

use std::time::Duration;

use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::args;
use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ContextDelta, ExecCtx};
use crate::error::Result;
use crate::output::{ClickData, CommandInputs, DownloadedFile};
use crate::session_broker::SessionRequest;
use crate::session_helpers::{ArtifactsPolicy, with_session};
use crate::target::{ResolveEnv, ResolvedTarget, TargetPolicy};

/// Raw inputs from CLI or batch JSON.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClickRaw {
	#[serde(default)]
	pub url: Option<String>,
	#[serde(default)]
	pub selector: Option<String>,
	#[serde(default, alias = "url_flag")]
	pub url_flag: Option<String>,
	#[serde(default, alias = "selector_flag")]
	pub selector_flag: Option<String>,
	#[serde(default, alias = "wait_ms")]
	pub wait_ms: Option<u64>,
}

/// Resolved inputs ready for execution.
#[derive(Debug, Clone)]
pub struct ClickResolved {
	pub target: ResolvedTarget,
	pub selector: String,
	pub wait_ms: u64,
}

pub struct ClickCommand;

impl CommandDef for ClickCommand {
	const NAME: &'static str = "click";

	type Raw = ClickRaw;
	type Resolved = ClickResolved;
	type Data = ClickData;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		let resolved = args::resolve_url_and_selector(
			raw.url.clone(),
			raw.url_flag,
			raw.selector_flag.or(raw.selector),
		);

		let target = env.resolve_target(resolved.url, TargetPolicy::AllowCurrentPage)?;
		let selector = env.resolve_selector(resolved.selector, Some("css=button"))?;
		let wait_ms = raw.wait_ms.unwrap_or(0);

		Ok(ClickResolved {
			target,
			selector,
			wait_ms,
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
			info!(target = "pw", url = %url_display, selector = %args.selector, browser = %exec.ctx.browser, "click element");

			let preferred_url = args.target.preferred_url(exec.last_url);
			let timeout_ms = exec.ctx.timeout_ms();
			let target = args.target.target.clone();
			let selector = args.selector.clone();
			let selector_for_outcome = selector.clone();
			let wait_ms = args.wait_ms;

			let req = SessionRequest::from_context(WaitUntil::NetworkIdle, exec.ctx)
				.with_preferred_url(preferred_url);

			let (after_url, data) = with_session(
				&mut exec,
				req,
				ArtifactsPolicy::OnError { command: "click" },
				move |session| {
					let selector = selector.clone();
					Box::pin(async move {
						session.goto_target(&target, timeout_ms).await?;

						let before_url = session
							.page()
							.evaluate_value("window.location.href")
							.await
							.unwrap_or_else(|_| session.page().url());

						let locator = session.page().locator(&selector).await;
						locator.click(None).await?;

						if wait_ms > 0 {
							tokio::time::sleep(Duration::from_millis(wait_ms)).await;
						}

						let after_url = session
							.page()
							.evaluate_value("window.location.href")
							.await
							.unwrap_or_else(|_| session.page().url());

						let navigated = before_url != after_url;

						let downloads: Vec<DownloadedFile> = session
							.downloads()
							.into_iter()
							.map(|d| DownloadedFile {
								url: d.url,
								suggested_filename: d.suggested_filename,
								path: d.path,
							})
							.collect();

						let data = ClickData {
							before_url,
							after_url: after_url.clone(),
							navigated,
							selector: selector.clone(),
							downloads,
						};

						Ok((after_url, data))
					})
				},
			)
			.await?;

			let inputs = CommandInputs {
				url: args.target.url_str().map(String::from),
				selector: Some(selector_for_outcome.clone()),
				..Default::default()
			};

			Ok(CommandOutcome {
				inputs,
				data,
				delta: ContextDelta {
					url: Some(after_url),
					selector: Some(selector_for_outcome),
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
	fn click_raw_deserialize() {
		let json = r#"{"url": "https://example.com", "selector": "button", "wait_ms": 1000}"#;
		let raw: ClickRaw = serde_json::from_str(json).unwrap();
		assert_eq!(raw.url, Some("https://example.com".into()));
		assert_eq!(raw.selector, Some("button".into()));
		assert_eq!(raw.wait_ms, Some(1000));
	}

	#[test]
	fn click_raw_default_wait_ms() {
		let json = r#"{"selector": "button"}"#;
		let raw: ClickRaw = serde_json::from_str(json).unwrap();
		assert_eq!(raw.wait_ms, None);
	}
}
