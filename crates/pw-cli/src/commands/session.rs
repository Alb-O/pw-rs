use crate::context_store::ContextState;
use crate::error::Result;
use crate::session_broker::{SessionBroker, SessionDescriptor, SessionRequest};
use pw::WaitUntil;
use serde_json::json;
use std::fs;
use tracing::{info, warn};

pub async fn status(ctx_state: &ContextState) -> Result<()> {
    let Some(path) = ctx_state.session_descriptor_path() else {
        println!("No active context; session status unavailable");
        return Ok(());
    };

    match SessionDescriptor::load(&path)? {
        Some(desc) => {
            let alive = desc.is_alive();
            let payload = json!({
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
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        None => {
            println!("No session descriptor for context; run a browser command to create one");
        }
    }

    Ok(())
}

pub async fn clear(ctx_state: &ContextState) -> Result<()> {
    let Some(path) = ctx_state.session_descriptor_path() else {
        println!("No active context; nothing to clear");
        return Ok(());
    };

    if path.exists() {
        fs::remove_file(&path)?;
        info!(target = "pw.session", path = %path.display(), "session descriptor removed");
        println!("Removed session descriptor at {}", path.display());
    } else {
        warn!(target = "pw.session", path = %path.display(), "no session descriptor to remove");
        println!("No session descriptor found at {}", path.display());
    }

    Ok(())
}

pub async fn start(
    _ctx_state: &ContextState,
    broker: &mut SessionBroker<'_>,
    headful: bool,
) -> Result<()> {
    let mut request = SessionRequest::from_context(WaitUntil::NetworkIdle, broker.context());
    request.headless = !headful;
    request.launch_server = true;

    let session = broker.session(request).await?;
    if let Some(endpoint) = session.ws_endpoint() {
        println!("Session started; ws endpoint: {endpoint}");
    } else {
        println!("Session started");
    }
    session.close().await
}

pub async fn stop(ctx_state: &ContextState, broker: &mut SessionBroker<'_>) -> Result<()> {
    let Some(path) = ctx_state.session_descriptor_path() else {
        println!("No active context; nothing to stop");
        return Ok(());
    };

    let Some(descriptor) = SessionDescriptor::load(&path)? else {
        println!("No session descriptor for context; nothing to stop");
        return Ok(());
    };

    let endpoint = descriptor
        .ws_endpoint
        .as_deref()
        .or_else(|| descriptor.cdp_endpoint.as_deref());

    let Some(endpoint) = endpoint else {
        println!("Descriptor missing endpoint; removing");
        fs::remove_file(&path)?;
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
    println!(
        "Stopped session and removed descriptor at {}",
        path.display()
    );

    Ok(())
}
