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
    info!(target = "pw", %url, browser = %ctx.browser, "navigate");

    let session = broker
        .session(SessionRequest::from_context(WaitUntil::NetworkIdle, ctx))
        .await?;

    session.goto(url).await?;

    let title = session.page().title().await.unwrap_or_default();
    let final_url = session.page().url();

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

    for error in &errors {
        builder = builder.diagnostic_with_source(DiagnosticLevel::Error, error, "browser");
    }

    let result = builder.build();

    print_result(&result, format);

    session.close().await
}
