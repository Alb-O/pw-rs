//! URL and selector argument detection helpers.
//!
//! This module provides heuristics for distinguishing CSS selectors from URLs
//! when the user provides a single positional argument. This allows commands like
//! `pw text ".selector"` to work without requiring the explicit `-s` flag.
//!
//! Also provides the [`choose`] helper for resolving positional vs flag arguments
//! with conflict detection.

#[cfg(test)]
mod tests;

/// Characters that strongly indicate a CSS selector.
const SELECTOR_CHARS: &[char] = &['.', '#', '>', '~', '+', ':', '[', ']', '*'];

/// URL scheme prefixes that indicate the string is a URL.
const URL_PREFIXES: &[&str] = &["http://", "https://", "ws://", "wss://", "file://", "data:"];

/// Common HTML tag names recognized as selectors (matched case-insensitively).
const HTML_TAGS: &[&str] = &[
	"html",
	"head",
	"body",
	"main",
	"header",
	"footer",
	"nav",
	"aside",
	"section",
	"article",
	"div",
	"span",
	"h1",
	"h2",
	"h3",
	"h4",
	"h5",
	"h6",
	"p",
	"a",
	"strong",
	"em",
	"b",
	"i",
	"u",
	"s",
	"small",
	"mark",
	"blockquote",
	"pre",
	"code",
	"kbd",
	"samp",
	"var",
	"cite",
	"q",
	"abbr",
	"time",
	"address",
	"sub",
	"sup",
	"ul",
	"ol",
	"li",
	"dl",
	"dt",
	"dd",
	"menu",
	"table",
	"thead",
	"tbody",
	"tfoot",
	"tr",
	"th",
	"td",
	"caption",
	"colgroup",
	"col",
	"form",
	"input",
	"button",
	"textarea",
	"select",
	"option",
	"optgroup",
	"label",
	"fieldset",
	"legend",
	"datalist",
	"output",
	"progress",
	"meter",
	"img",
	"picture",
	"source",
	"video",
	"audio",
	"track",
	"canvas",
	"svg",
	"figure",
	"figcaption",
	"iframe",
	"embed",
	"object",
	"param",
	"details",
	"summary",
	"dialog",
	"br",
	"hr",
	"wbr",
	"template",
	"slot",
	"noscript",
	"script",
	"style",
	"link",
	"meta",
];

/// Returns true if the string looks like a CSS selector rather than a URL.
///
/// A string is considered a selector if:
/// - It contains CSS selector characters (`.`, `#`, `>`, `~`, `+`, `:`, `[`, `]`, `*`)
/// - OR it exactly matches a common HTML tag name (case-insensitive)
/// - AND it does not look like a URL (no `://`, doesn't start with known URL schemes)
///
/// # Examples
///
/// ```
/// use pw_cli::args::looks_like_selector;
///
/// // Selectors
/// assert!(looks_like_selector(".class"));
/// assert!(looks_like_selector("#id"));
/// assert!(looks_like_selector("div > span"));
/// assert!(looks_like_selector("[data-id]"));
/// assert!(looks_like_selector("button:hover"));
/// assert!(looks_like_selector("*"));
///
/// // HTML tags are selectors
/// assert!(looks_like_selector("h1"));
/// assert!(looks_like_selector("div"));
/// assert!(looks_like_selector("body"));
///
/// // URLs (not selectors)
/// assert!(!looks_like_selector("https://example.com"));
/// assert!(!looks_like_selector("http://localhost:3000"));
/// assert!(!looks_like_selector("ws://localhost/socket"));
///
/// // Domain-like strings with dots ARE treated as selectors
/// // (use explicit URL flag if you need domain without scheme)
/// assert!(looks_like_selector("example.com"));
///
/// // Plain strings without selector chars (not selectors)
/// assert!(!looks_like_selector("localhost"));
/// ```
pub fn looks_like_selector(s: &str) -> bool {
	let s = s.trim();

	// Empty strings are not selectors
	if s.is_empty() {
		return false;
	}

	// Check if it looks like a URL first
	if looks_like_url(s) {
		return false;
	}

	// Check for CSS selector characters
	if s.chars().any(|c| SELECTOR_CHARS.contains(&c)) {
		return true;
	}

	// Check if it's a bare HTML tag name
	let lower = s.to_lowercase();
	HTML_TAGS.contains(&lower.as_str())
}

