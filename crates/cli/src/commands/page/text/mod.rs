//! Text content extraction command.

use clap::Args;
use pw_rs::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::commands::contract::{resolve_target_and_selector, standard_delta, standard_inputs};
use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ExecCtx};
use crate::commands::exec_flow::navigation_plan;
use crate::error::{PwError, Result};
use crate::output::TextData;
use crate::session_helpers::{ArtifactsPolicy, with_session};
use crate::target::{ResolveEnv, ResolvedTarget};

/// Raw inputs from CLI or batch JSON.
#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRaw {
	/// Target URL (positional)
	#[serde(default)]
	pub url: Option<String>,

	/// CSS selector (positional)
	#[serde(default)]
	pub selector: Option<String>,

	/// Target URL (named alternative)
	#[arg(long = "url", short = 'u', value_name = "URL")]
	#[serde(default, alias = "url_flag")]
	pub url_flag: Option<String>,

	/// CSS selector (named alternative)
	#[arg(long = "selector", short = 's', value_name = "SELECTOR")]
	#[serde(default, alias = "selector_flag")]
	pub selector_flag: Option<String>,
}

/// Resolved inputs ready for execution.
#[derive(Debug, Clone)]
pub struct TextResolved {
	pub target: ResolvedTarget,
	pub selector: String,
}

pub struct TextCommand;

impl CommandDef for TextCommand {
	const NAME: &'static str = "page.text";

	type Raw = TextRaw;
	type Resolved = TextResolved;
	type Data = TextData;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		let (target, selector) = resolve_target_and_selector(raw.url, raw.selector, raw.url_flag, raw.selector_flag, env, None)?;

		Ok(TextResolved { target, selector })
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, mut exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let url_display = args.target.url_str().unwrap_or("<current page>");
			info!(target = "pw", url = %url_display, selector = %args.selector, browser = %exec.ctx.browser, "get text");

			let plan = navigation_plan(exec.ctx, exec.last_url, &args.target, WaitUntil::NetworkIdle);
			let timeout_ms = plan.timeout_ms;
			let target = plan.target;
			let selector = args.selector.clone();

			let data = with_session(&mut exec, plan.request, ArtifactsPolicy::Never, move |session| {
				let selector = selector.clone();
				Box::pin(async move {
					session.goto_target(&target, timeout_ms).await?;

					let locator = session.page().locator(&selector).await;
					let count = locator.count().await?;

					if count == 0 {
						return Err(PwError::ElementNotFound { selector });
					}

					let text = locator.inner_text().await?;
					let filtered = filter_garbage(&text);
					let trimmed = filtered.trim().to_string();

					Ok(TextData {
						text: trimmed,
						selector,
						match_count: count,
					})
				})
			})
			.await?;

			let inputs = standard_inputs(&args.target, Some(&args.selector), None, None, None);

			Ok(CommandOutcome {
				inputs,
				data,
				delta: standard_delta(&args.target, Some(&args.selector), None),
			})
		})
	}
}

/// Heuristically detect if a line looks like minified JavaScript or garbage
fn is_garbage_line(line: &str) -> bool {
	let trimmed = line.trim();

	if trimmed.is_empty() {
		return false;
	}

	if trimmed.len() > 200 {
		let space_ratio = trimmed.chars().filter(|c| c.is_whitespace()).count() as f32 / trimmed.len() as f32;
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
		let alnum_ratio = trimmed.chars().filter(|c| c.is_alphanumeric()).count() as f32 / trimmed.len() as f32;
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
		assert!(!is_garbage_line("Click here to learn more about our products."));
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
