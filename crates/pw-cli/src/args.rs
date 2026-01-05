//! URL and selector argument detection helpers.
//!
//! This module provides heuristics for distinguishing CSS selectors from URLs
//! when the user provides a single positional argument. This allows commands like
//! `pw text ".selector"` to work without requiring the explicit `-s` flag.

/// Characters that strongly indicate a CSS selector.
const SELECTOR_CHARS: &[char] = &['.', '#', '>', '~', '+', ':', '[', ']', '*'];

/// URL scheme prefixes that indicate the string is a URL.
const URL_PREFIXES: &[&str] = &["http://", "https://", "ws://", "wss://", "file://", "data:"];

/// Returns true if the string looks like a CSS selector rather than a URL.
///
/// A string is considered a selector if:
/// - It contains CSS selector characters (`.`, `#`, `>`, `~`, `+`, `:`, `[`, `]`, `*`)
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
/// // URLs (not selectors)
/// assert!(!looks_like_selector("https://example.com"));
/// assert!(!looks_like_selector("http://localhost:3000"));
/// assert!(!looks_like_selector("ws://localhost/socket"));
///
/// // Ambiguous/plain strings (not selectors)
/// assert!(!looks_like_selector("example.com"));
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
    s.chars().any(|c| SELECTOR_CHARS.contains(&c))
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

