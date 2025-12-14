use std::time::Duration;

use crate::browser::js::console_capture_injection_js;
use crate::context::CommandContext;
use crate::error::Result;
use crate::session_broker::{SessionBroker, SessionRequest};
use crate::types::ConsoleMessage;
use pw::WaitUntil;
use tracing::{info, warn};

pub async fn execute(
    url: &str,
    timeout_ms: u64,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
) -> Result<()> {
    info!(target = "pw", %url, timeout_ms, browser = %ctx.browser, "capture console");
    let session = broker
        .session(SessionRequest::from_context(WaitUntil::NetworkIdle, ctx))
        .await?;

    if let Err(err) = session
        .page()
        .evaluate(console_capture_injection_js())
        .await
    {
        warn!(target = "pw.browser.console", error = %err, "failed to inject console capture");
    }

    session.goto(url).await?;

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

    println!("{}", serde_json::to_string_pretty(&messages)?);

    session.close().await
}
