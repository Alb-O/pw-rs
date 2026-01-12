use crate::context::CommandContext;
use crate::error::Result;
use crate::output::{CommandInputs, OutputFormat, ResultBuilder, print_result};
use crate::session_broker::{SessionBroker, SessionRequest};
use pw::WaitUntil;
use serde::Serialize;
use tracing::info;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HtmlData {
    html: String,
    selector: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    length: Option<usize>,
}

pub async fn execute(
    url: &str,
    selector: &str,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    preferred_url: Option<&str>,
) -> Result<()> {
    if selector == "html" {
        info!(target = "pw", %url, browser = %ctx.browser, "get full page HTML");
    } else {
        info!(target = "pw", %url, %selector, browser = %ctx.browser, "get HTML for selector");
    }

    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(preferred_url),
        )
        .await?;
    session.goto_unless_current(url).await?;

    let locator = session.page().locator(selector).await;
    let html = locator.inner_html().await?;

    let result = ResultBuilder::new("html")
        .inputs(CommandInputs {
            url: Some(url.to_string()),
            selector: Some(selector.to_string()),
            ..Default::default()
        })
        .data(HtmlData {
            length: Some(html.len()),
            html,
            selector: selector.to_string(),
        })
        .build();

    print_result(&result, format);
    session.close().await
}