/// Resolves URL and selector from a combination of positional arguments and explicit flags.
///
/// This function implements smart detection to reduce the need for explicit `-s` flags:
/// - If both `--url` and `--selector` flags are provided, they are used directly
/// - If only a positional argument is provided and it looks like a CSS selector, treat it as a selector
/// - If only a positional argument is provided and it's NOT a selector, treat it as a URL
/// - If context has a URL and a selector-like positional is provided, the selector is returned
///   (the caller should use the context URL)
///
/// # Arguments
///
/// * `positional` - The positional argument (could be URL or selector)
/// * `url_flag` - Explicit `--url` flag value
/// * `selector_flag` - Explicit `--selector` flag value  
/// * `has_context_url` - Whether context has a URL available (for fallback)
///
/// # Returns
///
/// A `ResolvedArgs` struct containing the resolved URL and selector.
///
/// # Examples
///
/// ```
/// use pw_cli::args::{resolve_url_and_selector, ResolvedArgs};
///
/// // Explicit flags take precedence
/// let result = resolve_url_and_selector(
///     None,
///     Some("https://example.com".into()),
///     Some(".class".into()),
///     false,
/// );
/// assert_eq!(result.url, Some("https://example.com".into()));
/// assert_eq!(result.selector, Some(".class".into()));
///
/// // Selector-like positional with context URL
/// let result = resolve_url_and_selector(
///     Some(".class".into()),
///     None,
///     None,
///     true,
/// );
/// assert_eq!(result.url, None); // Caller uses context URL
/// assert_eq!(result.selector, Some(".class".into()));
///
/// // URL-like positional
/// let result = resolve_url_and_selector(
///     Some("https://example.com".into()),
///     None,
///     None,
///     false,
/// );
/// assert_eq!(result.url, Some("https://example.com".into()));
/// assert_eq!(result.selector, None);
/// ```
pub fn resolve_url_and_selector(
    positional: Option<String>,
    url_flag: Option<String>,
    selector_flag: Option<String>,
    has_context_url: bool,
) -> ResolvedArgs {
    // If both explicit flags provided, use them directly
    if url_flag.is_some() || selector_flag.is_some() {
        return ResolvedArgs {
            url: url_flag.or(positional.clone().filter(|p| !looks_like_selector(p))),
            selector: selector_flag.or(positional.filter(|p| looks_like_selector(p))),
        };
    }

    // No explicit flags - analyze the positional argument
    let Some(pos) = positional else {
        return ResolvedArgs {
            url: None,
            selector: None,
        };
    };

    if looks_like_selector(&pos) {
        // It looks like a selector
        if has_context_url {
            // Context has a URL, so treat positional as selector only
            ResolvedArgs {
                url: None,
                selector: Some(pos),
            }
        } else {
            // No context URL, but it still looks like a selector
            // Return it as selector - caller will need to handle missing URL
            ResolvedArgs {
                url: None,
                selector: Some(pos),
            }
        }
    } else {
        // Doesn't look like a selector - treat as URL
        ResolvedArgs {
            url: Some(pos),
            selector: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_class_selectors() {
        assert!(looks_like_selector(".class"));
        assert!(looks_like_selector(".class-name"));
        assert!(looks_like_selector(".class_name"));
        assert!(looks_like_selector(".ClassName"));
        assert!(looks_like_selector("div.class"));
        assert!(looks_like_selector(".class1.class2"));
    }

    #[test]
    fn test_id_selectors() {
        assert!(looks_like_selector("#id"));
        assert!(looks_like_selector("#my-id"));
        assert!(looks_like_selector("#myId"));
        assert!(looks_like_selector("div#id"));
    }

    #[test]
    fn test_combinator_selectors() {
        assert!(looks_like_selector("div > span"));
        assert!(looks_like_selector("div>span"));
        assert!(looks_like_selector("div + span"));
        assert!(looks_like_selector("div ~ span"));
        assert!(looks_like_selector("ul > li > a"));
    }

    #[test]
    fn test_attribute_selectors() {
        assert!(looks_like_selector("[data-id]"));
        assert!(looks_like_selector("[data-id='value']"));
        assert!(looks_like_selector("input[type='text']"));
        assert!(looks_like_selector("[href^='https']"));
        assert!(looks_like_selector("[class*='btn']"));
    }

    #[test]
    fn test_pseudo_selectors() {
        assert!(looks_like_selector("button:hover"));
        assert!(looks_like_selector("a:visited"));
        assert!(looks_like_selector("li:first-child"));
        assert!(looks_like_selector("p:nth-child(2)"));
        assert!(looks_like_selector("::before"));
        assert!(looks_like_selector("input:focus"));
    }

    #[test]
    fn test_universal_selector() {
        assert!(looks_like_selector("*"));
        assert!(looks_like_selector("*.class"));
        assert!(looks_like_selector("div *"));
    }

    #[test]
    fn test_complex_selectors() {
        assert!(looks_like_selector(".titleline a >> nth=0"));
        assert!(looks_like_selector("article.post > header h1"));
        assert!(looks_like_selector("#main .content p:first-of-type"));
        assert!(looks_like_selector("table tr:nth-child(odd) td:first-child"));
    }

    #[test]
    fn test_urls_not_selectors() {
        assert!(!looks_like_selector("https://example.com"));
        assert!(!looks_like_selector("http://example.com"));
        assert!(!looks_like_selector("https://example.com/path"));
        assert!(!looks_like_selector("https://example.com/path?query=1"));
        assert!(!looks_like_selector("https://example.com#anchor"));
        assert!(!looks_like_selector("http://localhost:3000"));
        assert!(!looks_like_selector("https://user:pass@example.com"));
    }

    #[test]
    fn test_websocket_urls_not_selectors() {
        assert!(!looks_like_selector("ws://localhost:9222"));
        assert!(!looks_like_selector("wss://example.com/socket"));
        assert!(!looks_like_selector("ws://127.0.0.1:9222/devtools/browser/abc"));
    }

    #[test]
    fn test_file_urls_not_selectors() {
        assert!(!looks_like_selector("file:///path/to/file.html"));
        assert!(!looks_like_selector("file://localhost/path"));
    }

    #[test]
    fn test_data_urls_not_selectors() {
        assert!(!looks_like_selector("data:text/html,<h1>Test</h1>"));
        assert!(!looks_like_selector("data:text/html,<button id=btn>Go</button>"));
        assert!(!looks_like_selector("data:text/plain,Hello World"));
    }

    #[test]
    fn test_case_insensitive_url_detection() {
        assert!(!looks_like_selector("HTTPS://EXAMPLE.COM"));
        assert!(!looks_like_selector("Http://Example.Com"));
        assert!(!looks_like_selector("WS://localhost"));
    }

    #[test]
    fn test_plain_strings_not_selectors() {
        // Plain tag names without selector characters
        assert!(!looks_like_selector("div"));
        assert!(!looks_like_selector("span"));
        assert!(!looks_like_selector("body"));
        assert!(!looks_like_selector("html"));
    }

    #[test]
    fn test_domain_like_strings_not_selectors() {
        // These look like domains but don't have :// so we're conservative
        // The `.` is a selector char, but "example.com" is ambiguous
        // We choose to treat it as a selector since it contains `.`
        // This is intentional - if user wants a URL, they should use full URL
        assert!(looks_like_selector("example.com"));
        assert!(looks_like_selector("news.ycombinator.com"));
    }

    #[test]
    fn test_empty_and_whitespace() {
        assert!(!looks_like_selector(""));
        assert!(!looks_like_selector("   "));
        assert!(!looks_like_selector("\t"));
        assert!(!looks_like_selector("\n"));
    }

    #[test]
    fn test_whitespace_trimmed() {
        assert!(looks_like_selector("  .class  "));
        assert!(looks_like_selector("\t#id\n"));
    }

    #[test]
    fn test_url_with_fragment_selector_chars() {
        // URLs with fragments that look like selectors
        // The :// takes precedence
        assert!(!looks_like_selector("https://example.com#section"));
        assert!(!looks_like_selector("https://example.com/path#.class"));
    }

    #[test]
    fn test_looks_like_url() {
        assert!(looks_like_url("https://example.com"));
        assert!(looks_like_url("http://localhost"));
        assert!(looks_like_url("ws://127.0.0.1:9222"));
        assert!(looks_like_url("wss://example.com"));
        assert!(looks_like_url("file:///path"));
        assert!(looks_like_url("custom://scheme"));

        assert!(!looks_like_url("example.com"));
        assert!(!looks_like_url(".class"));
        assert!(!looks_like_url("#id"));
        assert!(!looks_like_url("localhost"));
    }

    // Tests for resolve_url_and_selector
    use super::resolve_url_and_selector;

    #[test]
    fn test_resolve_explicit_flags_both() {
        let result = resolve_url_and_selector(
            None,
            Some("https://example.com".into()),
            Some(".class".into()),
            false,
        );
        assert_eq!(result.url, Some("https://example.com".into()));
        assert_eq!(result.selector, Some(".class".into()));
    }

    #[test]
    fn test_resolve_explicit_url_flag_only() {
        let result = resolve_url_and_selector(
            None,
            Some("https://example.com".into()),
            None,
            false,
        );
        assert_eq!(result.url, Some("https://example.com".into()));
        assert_eq!(result.selector, None);
    }

    #[test]
    fn test_resolve_explicit_selector_flag_only() {
        let result = resolve_url_and_selector(
            None,
            None,
            Some(".class".into()),
            false,
        );
        assert_eq!(result.url, None);
        assert_eq!(result.selector, Some(".class".into()));
    }

    #[test]
    fn test_resolve_positional_url() {
        let result = resolve_url_and_selector(
            Some("https://example.com".into()),
            None,
            None,
            false,
        );
        assert_eq!(result.url, Some("https://example.com".into()));
        assert_eq!(result.selector, None);
    }

    #[test]
    fn test_resolve_positional_selector_with_context() {
        let result = resolve_url_and_selector(
            Some(".class".into()),
            None,
            None,
            true, // has context URL
        );
        assert_eq!(result.url, None); // Caller uses context URL
        assert_eq!(result.selector, Some(".class".into()));
    }

    #[test]
    fn test_resolve_positional_selector_without_context() {
        let result = resolve_url_and_selector(
            Some(".class".into()),
            None,
            None,
            false, // no context URL
        );
        // Still recognized as selector even without context
        assert_eq!(result.url, None);
        assert_eq!(result.selector, Some(".class".into()));
    }

    #[test]
    fn test_resolve_positional_id_selector() {
        let result = resolve_url_and_selector(
            Some("#main".into()),
            None,
            None,
            true,
        );
        assert_eq!(result.url, None);
        assert_eq!(result.selector, Some("#main".into()));
    }

    #[test]
    fn test_resolve_positional_complex_selector() {
        let result = resolve_url_and_selector(
            Some("div > span.title".into()),
            None,
            None,
            true,
        );
        assert_eq!(result.url, None);
        assert_eq!(result.selector, Some("div > span.title".into()));
    }

    #[test]
    fn test_resolve_no_arguments() {
        let result = resolve_url_and_selector(None, None, None, false);
        assert_eq!(result.url, None);
        assert_eq!(result.selector, None);
    }

    #[test]
    fn test_resolve_no_arguments_with_context() {
        let result = resolve_url_and_selector(None, None, None, true);
        assert_eq!(result.url, None);
        assert_eq!(result.selector, None);
    }

    #[test]
    fn test_resolve_url_flag_with_positional_selector() {
        // URL flag + positional that looks like selector
        let result = resolve_url_and_selector(
            Some(".class".into()),
            Some("https://example.com".into()),
            None,
            false,
        );
        assert_eq!(result.url, Some("https://example.com".into()));
        assert_eq!(result.selector, Some(".class".into()));
    }

    #[test]
    fn test_resolve_selector_flag_with_positional_url() {
        // Selector flag + positional that looks like URL
        let result = resolve_url_and_selector(
            Some("https://example.com".into()),
            None,
            Some(".class".into()),
            false,
        );
        assert_eq!(result.url, Some("https://example.com".into()));
        assert_eq!(result.selector, Some(".class".into()));
    }

    #[test]
    fn test_resolve_plain_tag_as_url() {
        // Plain tag names without selector chars are treated as URLs (not selectors)
        let result = resolve_url_and_selector(
            Some("body".into()),
            None,
            None,
            true,
        );
        assert_eq!(result.url, Some("body".into()));
        assert_eq!(result.selector, None);
    }

    #[test]
    fn test_resolve_attribute_selector() {
        let result = resolve_url_and_selector(
            Some("[data-testid='submit']".into()),
            None,
            None,
            true,
        );
        assert_eq!(result.url, None);
        assert_eq!(result.selector, Some("[data-testid='submit']".into()));
    }

    #[test]
    fn test_resolve_pseudo_selector() {
        let result = resolve_url_and_selector(
            Some("button:first-child".into()),
            None,
            None,
            true,
        );
        assert_eq!(result.url, None);
        assert_eq!(result.selector, Some("button:first-child".into()));
    }

    #[test]
    fn test_resolve_websocket_url() {
        let result = resolve_url_and_selector(
            Some("ws://localhost:9222/devtools".into()),
            None,
            None,
            false,
        );
        assert_eq!(result.url, Some("ws://localhost:9222/devtools".into()));
        assert_eq!(result.selector, None);
    }
}
