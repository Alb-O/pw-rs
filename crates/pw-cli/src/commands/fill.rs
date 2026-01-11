use crate::context::CommandContext;
use crate::error::Result;
use crate::output::{CommandInputs, FillData, OutputFormat, ResultBuilder, print_result};
use crate::session_broker::{SessionBroker, SessionRequest};
use pw::WaitUntil;
use tracing::info;

pub async fn execute(
    url: &str,
    selector: &str,
    text: &str,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    preferred_url: Option<&str>,
) -> Result<()> {
    info!(target = "pw", %url, %selector, "fill");

    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::Load, ctx).with_preferred_url(preferred_url),
        )
        .await?;

    session.goto_unless_current(url).await?;

    let locator = session.page().locator(selector).await;
    locator.fill(text, None).await?;

    let result = ResultBuilder::new("fill")
        .inputs(CommandInputs {
            url: Some(url.to_string()),
            selector: Some(selector.to_string()),
            ..Default::default()
        })
        .data(FillData {
            selector: selector.to_string(),
            text: text.to_string(),
        })
        .build();

    print_result(&result, format);

    session.close().await
}
