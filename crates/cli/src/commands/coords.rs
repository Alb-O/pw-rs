use crate::browser::js;
use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::output::{CommandInputs, ErrorCode, OutputFormat, ResultBuilder, print_result};
use crate::session_broker::{SessionBroker, SessionRequest};
use crate::types::{ElementCoords, IndexedElementCoords};
use pw::WaitUntil;
use serde::Serialize;
use tracing::info;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CoordsData {
    coords: ElementCoords,
    selector: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CoordsAllData {
    coords: Vec<IndexedElementCoords>,
    selector: String,
    count: usize,
}

pub async fn execute_single(
    url: &str,
    selector: &str,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    preferred_url: Option<&str>,
) -> Result<()> {
    info!(target = "pw", %url, %selector, browser = %ctx.browser, "coords single");
    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(preferred_url),
        )
        .await?;
    session.goto_unless_current(url).await?;

    let result_json = session
        .page()
        .evaluate_value(&js::get_element_coords_js(selector))
        .await?;

    if result_json == "null" {
        let result = ResultBuilder::<CoordsData>::new("coords")
            .inputs(CommandInputs {
                url: Some(url.to_string()),
                selector: Some(selector.to_string()),
                ..Default::default()
            })
            .error(
                ErrorCode::SelectorNotFound,
                format!("Element not found or not visible: {selector}"),
            )
            .build();

        print_result(&result, format);
        session.close().await?;
        return Err(PwError::ElementNotFound {
            selector: selector.to_string(),
        });
    }

    let coords: ElementCoords = serde_json::from_str(&result_json)?;

    let result = ResultBuilder::new("coords")
        .inputs(CommandInputs {
            url: Some(url.to_string()),
            selector: Some(selector.to_string()),
            ..Default::default()
        })
        .data(CoordsData {
            coords,
            selector: selector.to_string(),
        })
        .build();

    print_result(&result, format);
    session.close().await
}

pub async fn execute_all(
    url: &str,
    selector: &str,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    preferred_url: Option<&str>,
) -> Result<()> {
    info!(target = "pw", %url, %selector, browser = %ctx.browser, "coords all");
    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(preferred_url),
        )
        .await?;
    session.goto_unless_current(url).await?;

    let results_json = session
        .page()
        .evaluate_value(&js::get_all_element_coords_js(selector))
        .await?;

    let coords: Vec<IndexedElementCoords> = serde_json::from_str(&results_json)?;
    let count = coords.len();

    let result = ResultBuilder::new("coords-all")
        .inputs(CommandInputs {
            url: Some(url.to_string()),
            selector: Some(selector.to_string()),
            ..Default::default()
        })
        .data(CoordsAllData {
            coords,
            selector: selector.to_string(),
            count,
        })
        .build();

    print_result(&result, format);
    session.close().await
}
