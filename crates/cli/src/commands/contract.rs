//! Shared command argument and output contract helpers.
//!
//! These helpers centralize repeated patterns across command modules:
//! * URL and selector resolution from positional args and flags
//! * Standard [`crate::output::CommandInputs`] construction
//! * Standard [`crate::commands::def::ContextDelta`] construction

use std::path::{Path, PathBuf};

use crate::args;
use crate::commands::def::ContextDelta;
use crate::error::Result;
use crate::output::CommandInputs;
use crate::target::{ResolveEnv, ResolvedTarget, TargetPolicy};

/// Resolve a target URL from positional and named URL forms.
pub fn resolve_target_from_url_pair(url: Option<String>, url_flag: Option<String>, env: &ResolveEnv<'_>, policy: TargetPolicy) -> Result<ResolvedTarget> {
	env.resolve_target(url_flag.or(url), policy)
}

/// Resolve target + selector using positional URL/selector heuristics.
///
/// This preserves existing `args::resolve_url_and_selector` behavior:
/// if a single positional argument looks like a selector, it is treated as one.
pub fn resolve_target_and_selector(
	positional_url: Option<String>,
	positional_selector: Option<String>,
	url_flag: Option<String>,
	selector_flag: Option<String>,
	env: &ResolveEnv<'_>,
	selector_fallback: Option<&str>,
) -> Result<(ResolvedTarget, String)> {
	let resolved = args::resolve_url_and_selector(positional_url, url_flag, selector_flag.or(positional_selector));
	let target = env.resolve_target(resolved.url, TargetPolicy::AllowCurrentPage)?;
	let selector = env.resolve_selector(resolved.selector, selector_fallback)?;
	Ok((target, selector))
}

/// Resolve target + selector where selector is explicit (no URL/selector heuristic split).
pub fn resolve_target_and_explicit_selector(
	url: Option<String>,
	url_flag: Option<String>,
	selector: Option<String>,
	selector_flag: Option<String>,
	env: &ResolveEnv<'_>,
	selector_fallback: Option<&str>,
) -> Result<(ResolvedTarget, String)> {
	let target = env.resolve_target(url_flag.or(url), TargetPolicy::AllowCurrentPage)?;
	let selector = env.resolve_selector(selector_flag.or(selector), selector_fallback)?;
	Ok((target, selector))
}

/// Build standard command input metadata.
pub fn standard_inputs(
	target: &ResolvedTarget,
	selector: Option<&str>,
	expression: Option<String>,
	output: Option<&Path>,
	extra: Option<serde_json::Value>,
) -> CommandInputs {
	CommandInputs {
		url: target.url_str().map(String::from),
		selector: selector.map(String::from),
		expression,
		output_path: output.map(PathBuf::from),
		extra,
	}
}

/// Build a standard context delta using the resolved target URL.
pub fn standard_delta(target: &ResolvedTarget, selector: Option<&str>, output: Option<&Path>) -> ContextDelta {
	standard_delta_with_url(target.url_str().map(String::from), selector, output)
}

/// Build a standard context delta with an explicit URL override.
pub fn standard_delta_with_url(url: Option<String>, selector: Option<&str>, output: Option<&Path>) -> ContextDelta {
	ContextDelta {
		url,
		selector: selector.map(String::from),
		output: output.map(PathBuf::from),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::target::{ResolvedTarget, Target, TargetSource};

	fn resolved_target(url: &str) -> ResolvedTarget {
		ResolvedTarget {
			target: Target::Navigate(url::Url::parse(url).unwrap()),
			source: TargetSource::Explicit,
		}
	}

	#[test]
	fn standard_inputs_populates_fields() {
		let target = resolved_target("https://example.com");
		let inputs = standard_inputs(
			&target,
			Some("#main"),
			Some("document.title".to_string()),
			Some(Path::new("shot.png")),
			Some(serde_json::json!({ "flag": true })),
		);

		assert_eq!(inputs.url.as_deref(), Some("https://example.com/"));
		assert_eq!(inputs.selector.as_deref(), Some("#main"));
		assert_eq!(inputs.expression.as_deref(), Some("document.title"));
		assert_eq!(inputs.output_path, Some(PathBuf::from("shot.png")));
		assert_eq!(inputs.extra, Some(serde_json::json!({ "flag": true })));
	}

	#[test]
	fn standard_delta_uses_target_url() {
		let target = resolved_target("https://example.com/page");
		let delta = standard_delta(&target, Some(".btn"), Some(Path::new("out.png")));
		assert_eq!(delta.url.as_deref(), Some("https://example.com/page"));
		assert_eq!(delta.selector.as_deref(), Some(".btn"));
		assert_eq!(delta.output, Some(PathBuf::from("out.png")));
	}

	#[test]
	fn standard_delta_with_url_overrides_target_url() {
		let delta = standard_delta_with_url(Some("https://override.com".to_string()), Some("#x"), None);
		assert_eq!(delta.url.as_deref(), Some("https://override.com"));
		assert_eq!(delta.selector.as_deref(), Some("#x"));
		assert!(delta.output.is_none());
	}
}
