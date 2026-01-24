//! Typed target resolution for browser navigation.
//!
//! This module replaces the magic `__CURRENT_PAGE__` sentinel string with a typed
//! [`Target`] enum, providing compile-time safety and clear semantics for navigation.

use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{PwError, Result};

/// The resolved navigation intention.
#[derive(Debug, Clone)]
pub enum Target {
	/// Navigate to this URL.
	Navigate(Url),
	/// Operate on whatever page is currently active (CDP mode).
	CurrentPage,
}

impl Target {
	/// Returns the URL if this is a `Navigate` target, `None` for `CurrentPage`.
	pub fn url(&self) -> Option<&Url> {
		match self {
			Target::Navigate(url) => Some(url),
			Target::CurrentPage => None,
		}
	}

	/// Returns the URL as a string, or `None` for `CurrentPage`.
	pub fn url_str(&self) -> Option<&str> {
		self.url().map(|u| u.as_str())
	}

	/// Returns `true` if this is `CurrentPage`.
	pub fn is_current_page(&self) -> bool {
		matches!(self, Target::CurrentPage)
	}
}

/// Where the target URL came from (for diagnostics).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetSource {
	/// User provided URL explicitly.
	Explicit,
	/// Fell back to context's last_url.
	ContextLastUrl,
	/// Fell back to context's base_url.
	BaseUrl,
	/// CDP mode default (no URL provided).
	CdpCurrentPageDefault,
}

impl std::fmt::Display for TargetSource {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			TargetSource::Explicit => write!(f, "explicit"),
			TargetSource::ContextLastUrl => write!(f, "context_last_url"),
			TargetSource::BaseUrl => write!(f, "base_url"),
			TargetSource::CdpCurrentPageDefault => write!(f, "cdp_current_page"),
		}
	}
}

/// Fully resolved target with provenance.
#[derive(Debug, Clone)]
pub struct ResolvedTarget {
	/// The resolved navigation target.
	pub target: Target,
	/// Where the target came from.
	pub source: TargetSource,
}

impl ResolvedTarget {
	/// Returns the URL if this is a `Navigate` target, `None` for `CurrentPage`.
	pub fn url(&self) -> Option<&Url> {
		self.target.url()
	}

	/// Returns the URL as a string, or `None` for `CurrentPage`.
	pub fn url_str(&self) -> Option<&str> {
		self.target.url_str()
	}

	/// Returns `true` if this is `CurrentPage`.
	pub fn is_current_page(&self) -> bool {
		self.target.is_current_page()
	}

	/// Returns the URL for page preference matching.
	///
	/// For `Navigate` targets, returns the URL. For `CurrentPage`, returns
	/// the provided `last_url` from context (if any) as a preference hint.
	pub fn preferred_url<'a>(&'a self, last_url: Option<&'a str>) -> Option<&'a str> {
		match &self.target {
			Target::Navigate(url) => Some(url.as_str()),
			Target::CurrentPage => last_url,
		}
	}
}

/// Policy for how to handle missing URLs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TargetPolicy {
	/// Error if URL not resolvable.
	RequireUrl,
	/// CDP + no url => CurrentPage (default for most commands).
	#[default]
	AllowCurrentPage,
}

