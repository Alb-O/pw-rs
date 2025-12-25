use crate::context_store::ContextState;
use crate::error::Result;
use crate::output::{OutputFormat, ResultBuilder, SessionStartData, print_result};
use crate::session_broker::{SessionBroker, SessionDescriptor, SessionRequest};
use pw::WaitUntil;
use serde_json::json;
use std::fs;
use tracing::{info, warn};

pub async fn status(ctx_state: &ContextState, format: OutputFormat) -> Result<()> {
    let Some(path) = ctx_state.session_descriptor_path() else {
        let result = ResultBuilder::<serde_json::Value>::new("session status")
            .data(json!({
                "active": false,
                "message": "No active context; session status unavailable"
            }))
            .build();
        print_result(&result, format);
        return Ok(());
    };

    match SessionDescriptor::load(&path)? {
        Some(desc) => {
            let alive = desc.is_alive();
            let payload = json!({
                "active": true,
                "path": path,
                "browser": desc.browser,
                "headless": desc.headless,
                "cdp_endpoint": desc.cdp_endpoint,
                "ws_endpoint": desc.ws_endpoint,
                "driver_hash": desc.driver_hash,
                "pid": desc.pid,
                "created_at": desc.created_at,
                "alive": alive,
            });

            let result = ResultBuilder::new("session status")
                .data(payload)
                .build();
            print_result(&result, format);
        }
        None => {
            let result = ResultBuilder::<serde_json::Value>::new("session status")
                .data(json!({
                    "active": false,
                    "message": "No session descriptor for context; run a browser command to create one"
                }))
                .build();
            print_result(&result, format);
        }
    }

    Ok(())
}

pub async fn clear(ctx_state: &ContextState, format: OutputFormat) -> Result<()> {
    let Some(path) = ctx_state.session_descriptor_path() else {
        let result = ResultBuilder::<serde_json::Value>::new("session clear")
            .data(json!({
                "cleared": false,
                "message": "No active context; nothing to clear"
            }))
            .build();
        print_result(&result, format);
        return Ok(());
    };

    if path.exists() {
        fs::remove_file(&path)?;
        info!(target = "pw.session", path = %path.display(), "session descriptor removed");

        let result = ResultBuilder::<serde_json::Value>::new("session clear")
            .data(json!({
                "cleared": true,
                "path": path,
            }))
            .build();
        print_result(&result, format);
    } else {
        warn!(target = "pw.session", path = %path.display(), "no session descriptor to remove");

        let result = ResultBuilder::<serde_json::Value>::new("session clear")
            .data(json!({
                "cleared": false,
                "path": path,
                "message": "No session descriptor found"
            }))
            .build();
        print_result(&result, format);
    }

    Ok(())
}

pub async fn start(
    _ctx_state: &ContextState,
    broker: &mut SessionBroker<'_>,
    headful: bool,
    format: OutputFormat,
) -> Result<()> {
    let mut request = SessionRequest::from_context(WaitUntil::NetworkIdle, broker.context());
    request.headless = !headful;
    request.launch_server = true;

    let session = broker.session(request).await?;

    let ws_endpoint = session.ws_endpoint().map(|s| s.to_string());
    let browser = broker.context().browser.to_string();

    let result = ResultBuilder::new("session start")
        .data(SessionStartData {
            ws_endpoint,
            cdp_endpoint: None, // TODO: Add if available
            browser,
            headless: !headful,
        })
        .build();

    print_result(&result, format);
    session.close().await
}

pub async fn stop(
    ctx_state: &ContextState,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
) -> Result<()> {
    let Some(path) = ctx_state.session_descriptor_path() else {
        let result = ResultBuilder::<serde_json::Value>::new("session stop")
            .data(json!({
                "stopped": false,
                "message": "No active context; nothing to stop"
            }))
            .build();
        print_result(&result, format);
        return Ok(());
    };

    let Some(descriptor) = SessionDescriptor::load(&path)? else {
        let result = ResultBuilder::<serde_json::Value>::new("session stop")
            .data(json!({
                "stopped": false,
                "message": "No session descriptor for context; nothing to stop"
            }))
            .build();
        print_result(&result, format);
        return Ok(());
    };

    let endpoint = descriptor
        .ws_endpoint
        .as_deref()
        .or_else(|| descriptor.cdp_endpoint.as_deref());

    let Some(endpoint) = endpoint else {
        fs::remove_file(&path)?;
        let result = ResultBuilder::<serde_json::Value>::new("session stop")
            .data(json!({
                "stopped": false,
                "path": path,
                "message": "Descriptor missing endpoint; removed descriptor"
            }))
            .build();
        print_result(&result, format);
        return Ok(());
    };

    let mut request = SessionRequest::from_context(WaitUntil::NetworkIdle, broker.context());
    request.browser = descriptor.browser;
    request.headless = descriptor.headless;
    request.cdp_endpoint = Some(endpoint);
    request.launch_server = false;

    let session = broker.session(request).await?;
    session.shutdown_server().await?;
    fs::remove_file(&path)?;

    let result = ResultBuilder::<serde_json::Value>::new("session stop")
        .data(json!({
            "stopped": true,
            "path": path,
        }))
        .build();
    print_result(&result, format);

    Ok(())
}
