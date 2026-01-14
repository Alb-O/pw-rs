//! JavaScript evaluation command.

use std::time::Instant;

use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::output::{
    CommandInputs, ErrorCode, EvalData, OutputFormat, ResultBuilder, print_result,
};
use crate::session_broker::{SessionBroker, SessionRequest};
use crate::target::{Resolve, ResolveEnv, ResolvedTarget, TargetPolicy};
use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

// ---------------------------------------------------------------------------
// Raw and Resolved Types
// ---------------------------------------------------------------------------

/// Raw inputs from CLI or batch JSON.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalRaw {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default, alias = "url_flag")]
    pub url_flag: Option<String>,
    #[serde(default)]
    pub expression: Option<String>,
    #[serde(default, alias = "expression_flag", alias = "expr")]
    pub expression_flag: Option<String>,
}

impl EvalRaw {
    pub fn from_cli(
        url: Option<String>,
        url_flag: Option<String>,
        expression: Option<String>,
        expression_flag: Option<String>,
    ) -> Self {
        Self {
            url,
            url_flag,
            expression,
            expression_flag,
        }
    }
}

/// Resolved inputs ready for execution.
#[derive(Debug, Clone)]
pub struct EvalResolved {
    pub target: ResolvedTarget,
    pub expression: String,
}

impl EvalResolved {
    pub fn preferred_url<'a>(&'a self, last_url: Option<&'a str>) -> Option<&'a str> {
        self.target.preferred_url(last_url)
    }
}

impl Resolve for EvalRaw {
    type Output = EvalResolved;

    fn resolve(self, env: &ResolveEnv<'_>) -> Result<EvalResolved> {
        let url = self.url_flag.or(self.url);
        let target = env.resolve_target(url, TargetPolicy::AllowCurrentPage)?;

        let expression = self.expression_flag.or(self.expression).ok_or_else(|| {
            PwError::Context(
                "expression is required (provide positionally, via --expr, or via --file)".into(),
            )
        })?;

        Ok(EvalResolved { target, expression })
    }
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

/// Execute eval with resolved arguments.
pub async fn execute_resolved(
    args: &EvalResolved,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    last_url: Option<&str>,
) -> Result<()> {
    let _start = Instant::now();
    let url_display = args.target.url_str().unwrap_or("<current page>");
    info!(target = "pw", url = %url_display, browser = %ctx.browser, "eval js");
    debug!(target = "pw", expression = %args.expression, "expression");

    let preferred_url = args.preferred_url(last_url);
    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(preferred_url),
        )
        .await?;
    session
        .goto_target(&args.target.target, ctx.timeout_ms())
        .await?;

    let wrapped_expr = format!("JSON.stringify({})", args.expression);
    let raw_result = session.page().evaluate_value(&wrapped_expr).await;

    match raw_result {
        Ok(json_str) => {
            let value: serde_json::Value =
                serde_json::from_str(&json_str).unwrap_or(serde_json::Value::Null);

            let result = ResultBuilder::new("eval")
                .inputs(CommandInputs {
                    url: args.target.url_str().map(String::from),
                    expression: Some(truncate_expression(&args.expression)),
                    ..Default::default()
                })
                .data(EvalData {
                    result: value,
                    expression: args.expression.clone(),
                })
                .build();

            print_result(&result, format);
        }
        Err(e) => {
            let error_msg = e.to_string();

            let result = ResultBuilder::<EvalData>::new("eval")
                .inputs(CommandInputs {
                    url: args.target.url_str().map(String::from),
                    expression: Some(truncate_expression(&args.expression)),
                    ..Default::default()
                })
                .error_with_details(
                    ErrorCode::JsEvalFailed,
                    format!("Evaluation failed: {error_msg}"),
                    serde_json::json!({
                        "expression": truncate_expression(&args.expression),
                    }),
                )
                .build();

            print_result(&result, format);
            session.close().await?;
            return Err(PwError::JsEval(error_msg));
        }
    }

    session.close().await
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
