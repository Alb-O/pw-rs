//! Console message capture command.
//!
//! Captures JavaScript console output (log, warn, error, etc.) from a page.
//! Injects a capture script before navigation, then collects messages after
//! a configurable timeout.
//!
//! # Example
//!
//! ```bash
//! pw console https://example.com --timeout-ms 5000
//! ```

use std::time::Duration;

use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::browser::js::console_capture_injection_js;
use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ContextDelta, ExecCtx};
use crate::error::Result;
use crate::output::CommandInputs;
use crate::session_broker::SessionRequest;
use crate::session_helpers::{ArtifactsPolicy, with_session};
use crate::target::{ResolveEnv, ResolvedTarget, TargetPolicy};
use crate::types::ConsoleMessage;

/// Raw inputs from CLI or batch JSON before resolution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleRaw {
	/// Target URL, resolved from context if not provided.
	#[serde(default)]
	pub url: Option<String>,

	/// How long to capture messages before returning (default: 3000ms).
	#[serde(default, alias = "timeout_ms")]
	pub timeout_ms: Option<u64>,
}

/// Resolved inputs ready for execution.
///
/// The [`timeout_ms`](Self::timeout_ms) defaults to 3000 if not specified.
#[derive(Debug, Clone)]
pub struct ConsoleResolved {
	/// Navigation target (URL or current page).
	pub target: ResolvedTarget,

	/// Capture duration in milliseconds.
	pub timeout_ms: u64,
}

/// Captured console messages with summary counts.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleData {
	pub messages: Vec<ConsoleMessage>,
	pub count: usize,
	pub error_count: usize,
	pub warning_count: usize,
}

pub struct ConsoleCommand;

impl CommandDef for ConsoleCommand {
	const NAME: &'static str = "console";

	type Raw = ConsoleRaw;
	type Resolved = ConsoleResolved;
	type Data = ConsoleData;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		let target = env.resolve_target(raw.url, TargetPolicy::AllowCurrentPage)?;

		Ok(ConsoleResolved {
			target,
			timeout_ms: raw.timeout_ms.unwrap_or(3000),
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
			info!(target = "pw", url = %url_display, timeout_ms = args.timeout_ms, browser = %exec.ctx.browser, "capture console");

			let preferred_url = args.target.preferred_url(exec.last_url);
			let timeout_ms = exec.ctx.timeout_ms();
			let target = args.target.target.clone();
			let capture_timeout_ms = args.timeout_ms;

			let req = SessionRequest::from_context(WaitUntil::NetworkIdle, exec.ctx)
				.with_preferred_url(preferred_url);

			let data = with_session(&mut exec, req, ArtifactsPolicy::Never, move |session| {
				Box::pin(async move {
					if let Err(err) = session
						.page()
						.evaluate(console_capture_injection_js())
						.await
					{
						warn!(target = "pw.browser.console", error = %err, "failed to inject console capture");
					}

					session.goto_target(&target, timeout_ms).await?;

					tokio::time::sleep(Duration::from_millis(capture_timeout_ms)).await;

					let messages_json = session
						.page()
						.evaluate_value("JSON.stringify(window.__consoleMessages || [])")
						.await
						.unwrap_or_else(|_| "[]".to_string());

					let messages: Vec<ConsoleMessage> =
						serde_json::from_str(&messages_json).unwrap_or_default();

					for msg in &messages {
						info!(
							target = "pw.browser.console",
							kind = %msg.msg_type,
							text = %msg.text,
							stack = ?msg.stack,
							"browser console"
						);
					}

					let error_count = messages.iter().filter(|m| m.msg_type == "error").count();
					let warning_count = messages.iter().filter(|m| m.msg_type == "warning").count();
					let count = messages.len();

					Ok(ConsoleData {
						messages,
						count,
						error_count,
						warning_count,
					})
				})
			})
			.await?;

			let inputs = CommandInputs {
				url: args.target.url_str().map(String::from),
				extra: Some(serde_json::json!({ "timeout_ms": args.timeout_ms })),
				..Default::default()
			};

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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn console_raw_deserialize_from_json() {
		let json = r#"{"url": "https://example.com", "timeout_ms": 5000}"#;
		let raw: ConsoleRaw = serde_json::from_str(json).unwrap();
		assert_eq!(raw.url, Some("https://example.com".into()));
		assert_eq!(raw.timeout_ms, Some(5000));
	}

	#[test]
	fn console_raw_defaults() {
		let json = r#"{}"#;
		let raw: ConsoleRaw = serde_json::from_str(json).unwrap();
		assert_eq!(raw.timeout_ms, None);
	}
}