/// Resolve a target URL from provided/context values.
///
/// # Resolution Order
///
/// 1. If `provided` URL is given, use it (apply base_url if relative).
/// 2. If CDP mode and policy is `AllowCurrentPage`, return `CurrentPage`.
/// 3. Fall back to `last_url` from context.
/// 4. Fall back to `base_url`.
/// 5. Error if no URL available.
///
/// # Arguments
///
/// * `provided` - User-provided URL (positional or flag).
/// * `base_url` - Base URL for resolving relative URLs.
/// * `last_url` - Last URL from context (for fallback).
/// * `has_cdp` - Whether a CDP endpoint is active.
/// * `policy` - How to handle missing URLs.
pub fn resolve_target(
	provided: Option<String>,
	base_url: Option<&str>,
	last_url: Option<&str>,
	has_cdp: bool,
	policy: TargetPolicy,
) -> Result<ResolvedTarget> {
	// 1. Explicit URL provided
	if let Some(u) = provided {
		let url = apply_base_url(&u, base_url)?;
		return Ok(ResolvedTarget {
			target: Target::Navigate(url),
			source: TargetSource::Explicit,
		});
	}

	// 2. CDP mode with AllowCurrentPage policy
	if has_cdp && policy == TargetPolicy::AllowCurrentPage {
		return Ok(ResolvedTarget {
			target: Target::CurrentPage,
			source: TargetSource::CdpCurrentPageDefault,
		});
	}

	// 3. Fall back to last_url
	if let Some(u) = last_url {
		let url = apply_base_url(u, base_url)?;
		return Ok(ResolvedTarget {
			target: Target::Navigate(url),
			source: TargetSource::ContextLastUrl,
		});
	}

	// 4. Fall back to base_url
	if let Some(b) = base_url {
		let url = Url::parse(b)
			.map_err(|e| PwError::Context(format!("invalid base URL '{}': {}", b, e)))?;
		return Ok(ResolvedTarget {
			target: Target::Navigate(url),
			source: TargetSource::BaseUrl,
		});
	}

	// 5. No URL available
	Err(PwError::Context(
		"No URL provided and no URL in context. \
         Use `pw navigate <url>` first to set context, or provide a URL explicitly."
			.into(),
	))
}

/// Apply base URL to a potentially relative URL.
fn apply_base_url(url: &str, base: Option<&str>) -> Result<Url> {
	// Check if URL is already absolute
	if is_absolute(url) {
		return Url::parse(url)
			.map_err(|e| PwError::Context(format!("invalid URL '{}': {}", url, e)));
	}

	// Relative URL needs a base
	let Some(base_str) = base else {
		return Err(PwError::Context(format!(
			"relative URL '{}' requires a base URL (use --base-url or set in context)",
			url
		)));
	};

	// Parse base and join
	let base_url = Url::parse(base_str)
		.map_err(|e| PwError::Context(format!("invalid base URL '{}': {}", base_str, e)))?;

	base_url.join(url).map_err(|e| {
		PwError::Context(format!(
			"failed to join '{}' with base '{}': {}",
			url, base_str, e
		))
	})
}

fn is_absolute(url: &str) -> bool {
	url.starts_with("http://")
		|| url.starts_with("https://")
		|| url.starts_with("ws://")
		|| url.starts_with("wss://")
		|| url.starts_with("file://")
		|| url.starts_with("data:")
}

// ---------------------------------------------------------------------------
// Argument Resolution Framework
// ---------------------------------------------------------------------------

use crate::context_store::ContextState;

/// Environment for resolving raw command arguments.
///
/// Provides access to context state and runtime flags needed during resolution.
pub struct ResolveEnv<'a> {
	/// Context state for URL/selector resolution.
	pub ctx_state: &'a ContextState,
	/// Whether a CDP endpoint is active.
	pub has_cdp: bool,
	/// Command name (for error messages).
	pub command: &'static str,
}

impl<'a> ResolveEnv<'a> {
	/// Create a new resolution environment.
	pub fn new(ctx_state: &'a ContextState, has_cdp: bool, command: &'static str) -> Self {
		Self {
			ctx_state,
			has_cdp,
			command,
		}
	}

	/// Resolve a target URL using context and CDP state.
	pub fn resolve_target(
		&self,
		provided: Option<String>,
		policy: TargetPolicy,
	) -> Result<ResolvedTarget> {
		resolve_target(
			provided,
			self.ctx_state.base_url(),
			self.ctx_state.last_url(),
			self.has_cdp,
			policy,
		)
	}

	/// Resolve a selector with optional fallback.
	pub fn resolve_selector(
		&self,
		provided: Option<String>,
		fallback: Option<&str>,
	) -> Result<String> {
		self.ctx_state.resolve_selector(provided, fallback)
	}
}

/// Trait for resolving raw command arguments into ready-to-execute arguments.
///
/// Each command defines a `*Raw` struct (from CLI/JSON) and a `*Resolved` struct
/// (ready for execution). This trait bridges them with consistent resolution logic.
///
/// # Example
///
/// ```ignore
/// impl Resolve for HtmlRaw {
///     type Output = HtmlResolved;
///     
///     fn resolve(self, env: &ResolveEnv<'_>) -> Result<HtmlResolved> {
///         let target = env.resolve_target(self.url, TargetPolicy::AllowCurrentPage)?;
///         let selector = env.resolve_selector(self.selector, Some("html"))?;
///         Ok(HtmlResolved { target, selector })
///     }
/// }
/// ```
pub trait Resolve {
	/// The resolved output type.
	type Output;

