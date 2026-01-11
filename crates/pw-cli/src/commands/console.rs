use std::time::Duration;

use crate::browser::js::console_capture_injection_js;
use crate::context::CommandContext;
use crate::error::Result;
use crate::output::{CommandInputs, OutputFormat, ResultBuilder, print_result};
use crate::session_broker::{SessionBroker, SessionRequest};
use crate::types::ConsoleMessage;
use pw::WaitUntil;
use serde::Serialize;
use tracing::{info, warn};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConsoleData {
    messages: Vec<ConsoleMessage>,
    count: usize,
    error_count: usize,
    warning_count: usize,
}

pub async fn execute(
    url: &str,
    timeout_ms: u64,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    preferred_url: Option<&str>,
) -> Result<()> {
    info!(target = "pw", %url, timeout_ms, browser = %ctx.browser, "capture console");
    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(preferred_url),
        )
        .await?;

    if let Err(err) = session
        .page()
        .evaluate(console_capture_injection_js())
        .await
    {
        warn!(target = "pw.browser.console", error = %err, "failed to inject console capture");
    }

    session.goto_unless_current(url).await?;

    tokio::time::sleep(Duration::from_millis(timeout_ms)).await;

    let messages_json = session
        .page()
        .evaluate_value("JSON.stringify(window.__consoleMessages || [])")
        .await
        .unwrap_or_else(|_| "[]".to_string());

    let messages: Vec<ConsoleMessage> = serde_json::from_str(&messages_json).unwrap_or_default();

    // Emit browser console messages to tracing for visibility
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

    let result = ResultBuilder::new("console")
        .inputs(CommandInputs {
            url: Some(url.to_string()),
            extra: Some(serde_json::json!({ "timeout_ms": timeout_ms })),
            ..Default::default()
        })
        .data(ConsoleData {
            messages,
            count,
            error_count,
            warning_count,
        })
        .build();

    print_result(&result, format);

    session.close().await
}
