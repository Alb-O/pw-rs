use std::time::Instant;

use crate::context::CommandContext;
use crate::error::Result;
use crate::output::{
    CommandInputs, DiagnosticLevel, NavigateData, OutputFormat, ResultBuilder, print_result,
};
use crate::session_broker::{SessionBroker, SessionRequest};
use pw::WaitUntil;
use tracing::{info, warn};

pub async fn execute(
    url: &str,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
) -> Result<()> {
    let start = Instant::now();
    info!(target = "pw", %url, browser = %ctx.browser, "navigate");

    let session = broker
        .session(SessionRequest::from_context(WaitUntil::NetworkIdle, ctx))
        .await?;

    // Navigate and wait for load
    session.goto(url).await?;

    // Get page info
    let title = session.page().title().await.unwrap_or_default();
    let final_url = session.page().url();

    // Collect any JS errors from the page
    let errors_json = session
        .page()
        .evaluate_value("JSON.stringify(window.__playwrightErrors || [])")
        .await
        .unwrap_or_else(|_| "[]".to_string());

    let errors: Vec<String> = serde_json::from_str(&errors_json).unwrap_or_default();

    if !errors.is_empty() {
        warn!(
            target = "pw.browser",
            count = errors.len(),
            "page reported errors"
        );
    }

    // Build result
    let mut builder = ResultBuilder::new("navigate")
        .inputs(CommandInputs {
            url: Some(url.to_string()),
            ..Default::default()
        })
        .data(NavigateData {
            url: final_url,
            title,
            errors: errors.clone(),
            warnings: vec![],
        });

    // Add diagnostics for any errors found
    for error in &errors {
        builder = builder.diagnostic_with_source(DiagnosticLevel::Error, error, "browser");
    }

    let result = builder.build();

    // Record timing
    let _elapsed = start.elapsed();

    print_result(&result, format);

    session.close().await
}
