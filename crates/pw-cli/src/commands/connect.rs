use crate::context_store::ContextState;
use crate::error::Result;
use crate::output::{OutputFormat, ResultBuilder, print_result};
use serde_json::json;

pub fn run(
    ctx_state: &mut ContextState,
    format: OutputFormat,
    endpoint: Option<String>,
    clear: bool,
) -> Result<()> {
    if clear {
        ctx_state.set_cdp_endpoint(None);
        let result = ResultBuilder::<serde_json::Value>::new("connect")
            .data(json!({
                "action": "cleared",
                "message": "CDP endpoint cleared"
            }))
            .build();
        print_result(&result, format);
        return Ok(());
    }

    if let Some(ep) = endpoint {
        ctx_state.set_cdp_endpoint(Some(ep.clone()));
        let result = ResultBuilder::<serde_json::Value>::new("connect")
            .data(json!({
                "action": "set",
                "endpoint": ep,
                "message": format!("CDP endpoint set to {}", ep)
            }))
            .build();
        print_result(&result, format);
    } else {
        // Show current endpoint
        match ctx_state.cdp_endpoint() {
            Some(ep) => {
                let result = ResultBuilder::<serde_json::Value>::new("connect")
                    .data(json!({
                        "action": "show",
                        "endpoint": ep,
                        "message": format!("Current CDP endpoint: {}", ep)
                    }))
                    .build();
                print_result(&result, format);
            }
            None => {
                let result = ResultBuilder::<serde_json::Value>::new("connect")
                    .data(json!({
                        "action": "show",
                        "endpoint": null,
                        "message": "No CDP endpoint configured"
                    }))
                    .build();
                print_result(&result, format);
            }
        }
    }

    Ok(())
}
