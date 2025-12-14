use crate::browser::js;
use crate::context::CommandContext;
use crate::error::Result;
use crate::session_broker::{SessionBroker, SessionRequest};
use crate::types::{ElementCoords, IndexedElementCoords};
use pw::WaitUntil;
use tracing::info;

pub async fn execute_single(
    url: &str,
    selector: &str,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
) -> Result<()> {
    info!(target = "pw", %url, %selector, browser = %ctx.browser, "coords single");
    let session = broker
        .session(SessionRequest::from_context(WaitUntil::NetworkIdle, ctx))
        .await?;
    session.goto(url).await?;

    let result_json = session
        .page()
        .evaluate_value(&js::get_element_coords_js(selector))
        .await?;

    if result_json == "null" {
        println!("Element not found or not visible");
    } else {
        let coords: ElementCoords = serde_json::from_str(&result_json)?;
        println!("{}", serde_json::to_string_pretty(&coords)?);
    }

    session.close().await
}

pub async fn execute_all(
    url: &str,
    selector: &str,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
) -> Result<()> {
    info!(target = "pw", %url, %selector, browser = %ctx.browser, "coords all");
    let session = broker
        .session(SessionRequest::from_context(WaitUntil::NetworkIdle, ctx))
        .await?;
    session.goto(url).await?;

    let results_json = session
        .page()
        .evaluate_value(&js::get_all_element_coords_js(selector))
        .await?;

    let results: Vec<IndexedElementCoords> = serde_json::from_str(&results_json)?;
    println!("{}", serde_json::to_string_pretty(&results)?);

    session.close().await
}
