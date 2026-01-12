use std::time::Instant;

use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::output::{
    CommandInputs, ErrorCode, EvalData, OutputFormat, ResultBuilder, print_result,
};
use crate::session_broker::{SessionBroker, SessionRequest};
use pw::WaitUntil;
use tracing::{debug, info};

pub async fn execute(
    url: &str,
    expression: &str,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    preferred_url: Option<&str>,
) -> Result<()> {
    let _start = Instant::now();
    info!(target = "pw", %url, browser = %ctx.browser, "eval js");
    debug!(target = "pw", %expression, "expression");

    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(preferred_url),
        )
        .await?;
    session.goto_unless_current(url).await?;

    // Use JSON.stringify wrapper to get the value
    let wrapped_expr = format!("JSON.stringify({})", expression);
    let raw_result = session.page().evaluate_value(&wrapped_expr).await;

    match raw_result {
        Ok(json_str) => {
            // The result is already JSON-stringified
            let value: serde_json::Value =
                serde_json::from_str(&json_str).unwrap_or(serde_json::Value::Null);

            let result = ResultBuilder::new("eval")
                .inputs(CommandInputs {
                    url: Some(url.to_string()),
                    expression: Some(truncate_expression(expression)),
                    ..Default::default()
                })
                .data(EvalData {
                    result: value,
                    expression: expression.to_string(),
                })
                .build();

            print_result(&result, format);
        }
        Err(e) => {
            // Playwright-level error - likely a JS exception
            let error_msg = e.to_string();

            let result = ResultBuilder::<EvalData>::new("eval")
                .inputs(CommandInputs {
                    url: Some(url.to_string()),
                    expression: Some(truncate_expression(expression)),
                    ..Default::default()
                })
                .error_with_details(
                    ErrorCode::JsEvalFailed,
                    format!("Evaluation failed: {error_msg}"),
                    serde_json::json!({
                        "expression": truncate_expression(expression),
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
        format!("{}...", &expr[..MAX_LEN])
    } else {
        expr.to_string()
    }
}
