//! Text content extraction command.

use std::path::Path;

use crate::args;
use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::output::{
    CommandInputs, ErrorCode, FailureWithArtifacts, OutputFormat, ResultBuilder, TextData,
    print_failure_with_artifacts, print_result,
};
use crate::session_broker::{SessionBroker, SessionHandle, SessionRequest};
use crate::target::{Resolve, ResolveEnv, ResolvedTarget, TargetPolicy};
use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

// ---------------------------------------------------------------------------
// Raw and Resolved Types
// ---------------------------------------------------------------------------

/// Raw inputs from CLI or batch JSON.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRaw {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default, alias = "url_flag")]
    pub url_flag: Option<String>,
    #[serde(default, alias = "selector_flag")]
    pub selector_flag: Option<String>,
}

impl TextRaw {
    pub fn from_cli(
        url: Option<String>,
        selector: Option<String>,
        url_flag: Option<String>,
        selector_flag: Option<String>,
    ) -> Self {
        Self {
            url,
            selector,
            url_flag,
            selector_flag,
        }
    }
}

/// Resolved inputs ready for execution.
#[derive(Debug, Clone)]
pub struct TextResolved {
    pub target: ResolvedTarget,
    pub selector: String,
}

impl TextResolved {
    pub fn preferred_url<'a>(&'a self, last_url: Option<&'a str>) -> Option<&'a str> {
        self.target.preferred_url(last_url)
    }
}

impl Resolve for TextRaw {
    type Output = TextResolved;

    fn resolve(self, env: &ResolveEnv<'_>) -> Result<TextResolved> {
        let resolved = args::resolve_url_and_selector(
            self.url.clone(),
            self.url_flag,
            self.selector_flag.or(self.selector),
        );

        let target = env.resolve_target(resolved.url, TargetPolicy::AllowCurrentPage)?;
        let selector = env.resolve_selector(resolved.selector, None)?;

        Ok(TextResolved { target, selector })
    }
}

// ---------------------------------------------------------------------------
// Garbage Filtering
// ---------------------------------------------------------------------------

/// Heuristically detect if a line looks like minified JavaScript or garbage
fn is_garbage_line(line: &str) -> bool {
    let trimmed = line.trim();

    if trimmed.is_empty() {
        return false;
    }

    if trimmed.len() > 200 {
        let space_ratio =
            trimmed.chars().filter(|c| c.is_whitespace()).count() as f32 / trimmed.len() as f32;
        if space_ratio < 0.05 {
            return true;
        }
    }

    let js_chars = trimmed
        .chars()
        .filter(|c| matches!(c, '{' | '}' | ';' | '(' | ')' | '=' | ',' | ':' | '[' | ']'))
        .count();
    if trimmed.len() > 50 && js_chars as f32 / trimmed.len() as f32 > 0.15 {
        return true;
    }

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

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

/// Execute the text command with resolved arguments.
pub async fn execute_resolved(
    args: &TextResolved,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    artifacts_dir: Option<&Path>,
    last_url: Option<&str>,
) -> Result<()> {
    let url_display = args.target.url_str().unwrap_or("<current page>");
    info!(target = "pw", url = %url_display, selector = %args.selector, browser = %ctx.browser, "get text");

    let preferred_url = args.preferred_url(last_url);
    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(preferred_url),
        )
        .await?;

    match execute_inner(
        &session,
        &args.target,
        &args.selector,
        format,
        ctx.timeout_ms(),
    )
    .await
    {
        Ok(()) => session.close().await,
        Err(e) => {
            let artifacts = session
                .collect_failure_artifacts(artifacts_dir, "text")
                .await;

            if !artifacts.is_empty() {
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
    target: &ResolvedTarget,
    selector: &str,
    format: OutputFormat,
    timeout_ms: Option<u64>,
) -> Result<()> {
    session.goto_target(&target.target, timeout_ms).await?;

    let locator = session.page().locator(selector).await;
    let count = locator.count().await?;

    if count == 0 {
        let result = ResultBuilder::<TextData>::new("text")
            .inputs(CommandInputs {
                url: target.url_str().map(String::from),
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
            url: target.url_str().map(String::from),
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
        assert_eq!(output, "Welcome\n\nGoodbye");
    }

    #[test]
    fn filter_garbage_collapses_multiple_blanks() {
        let input = "Hello\n\n\n\nWorld";
        let output = filter_garbage(input);
        assert_eq!(output, "Hello\n\nWorld");
    }

    #[test]
    fn text_raw_deserialize() {
        let json = r#"{"url": "https://example.com", "selector": "main"}"#;
        let raw: TextRaw = serde_json::from_str(json).unwrap();
        assert_eq!(raw.url, Some("https://example.com".into()));
        assert_eq!(raw.selector, Some("main".into()));
    }
}
