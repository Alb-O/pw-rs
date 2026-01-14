use crate::context_store::ContextState;
use crate::error::Result;
use crate::output::{OutputFormat, ResultBuilder, print_result};
use serde_json::json;

pub fn add(ctx_state: &mut ContextState, format: OutputFormat, pattern: String) -> Result<()> {
    let added = ctx_state.add_protected(pattern.clone());

    let result = ResultBuilder::new("protect add")
        .data(json!({
            "added": added,
            "pattern": pattern,
            "protected": ctx_state.protected_urls(),
        }))
        .build();

    print_result(&result, format);
    Ok(())
}

pub fn remove(ctx_state: &mut ContextState, format: OutputFormat, pattern: &str) -> Result<()> {
    let removed = ctx_state.remove_protected(pattern);

    let result = ResultBuilder::new("protect remove")
        .data(json!({
            "removed": removed,
            "pattern": pattern,
            "protected": ctx_state.protected_urls(),
        }))
        .build();

    print_result(&result, format);
    Ok(())
}

pub fn list(ctx_state: &ContextState, format: OutputFormat) -> Result<()> {
    let patterns = ctx_state.protected_urls();

    let result = ResultBuilder::new("protect list")
        .data(json!({
            "protected": patterns,
            "count": patterns.len(),
        }))
        .build();

    print_result(&result, format);
    Ok(())
}