/// Returns true if the string looks like a URL.
///
/// A string is considered a URL if:
/// - It starts with a known URL scheme (http://, https://, ws://, wss://, file://)
/// - OR it contains `://` (indicating some other scheme)
fn looks_like_url(s: &str) -> bool {
	let lower = s.to_lowercase();

	// Check for known URL schemes
	for prefix in URL_PREFIXES {
		if lower.starts_with(prefix) {
			return true;
		}
	}

	// Check for any scheme indicator
	s.contains("://")
}

/// Resolved URL and selector from command arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedArgs {
	/// The URL to navigate to, if any.
	pub url: Option<String>,
	/// The selector to use, if any.
	pub selector: Option<String>,
}

/// Resolves URL and selector from positional arguments and explicit flags.
///
/// Smart detection reduces the need for explicit `-s` flags:
/// - Explicit `--url` and `--selector` flags take precedence
/// - Positional arguments that look like CSS selectors are treated as selectors
/// - Other positional arguments are treated as URLs
pub fn resolve_url_and_selector(
	positional: Option<String>,
	url_flag: Option<String>,
	selector_flag: Option<String>,
) -> ResolvedArgs {
	if url_flag.is_some() || selector_flag.is_some() {
		return ResolvedArgs {
			url: url_flag.or(positional.clone().filter(|p| !looks_like_selector(p))),
			selector: selector_flag.or(positional.filter(|p| looks_like_selector(p))),
		};
	}

	let Some(pos) = positional else {
		return ResolvedArgs {
			url: None,
			selector: None,
		};
	};

	if looks_like_selector(&pos) {
		ResolvedArgs {
			url: None,
			selector: Some(pos),
		}
	} else {
		ResolvedArgs {
			url: Some(pos),
			selector: None,
		}
	}
}

/// Error type for argument conflicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgConflict {
	/// Name of the argument that has a conflict.
	pub name: &'static str,
}

impl std::fmt::Display for ArgConflict {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"provide {} either positionally or via flag, not both",
			self.name
		)
	}
}

impl std::error::Error for ArgConflict {}

/// Chooses between a positional argument and a named flag.
///
/// This helper prevents the subtle bug where a user provides the same argument
/// both positionally and via a named flag, which would silently pick one and
/// ignore the other.
///
/// # Errors
///
/// Returns [`ArgConflict`] if both `positional` and `flag` are `Some`.
///
/// # Examples
///
/// ```
/// use pw_cli::args::choose;
///
/// // Only positional provided
/// assert_eq!(
///     choose(Some("https://example.com".to_string()), None, "url").unwrap(),
///     Some("https://example.com".to_string())
/// );
///
/// // Only flag provided
/// assert_eq!(
///     choose(None, Some("https://example.com".to_string()), "url").unwrap(),
///     Some("https://example.com".to_string())
/// );
///
/// // Neither provided
/// assert_eq!(choose::<String>(None, None, "url").unwrap(), None);
///
/// // Both provided - error
/// assert!(choose(
///     Some("https://a.com".to_string()),
///     Some("https://b.com".to_string()),
///     "url"
/// ).is_err());
/// ```
pub fn choose<T>(
	positional: Option<T>,
	flag: Option<T>,
	name: &'static str,
) -> Result<Option<T>, ArgConflict> {
	match (positional, flag) {
		(Some(_), Some(_)) => Err(ArgConflict { name }),
		(a, b) => Ok(a.or(b)),
	}
}
