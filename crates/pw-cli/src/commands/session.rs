use crate::context_store::ContextState;
use crate::error::Result;
use crate::session_broker::SessionDescriptor;
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