	/// Resolve raw arguments into ready-to-execute arguments.
	fn resolve(self, env: &ResolveEnv<'_>) -> Result<Self::Output>;
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn explicit_url_takes_precedence() {
		let result = resolve_target(
			Some("https://example.com".into()),
			Some("https://base.com"),
			Some("https://last.com"),
			true,
			TargetPolicy::AllowCurrentPage,
		)
		.unwrap();

		assert!(matches!(result.target, Target::Navigate(_)));
		assert_eq!(result.source, TargetSource::Explicit);
		assert_eq!(result.url_str(), Some("https://example.com/"));
	}

	#[test]
	fn cdp_mode_returns_current_page() {
		let result = resolve_target(
			None,
			Some("https://base.com"),
			Some("https://last.com"),
			true,
			TargetPolicy::AllowCurrentPage,
		)
		.unwrap();

		assert!(result.is_current_page());
		assert_eq!(result.source, TargetSource::CdpCurrentPageDefault);
	}

	#[test]
	fn cdp_mode_require_url_falls_back_to_last() {
		let result = resolve_target(
			None,
			Some("https://base.com"),
			Some("https://last.com"),
			true,
			TargetPolicy::RequireUrl,
		)
		.unwrap();

		assert!(matches!(result.target, Target::Navigate(_)));
		assert_eq!(result.source, TargetSource::ContextLastUrl);
		assert_eq!(result.url_str(), Some("https://last.com/"));
	}

	#[test]
	fn falls_back_to_last_url() {
		let result = resolve_target(
			None,
			None,
			Some("https://last.com"),
			false,
			TargetPolicy::AllowCurrentPage,
		)
		.unwrap();

		assert_eq!(result.source, TargetSource::ContextLastUrl);
		assert_eq!(result.url_str(), Some("https://last.com/"));
	}

	#[test]
	fn falls_back_to_base_url() {
		let result = resolve_target(
			None,
			Some("https://base.com"),
			None,
			false,
			TargetPolicy::AllowCurrentPage,
		)
		.unwrap();

		assert_eq!(result.source, TargetSource::BaseUrl);
		assert_eq!(result.url_str(), Some("https://base.com/"));
	}

	#[test]
	fn error_when_no_url_available() {
		let result = resolve_target(None, None, None, false, TargetPolicy::AllowCurrentPage);

		assert!(result.is_err());
	}

	#[test]
	fn relative_url_joined_with_base() {
		let result = resolve_target(
			Some("/path/to/page".into()),
			Some("https://example.com"),
			None,
			false,
			TargetPolicy::AllowCurrentPage,
		)
		.unwrap();

		assert_eq!(result.url_str(), Some("https://example.com/path/to/page"));
	}

	#[test]
	fn relative_url_without_base_errors() {
		let result = resolve_target(
			Some("/path/to/page".into()),
			None,
			None,
			false,
			TargetPolicy::AllowCurrentPage,
		);

		assert!(result.is_err());
	}

	#[test]
	fn preferred_url_for_navigate() {
		let result = resolve_target(
			Some("https://example.com".into()),
			None,
			Some("https://last.com"),
			false,
			TargetPolicy::AllowCurrentPage,
		)
		.unwrap();

		assert_eq!(
			result.preferred_url(Some("https://last.com")),
			Some("https://example.com/")
		);
	}

	#[test]
	fn preferred_url_for_current_page_uses_last() {
		let result =
			resolve_target(None, None, None, true, TargetPolicy::AllowCurrentPage).unwrap();

		assert_eq!(
			result.preferred_url(Some("https://last.com")),
			Some("https://last.com")
		);
		assert_eq!(result.preferred_url(None), None);
	}

	#[test]
	fn data_url_is_absolute() {
		let result = resolve_target(
			Some("data:text/html,<h1>Test</h1>".into()),
			None,
			None,
			false,
			TargetPolicy::AllowCurrentPage,
		)
		.unwrap();

		assert_eq!(result.source, TargetSource::Explicit);
		assert_eq!(result.url_str(), Some("data:text/html,<h1>Test</h1>"));
	}
}
