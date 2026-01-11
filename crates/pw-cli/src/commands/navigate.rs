use crate::context::CommandContext;
use crate::context_store::is_current_page_sentinel;
use crate::error::Result;
use crate::output::{
    CommandInputs, DiagnosticLevel, NavigateData, OutputFormat, ResultBuilder, print_result,
};
use crate::session_broker::{SessionBroker, SessionRequest};
use pw::WaitUntil;
use tracing::{info, warn};

/// Execute navigation and return the actual browser URL after navigation.
///
/// The returned URL may differ from the input due to redirects.
pub async fn execute(
    url: &str,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    preferred_url: Option<&str>,
) -> Result<String> {
    info!(target = "pw", %url, browser = %ctx.browser, "navigate");

    // Use WaitUntil::Load instead of NetworkIdle - SPAs with analytics/websockets
    // often never reach "network idle", causing false timeout errors.
    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::Load, ctx).with_preferred_url(preferred_url),
        )
        .await?;

    // Skip navigation if already on the target URL (avoids page refresh)
    if !is_current_page_sentinel(url) {
        session.goto_if_needed(url).await?;
    }

    let title = session.page().title().await.unwrap_or_default();
    let actual_url = session
        .page()
        .evaluate_value("window.location.href")
        .await
        .unwrap_or_else(|_| session.page().url());

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

    let actual_url_field = if actual_url != url {
        Some(actual_url.clone())
    } else {
        None
    };

    let mut builder = ResultBuilder::new("navigate")
        .inputs(CommandInputs {
            url: Some(url.to_string()),
            ..Default::default()
        })
        .data(NavigateData {
            url: url.to_string(),
            actual_url: actual_url_field,
            title,
            errors: errors.clone(),
            warnings: vec![],
        });

    for error in &errors {
        builder = builder.diagnostic_with_source(DiagnosticLevel::Error, error, "browser");
    }

    let result = builder.build();

    print_result(&result, format);

    session.close().await?;
    Ok(actual_url)
}
