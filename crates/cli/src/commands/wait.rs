use std::time::Duration;

use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::output::{CommandInputs, ErrorCode, OutputFormat, ResultBuilder, print_result};
use crate::session_broker::{SessionBroker, SessionRequest};
use pw::WaitUntil;
use serde::Serialize;
use tracing::info;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WaitData {
    condition: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    waited_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selector_found: Option<bool>,
}

pub async fn execute(
    url: &str,
    condition: &str,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    preferred_url: Option<&str>,
) -> Result<()> {
    info!(target = "pw", %url, %condition, browser = %ctx.browser, "wait");

    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(preferred_url),
        )
        .await?;
    session.goto_unless_current(url).await?;

    if let Ok(ms) = condition.parse::<u64>() {
        tokio::time::sleep(Duration::from_millis(ms)).await;

        let result = ResultBuilder::new("wait")
            .inputs(CommandInputs {
                url: Some(url.to_string()),
                extra: Some(serde_json::json!({ "condition": condition })),
                ..Default::default()
            })
            .data(WaitData {
                condition: format!("timeout:{ms}ms"),
                waited_ms: Some(ms),
                selector_found: None,
            })
            .build();

        print_result(&result, format);
    } else if matches!(condition, "load" | "domcontentloaded" | "networkidle") {
        let result = ResultBuilder::new("wait")
            .inputs(CommandInputs {
                url: Some(url.to_string()),
                extra: Some(serde_json::json!({ "condition": condition })),
                ..Default::default()
            })
            .data(WaitData {
                condition: format!("loadstate:{condition}"),
                waited_ms: None,
                selector_found: None,
            })
            .build();

        print_result(&result, format);
    } else {
        let escaped = condition.replace('\\', "\\\\").replace('\'', "\\'");
        let mut attempts = 0;
        let max_attempts = 30u64;

        loop {
            let visible = session
                .page()
                .evaluate_value(&format!("document.querySelector('{escaped}') !== null"))
                .await
                .unwrap_or_else(|_| "false".to_string());

            if visible == "true" {
                let result = ResultBuilder::new("wait")
                    .inputs(CommandInputs {
                        url: Some(url.to_string()),
                        selector: Some(condition.to_string()),
                        ..Default::default()
                    })
                    .data(WaitData {
                        condition: format!("selector:{condition}"),
                        waited_ms: Some(attempts * 1000),
                        selector_found: Some(true),
                    })
                    .build();

                print_result(&result, format);
                break;
            }

            attempts += 1;
            if attempts >= max_attempts {
                let result = ResultBuilder::<WaitData>::new("wait")
                    .inputs(CommandInputs {
                        url: Some(url.to_string()),
                        selector: Some(condition.to_string()),
                        ..Default::default()
                    })
                    .error(
                        ErrorCode::Timeout,
                        format!(
                            "Timeout after {}ms waiting for selector: {condition}",
                            max_attempts * 1000
                        ),
                    )
                    .build();

                print_result(&result, format);
                session.close().await?;

                return Err(PwError::Timeout {
                    ms: max_attempts * 1000,
                    condition: condition.to_string(),
                });
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    session.close().await
}
