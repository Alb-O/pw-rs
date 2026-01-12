//! URL and selector argument detection helpers.
//!
//! This module provides heuristics for distinguishing CSS selectors from URLs
//! when the user provides a single positional argument. This allows commands like
//! `pw text ".selector"` to work without requiring the explicit `-s` flag.

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
        assert!(looks_like_selector(
            "table tr:nth-child(odd) td:first-child"
        ));
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
        assert!(!looks_like_selector(
            "ws://127.0.0.1:9222/devtools/browser/abc"
        ));
    }

    #[test]
    fn test_file_urls_not_selectors() {
        assert!(!looks_like_selector("file:///path/to/file.html"));
        assert!(!looks_like_selector("file://localhost/path"));
    }

    #[test]
    fn test_data_urls_not_selectors() {
        assert!(!looks_like_selector("data:text/html,<h1>Test</h1>"));
        assert!(!looks_like_selector(
            "data:text/html,<button id=btn>Go</button>"
        ));
        assert!(!looks_like_selector("data:text/plain,Hello World"));
    }

    #[test]
    fn test_case_insensitive_url_detection() {
        assert!(!looks_like_selector("HTTPS://EXAMPLE.COM"));
        assert!(!looks_like_selector("Http://Example.Com"));
        assert!(!looks_like_selector("WS://localhost"));
    }

    #[test]
    fn test_html_tags_are_selectors() {
        // Common HTML tag names are recognized as selectors
        assert!(looks_like_selector("div"));
        assert!(looks_like_selector("span"));
        assert!(looks_like_selector("body"));
        assert!(looks_like_selector("html"));
        assert!(looks_like_selector("h1"));
        assert!(looks_like_selector("h2"));
        assert!(looks_like_selector("p"));
        assert!(looks_like_selector("a"));
        assert!(looks_like_selector("button"));
        assert!(looks_like_selector("input"));
        assert!(looks_like_selector("table"));
        assert!(looks_like_selector("form"));
        assert!(looks_like_selector("img"));
        assert!(looks_like_selector("nav"));
        assert!(looks_like_selector("article"));
        // Case insensitive
        assert!(looks_like_selector("DIV"));
        assert!(looks_like_selector("H1"));
        assert!(looks_like_selector("Body"));
    }

    #[test]
    fn test_unknown_strings_not_selectors() {
        // Random strings that aren't tags or CSS selectors
        assert!(!looks_like_selector("foobar"));
        assert!(!looks_like_selector("localhost"));
        assert!(!looks_like_selector("mycomponent"));
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

    #[test]
    fn test_resolve_explicit_flags() {
        let r = resolve_url_and_selector(None, Some("https://x.com".into()), Some(".c".into()));
        assert_eq!(r.url, Some("https://x.com".into()));
        assert_eq!(r.selector, Some(".c".into()));

        let r = resolve_url_and_selector(None, Some("https://x.com".into()), None);
        assert_eq!(r.url, Some("https://x.com".into()));
        assert_eq!(r.selector, None);

        let r = resolve_url_and_selector(None, None, Some(".c".into()));
        assert_eq!(r.url, None);
        assert_eq!(r.selector, Some(".c".into()));
    }

    #[test]
    fn test_resolve_positional_detection() {
        // URL detected
        let r = resolve_url_and_selector(Some("https://x.com".into()), None, None);
        assert_eq!(r.url, Some("https://x.com".into()));
        assert_eq!(r.selector, None);

        // Selector detected (class)
        let r = resolve_url_and_selector(Some(".class".into()), None, None);
        assert_eq!(r.url, None);
        assert_eq!(r.selector, Some(".class".into()));

        // Selector detected (id)
        let r = resolve_url_and_selector(Some("#main".into()), None, None);
        assert_eq!(r.url, None);
        assert_eq!(r.selector, Some("#main".into()));

        // Selector detected (complex)
        let r = resolve_url_and_selector(Some("div > span.title".into()), None, None);
        assert_eq!(r.url, None);
        assert_eq!(r.selector, Some("div > span.title".into()));

        // HTML tags are selectors
        let r = resolve_url_and_selector(Some("body".into()), None, None);
        assert_eq!(r.url, None);
        assert_eq!(r.selector, Some("body".into()));

        let r = resolve_url_and_selector(Some("h1".into()), None, None);
        assert_eq!(r.url, None);
        assert_eq!(r.selector, Some("h1".into()));

        // Unknown strings treated as URLs
        let r = resolve_url_and_selector(Some("foobar".into()), None, None);
        assert_eq!(r.url, Some("foobar".into()));
        assert_eq!(r.selector, None);
    }

    #[test]
    fn test_resolve_no_arguments() {
        let r = resolve_url_and_selector(None, None, None);
        assert_eq!(r.url, None);
        assert_eq!(r.selector, None);
    }

    #[test]
    fn test_resolve_flag_with_positional() {
        // URL flag + selector-like positional
        let r = resolve_url_and_selector(Some(".c".into()), Some("https://x.com".into()), None);
        assert_eq!(r.url, Some("https://x.com".into()));
        assert_eq!(r.selector, Some(".c".into()));

        // Selector flag + URL-like positional
        let r = resolve_url_and_selector(Some("https://x.com".into()), None, Some(".c".into()));
        assert_eq!(r.url, Some("https://x.com".into()));
        assert_eq!(r.selector, Some(".c".into()));
    }
}
