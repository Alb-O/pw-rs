//! JavaScript evaluation command.

use std::path::PathBuf;

use clap::Args;
use pw_rs::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::commands::contract::{resolve_target_from_url_pair, standard_delta, standard_inputs};
use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ExecCtx};
use crate::commands::exec_flow::navigation_plan;
use crate::error::{PwError, Result};
use crate::output::EvalData;
use crate::session_helpers::{ArtifactsPolicy, with_session};
use crate::target::{ResolveEnv, ResolvedTarget, TargetPolicy};

/// Raw inputs from CLI or batch JSON.
#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalRaw {
	/// JavaScript expression (positional)
	#[serde(default)]
	pub expression: Option<String>,

	/// Target URL (positional, uses context when omitted)
	#[serde(default)]
	pub url: Option<String>,

	/// JavaScript expression (named alternative to positional)
	#[arg(long = "expr", short = 'e', value_name = "EXPRESSION")]
	#[serde(default, alias = "expression_flag", alias = "expr")]
	pub expression_flag: Option<String>,

	/// Read JavaScript expression from file
	#[arg(long = "file", short = 'F', value_name = "FILE")]
	#[serde(default)]
	pub file: Option<PathBuf>,

	/// Target URL (named alternative to positional)
	#[arg(long = "url", short = 'u', value_name = "URL")]
	#[serde(default, alias = "url_flag")]
	pub url_flag: Option<String>,
}

/// Resolved inputs ready for execution.
#[derive(Debug, Clone)]
pub struct EvalResolved {
	pub target: ResolvedTarget,
	pub expression: String,
}

pub struct EvalCommand;

impl CommandDef for EvalCommand {
	const NAME: &'static str = "page.eval";

	type Raw = EvalRaw;
	type Resolved = EvalResolved;
	type Data = EvalData;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		let target = resolve_target_from_url_pair(raw.url, raw.url_flag, env, TargetPolicy::AllowCurrentPage)?;

		let expression = raw
			.file
			.as_ref()
			.and_then(|p| std::fs::read_to_string(p).ok())
			.or(raw.expression_flag)
			.or(raw.expression)
			.ok_or_else(|| PwError::Context("expression is required (provide positionally, via --expr, or via --file)".into()))?;

		Ok(EvalResolved { target, expression })
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, mut exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let url_display = args.target.url_str().unwrap_or("<current page>");
			info!(target = "pw", url = %url_display, browser = %exec.ctx.browser, "eval js");
			debug!(target = "pw", expression = %args.expression, "expression");

			let plan = navigation_plan(exec.ctx, exec.last_url, &args.target, WaitUntil::NetworkIdle);
			let timeout_ms = plan.timeout_ms;
			let target = plan.target;
			let expression = args.expression.clone();
			let expression_for_inputs = truncate_expression(&expression);

			let data = with_session(&mut exec, plan.request, ArtifactsPolicy::Never, move |session| {
				let expression = expression.clone();
				Box::pin(async move {
					session.goto_target(&target, timeout_ms).await?;

					let wrapped_expr = format!("JSON.stringify({})", expression);
					let raw_result = session.page().evaluate_value(&wrapped_expr).await;

					let json_str = raw_result.map_err(|e| PwError::JsEval(e.to_string()))?;
					let value: serde_json::Value = serde_json::from_str(&json_str).unwrap_or(serde_json::Value::Null);

					Ok(EvalData { result: value, expression })
				})
			})
			.await?;

			let inputs = standard_inputs(&args.target, None, Some(expression_for_inputs), None, None);

			Ok(CommandOutcome {
				inputs,
				data,
				delta: standard_delta(&args.target, None, None),
			})
		})
	}
}

/// Truncate expression for output (avoid huge expressions in output)
fn truncate_expression(expr: &str) -> String {
	const MAX_LEN: usize = 500;
	if expr.len() > MAX_LEN {
		let truncate_at = expr
			.char_indices()
			.take_while(|(i, _)| *i < MAX_LEN)
			.last()
			.map(|(i, c)| i + c.len_utf8())
			.unwrap_or(0);
		format!("{}...", &expr[..truncate_at])
	} else {
		expr.to_string()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn truncate_handles_multibyte_utf8() {
		let s = "x".repeat(498) + "─────";
		let result = truncate_expression(&s);
		assert!(result.ends_with("..."));
		assert!(result.len() <= 504);
	}

	#[test]
	fn truncate_short_string_unchanged() {
		let s = "short";
		assert_eq!(truncate_expression(s), "short");
	}

	#[test]
	fn eval_raw_deserialize() {
		let json = r#"{"url": "https://example.com", "expression": "document.title"}"#;
		let raw: EvalRaw = serde_json::from_str(json).unwrap();
		assert_eq!(raw.url, Some("https://example.com".into()));
		assert_eq!(raw.expression, Some("document.title".into()));
	}
}
