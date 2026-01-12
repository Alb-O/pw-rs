use std::path::Path;

use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::output::{
    CommandInputs, ErrorCode, FailureWithArtifacts, OutputFormat, ResultBuilder, TextData,
    print_failure_with_artifacts, print_result,
};
use crate::session_broker::{SessionBroker, SessionHandle, SessionRequest};
use pw::WaitUntil;
use tracing::info;

/// Heuristically detect if a line looks like minified JavaScript or garbage
fn is_garbage_line(line: &str) -> bool {
    let trimmed = line.trim();

    // Skip empty lines (not garbage, just empty)
    if trimmed.is_empty() {
        return false;
    }

    // Long lines with few spaces suggest minified code
    if trimmed.len() > 200 {
        let space_ratio =
            trimmed.chars().filter(|c| c.is_whitespace()).count() as f32 / trimmed.len() as f32;
        if space_ratio < 0.05 {
            return true;
        }
    }

    // High density of JS syntax characters
    let js_chars = trimmed
        .chars()
        .filter(|c| matches!(c, '{' | '}' | ';' | '(' | ')' | '=' | ',' | ':' | '[' | ']'))
        .count();
    if trimmed.len() > 50 && js_chars as f32 / trimmed.len() as f32 > 0.15 {
        return true;
    }

    // Common JS/CSS patterns
    let lower = trimmed.to_lowercase();
    if lower.starts_with("function(")
        || lower.starts_with("!function")
        || lower.starts_with("(function")
        || lower.contains("use strict")
        || lower.contains("sourcemappingurl")
        || lower.contains("data:image/")
        || lower.contains("data:application/")
        || lower.starts_with("var ") && trimmed.contains("function")
        || lower.starts_with("const ") && trimmed.contains("=>")
        || (trimmed.contains("&&") && trimmed.contains("||") && trimmed.len() > 100)
    {
        return true;
    }

    // Base64-like long strings (no spaces, alphanumeric heavy)
    if trimmed.len() > 100 && !trimmed.contains(' ') {
        let alnum_ratio =
            trimmed.chars().filter(|c| c.is_alphanumeric()).count() as f32 / trimmed.len() as f32;
        if alnum_ratio > 0.9 {
            return true;
        }
    }

    false
}

/// Filter out garbage lines from extracted text, collapsing multiple blank lines
fn filter_garbage(text: &str) -> String {
    let filtered: Vec<&str> = text.lines().filter(|line| !is_garbage_line(line)).collect();

    // Collapse runs of empty lines into single blank lines
    let mut result = Vec::new();
    let mut prev_empty = false;
    for line in filtered {
        let is_empty = line.trim().is_empty();
        if is_empty && prev_empty {
            continue;
        }
        result.push(line);
        prev_empty = is_empty;
    }

    result.join("\n")
}

pub async fn execute(
    url: &str,
    selector: &str,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    artifacts_dir: Option<&Path>,
    preferred_url: Option<&str>,
) -> Result<()> {
    info!(target = "pw", %url, %selector, browser = %ctx.browser, "get text");

    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(preferred_url),
        )
        .await?;

    match execute_inner(&session, url, selector, format).await {
        Ok(()) => session.close().await,
        Err(e) => {
            // Collect artifacts on failure if artifacts_dir is set
            let artifacts = session
                .collect_failure_artifacts(artifacts_dir, "text")
                .await;

            if !artifacts.is_empty() {
                // Print failure with artifacts and signal that output is complete
                let failure = FailureWithArtifacts::new(e.to_command_error())
                    .with_artifacts(artifacts.artifacts);
                print_failure_with_artifacts("text", &failure, format);
                let _ = session.close().await;
                return Err(PwError::OutputAlreadyPrinted);
            }

            let _ = session.close().await;
            Err(e)
        }
    }
}

async fn execute_inner(
    session: &SessionHandle,
    url: &str,
    selector: &str,
    format: OutputFormat,
) -> Result<()> {
    session.goto_unless_current(url).await?;

    let locator = session.page().locator(selector).await;
    let count = locator.count().await?;

    if count == 0 {
        let result = ResultBuilder::<TextData>::new("text")
            .inputs(CommandInputs {
                url: Some(url.to_string()),
                selector: Some(selector.to_string()),
                ..Default::default()
            })
            .error(
                ErrorCode::SelectorNotFound,
                format!("No elements matched selector: {selector}"),
            )
            .build();

        print_result(&result, format);

        return Err(PwError::ElementNotFound {
            selector: selector.to_string(),
        });
    }

    let text = locator.inner_text().await?;
    let filtered = filter_garbage(&text);
    let trimmed = filtered.trim().to_string();

    let result = ResultBuilder::new("text")
        .inputs(CommandInputs {
            url: Some(url.to_string()),
            selector: Some(selector.to_string()),
            ..Default::default()
        })
        .data(TextData {
            text: trimmed,
            selector: selector.to_string(),
            match_count: count,
        })
        .build();

    print_result(&result, format);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_minified_js() {
        let minified = "var a=function(){return b.call(c,d)};var e=f.g(h,i,j,k,l,m,n,o,p);";
        assert!(is_garbage_line(minified));
    }

    #[test]
    fn filters_iife_patterns() {
        assert!(is_garbage_line("(function(){console.log('x')})()"));
        assert!(is_garbage_line("!function(a,b){return a+b}()"));
        assert!(is_garbage_line("function(e,t){return e+t}"));
    }

    #[test]
    fn filters_base64_data() {
        let base64 = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB";
        assert!(is_garbage_line(base64));
    }

    #[test]
    fn filters_source_maps() {
        assert!(is_garbage_line("//# sourceMappingURL=app.js.map"));
    }

    #[test]
    fn preserves_normal_text() {
        assert!(!is_garbage_line("Welcome to our website"));
        assert!(!is_garbage_line(
            "Click here to learn more about our products."
        ));
        assert!(!is_garbage_line("Copyright 2024 Company Inc."));
        assert!(!is_garbage_line(""));
    }

    #[test]
    fn preserves_short_code_snippets() {
        // Short inline code in articles should be preserved
        assert!(!is_garbage_line("const x = 5;"));
        assert!(!is_garbage_line("function hello() {}"));
    }

    #[test]
    fn filters_long_no_space_lines() {
        let long_minified = "a".repeat(250);
        assert!(is_garbage_line(&long_minified));
    }

    #[test]
    fn filter_garbage_preserves_structure() {
        let input = "Welcome\n\nfunction(a,b){return a}\n\nGoodbye";
        let output = filter_garbage(input);
        // Garbage removed, consecutive blank lines collapsed
        assert_eq!(output, "Welcome\n\nGoodbye");
    }

    #[test]
    fn filter_garbage_collapses_multiple_blanks() {
        let input = "Hello\n\n\n\nWorld";
        let output = filter_garbage(input);
        assert_eq!(output, "Hello\n\nWorld");
    }
}
