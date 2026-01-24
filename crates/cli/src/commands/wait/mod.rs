//! Wait command for various conditions.
//!
//! Waits for a specified condition before continuing. Supports:
//! - **Timeout**: numeric milliseconds (e.g., `"1000"`)
//! - **Load state**: `"load"`, `"domcontentloaded"`, `"networkidle"`
//! - **Selector**: CSS selector to wait for element presence
//!
//! # Examples
//!
//! ```bash
//! pw wait --condition 2000           # wait 2 seconds
//! pw wait --condition networkidle    # wait for network idle
//! pw wait --condition ".loaded"      # wait for element
//! ```

use std::time::Duration;

use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ContextDelta, ExecCtx};
use crate::error::{PwError, Result};
use crate::output::CommandInputs;
use crate::session_broker::{SessionHandle, SessionRequest};
use crate::session_helpers::ArtifactsPolicy;
use crate::target::{ResolveEnv, ResolvedTarget, TargetPolicy};

/// Raw inputs from CLI or batch JSON before resolution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitRaw {
	/// Target URL, resolved from context if not provided.
	#[serde(default)]
	pub url: Option<String>,

	/// Wait condition: milliseconds, load state, or CSS selector.
	#[serde(default)]
	pub condition: Option<String>,
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
		let target = env.resolve_target(raw.url, TargetPolicy::AllowCurrentPage)?;
		let condition = raw
			.condition
			.ok_or_else(|| PwError::Context("No condition provided for wait command".into()))?;

		Ok(WaitResolved { target, condition })
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
			info!(target = "pw", url = %url_display, condition = %args.condition, browser = %exec.ctx.browser, "wait");

			let preferred_url = args.target.preferred_url(exec.last_url);
			let timeout_ms = exec.ctx.timeout_ms();
			let target = args.target.target.clone();
			let condition = args.condition.clone();

			let req = SessionRequest::from_context(WaitUntil::NetworkIdle, exec.ctx)
				.with_preferred_url(preferred_url);

			let data = crate::session_helpers::with_session(
				&mut exec,
				req,
				ArtifactsPolicy::Never,
				move |session| {
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

						if matches!(
							condition.as_str(),
							"load" | "domcontentloaded" | "networkidle"
						) {
							return Ok(WaitData {
								condition: format!("loadstate:{condition}"),
								waited_ms: None,
								selector_found: None,
							});
						}

						wait_for_selector(session, &condition).await
					})
				},
			)
			.await?;

			let inputs = build_inputs(args.target.url_str(), args.condition.as_str());

			Ok(CommandOutcome {
				inputs,
				data,
				delta: ContextDelta {
					url: args.target.url_str().map(String::from),
					selector: None,
					output: None,
				},
			})
		})
	}
}

fn build_inputs(url_str: Option<&str>, condition: &str) -> CommandInputs {
	if condition.parse::<u64>().is_ok()
		|| matches!(condition, "load" | "domcontentloaded" | "networkidle")
	{
		CommandInputs {
			url: url_str.map(String::from),
			extra: Some(serde_json::json!({ "condition": condition })),
			..Default::default()
		}
	} else {
		CommandInputs {
			url: url_str.map(String::from),
			selector: Some(condition.to_string()),
			..Default::default()
		}
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
