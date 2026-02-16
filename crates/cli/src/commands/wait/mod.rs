//! Wait command for various conditions.
//!
//! Waits for a specified condition before continuing. Supports:
//! * Timeout: numeric milliseconds (e.g., `"1000"`)
//! * Load state: `"load"`, `"domcontentloaded"`, `"networkidle"`
//! * Selector: CSS selector to wait for element presence
//!
//! # Examples
//!
//! ```bash
//! pw wait --condition 2000           # wait 2 seconds
//! pw wait --condition networkidle    # wait for network idle
//! pw wait --condition ".loaded"      # wait for element
//! ```

use std::time::Duration;

use clap::Args;
use pw_rs::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::commands::contract::{resolve_target_from_url_pair, standard_delta, standard_inputs};
use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ExecCtx};
use crate::commands::exec_flow::navigation_plan;
use crate::error::{PwError, Result};
use crate::output::CommandInputs;
use crate::session_broker::SessionHandle;
use crate::session_helpers::ArtifactsPolicy;
use crate::target::{ResolveEnv, ResolvedTarget, TargetPolicy};

/// Raw inputs from CLI or batch JSON before resolution.
#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitRaw {
	/// Target URL (positional)
	#[serde(default)]
	pub url: Option<String>,

	/// Condition to wait for (selector, timeout ms, or load state)
	#[arg(default_value = "networkidle")]
	#[serde(default)]
	pub condition: Option<String>,

	/// Target URL (named alternative)
	#[arg(long = "url", short = 'u', value_name = "URL")]
	#[serde(default, alias = "url_flag")]
	pub url_flag: Option<String>,
}

/// Resolved inputs ready for execution.
#[derive(Debug, Clone)]
pub struct WaitResolved {
	/// Navigation target (URL or current page).
	pub target: ResolvedTarget,

	/// Wait condition (timeout ms, load state, or CSS selector).
	pub condition: String,
}

/// Output data for the wait command result.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitData {
	condition: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	waited_ms: Option<u64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	selector_found: Option<bool>,
}

pub struct WaitCommand;

impl CommandDef for WaitCommand {
	const NAME: &'static str = "wait";

	type Raw = WaitRaw;
	type Resolved = WaitResolved;
	type Data = WaitData;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		let target = resolve_target_from_url_pair(raw.url, raw.url_flag, env, TargetPolicy::AllowCurrentPage)?;
		let condition = raw.condition.ok_or_else(|| PwError::Context("No condition provided for wait command".into()))?;

		Ok(WaitResolved { target, condition })
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, mut exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let url_display = args.target.url_str().unwrap_or("<current page>");
			info!(target = "pw", url = %url_display, condition = %args.condition, browser = %exec.ctx.browser, "wait");

			let plan = navigation_plan(exec.ctx, exec.last_url, &args.target, WaitUntil::NetworkIdle);
			let timeout_ms = plan.timeout_ms;
			let target = plan.target;
			let condition = args.condition.clone();

			let data = crate::session_helpers::with_session(&mut exec, plan.request, ArtifactsPolicy::Never, move |session| {
				let condition = condition.clone();
				Box::pin(async move {
					session.goto_target(&target, timeout_ms).await?;

					if let Ok(ms) = condition.parse::<u64>() {
						tokio::time::sleep(Duration::from_millis(ms)).await;

						return Ok(WaitData {
							condition: format!("timeout:{ms}ms"),
							waited_ms: Some(ms),
							selector_found: None,
						});
					}

					if matches!(condition.as_str(), "load" | "domcontentloaded" | "networkidle") {
						return Ok(WaitData {
							condition: format!("loadstate:{condition}"),
							waited_ms: None,
							selector_found: None,
						});
					}

					wait_for_selector(session, &condition).await
				})
			})
			.await?;

			let inputs = build_inputs(&args.target, args.condition.as_str());

			Ok(CommandOutcome {
				inputs,
				data,
				delta: standard_delta(&args.target, None, None),
			})
		})
	}
}

fn build_inputs(target: &ResolvedTarget, condition: &str) -> CommandInputs {
	if condition.parse::<u64>().is_ok() || matches!(condition, "load" | "domcontentloaded" | "networkidle") {
		standard_inputs(target, None, None, None, Some(serde_json::json!({ "condition": condition })))
	} else {
		standard_inputs(target, Some(condition), None, None, None)
	}
}

/// Polls for a CSS selector until it appears or times out.
async fn wait_for_selector(session: &SessionHandle, selector: &str) -> Result<WaitData> {
	let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");
	let max_attempts = 30u64;

	for attempt in 0..max_attempts {
		let visible = session
			.page()
			.evaluate_value(&format!("document.querySelector('{escaped}') !== null"))
			.await
			.unwrap_or_else(|_| "false".to_string());

		if visible == "true" {
			return Ok(WaitData {
				condition: format!("selector:{selector}"),
				waited_ms: Some(attempt * 1000),
				selector_found: Some(true),
			});
		}

		tokio::time::sleep(Duration::from_secs(1)).await;
	}

	Err(PwError::Timeout {
		ms: max_attempts * 1000,
		condition: selector.to_string(),
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn wait_raw_deserialize_from_json() {
		let json = r#"{"url": "https://example.com", "condition": "1000"}"#;
		let raw: WaitRaw = serde_json::from_str(json).unwrap();
		assert_eq!(raw.url, Some("https://example.com".into()));
		assert_eq!(raw.condition, Some("1000".into()));
	}

	#[test]
	fn wait_raw_deserialize_selector_condition() {
		let json = r#"{"condition": ".loaded"}"#;
		let raw: WaitRaw = serde_json::from_str(json).unwrap();
		assert_eq!(raw.condition, Some(".loaded".into()));
	}
}
