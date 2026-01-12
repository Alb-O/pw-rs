//! Readable content extraction from HTML pages.
//!
//! This module extracts the main article content from web pages,
//! removing clutter like ads, navigation, sidebars, and footers.

use regex_lite::Regex;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::LazyLock;

/// Clutter patterns loaded from clutter.json at compile time
static CLUTTER: LazyLock<ClutterPatterns> = LazyLock::new(|| {
    let json = include_str!("clutter.json");
    serde_json::from_str(json).expect("Failed to parse clutter.json")
});

/// Pre-compiled regex for partial pattern matching
static PARTIAL_PATTERN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    let all_patterns: Vec<String> = CLUTTER
        .remove
        .partial_patterns
        .patterns
        .values()
        .flatten()
        .map(|s| regex_lite::escape(s))
        .collect();

    if all_patterns.is_empty() {
        // Fallback pattern that never matches
        Regex::new(r"(?!.*)").unwrap()
    } else {
        let pattern = all_patterns.join("|");
        Regex::new(&format!("(?i){}", pattern)).unwrap()
    }
});

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ClutterPatterns {
    content_selectors: ContentSelectors,
    remove: RemovePatterns,
    preserve: PreservePatterns,
    scoring: ScoringPatterns,
    junk_text: JunkTextPatterns,
}

#[derive(Debug, Deserialize)]
struct ContentSelectors {
    selectors: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct RemovePatterns {
    exact_selectors: Vec<String>,
    partial_patterns: PartialPatterns,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct PartialPatterns {
    check_attributes: Vec<String>,
    patterns: std::collections::HashMap<String, Vec<String>>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct PreservePatterns {
    preserve_elements: Vec<String>,
    inline_elements: Vec<String>,
    allowed_empty: Vec<String>,
    allowed_attributes: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ScoringPatterns {
    content_indicators: Vec<String>,
    navigation_indicators: Vec<String>,
    non_content_patterns: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct JunkTextPatterns {
    exact: Vec<String>,
}

/// Metadata extracted from the page
#[derive(Debug, Default, Clone)]
pub struct PageMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub published: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub site: Option<String>,
}

/// Result of readable content extraction
#[derive(Debug)]
pub struct ReadableContent {
    /// Clean HTML content
    pub html: String,
    /// Plain text content
    pub text: String,
    /// Markdown content (if conversion succeeded)
    pub markdown: Option<String>,
    /// Page metadata
    pub metadata: PageMetadata,
}

/// Extract readable content from HTML
pub fn extract_readable(html: &str, url: Option<&str>) -> ReadableContent {
    // Extract metadata first (before we modify the HTML)
    let metadata = extract_metadata(html, url);

    // Remove clutter
    let cleaned_html = remove_clutter(html);

    // Convert to text
    let text = html_to_text(&cleaned_html);

    // Simple markdown conversion (basic for now)
    let markdown = Some(html_to_markdown(&cleaned_html));

    ReadableContent {
        html: cleaned_html,
        text,
        markdown,
        metadata,
    }
}

/// Extract metadata from HTML
fn extract_metadata(html: &str, url: Option<&str>) -> PageMetadata {
    let mut metadata = PageMetadata::default();

    // Extract title
    metadata.title = extract_meta_content(html, "og:title")
        .or_else(|| extract_meta_content(html, "twitter:title"))
        .or_else(|| extract_title_tag(html));

    // Extract author
    metadata.author = extract_meta_content(html, "author")
        .or_else(|| extract_meta_content(html, "article:author"));

    // Extract description
    metadata.description = extract_meta_content(html, "og:description")
        .or_else(|| extract_meta_content(html, "description"))
        .or_else(|| extract_meta_content(html, "twitter:description"));

    // Extract image
    metadata.image = extract_meta_content(html, "og:image")
        .or_else(|| extract_meta_content(html, "twitter:image"));

    // Extract site name
    metadata.site = extract_meta_content(html, "og:site_name")
        .or_else(|| extract_meta_content(html, "twitter:site"));

    // Extract published date
    metadata.published = extract_meta_content(html, "article:published_time")
        .or_else(|| extract_meta_content(html, "datePublished"));

    // Extract domain from URL
    if metadata.site.is_none() {
        if let Some(u) = url {
            metadata.site = extract_domain(u);
        }
    }

    metadata
}

/// Extract content from a meta tag
fn extract_meta_content(html: &str, name: &str) -> Option<String> {
    // Try property attribute (og:*, article:*)
    let property_pattern = format!(
        r#"<meta[^>]*property=["']{}["'][^>]*content=["']([^"']+)["']"#,
        regex_lite::escape(name)
    );
    if let Some(caps) = Regex::new(&property_pattern).ok()?.captures(html) {
        return Some(decode_html_entities(caps.get(1)?.as_str()));
    }

    // Try content first, then property (different order)
    let property_pattern2 = format!(
        r#"<meta[^>]*content=["']([^"']+)["'][^>]*property=["']{}["']"#,
        regex_lite::escape(name)
    );
    if let Some(caps) = Regex::new(&property_pattern2).ok()?.captures(html) {
        return Some(decode_html_entities(caps.get(1)?.as_str()));
    }

    // Try name attribute
    let name_pattern = format!(
        r#"<meta[^>]*name=["']{}["'][^>]*content=["']([^"']+)["']"#,
        regex_lite::escape(name)
    );
    if let Some(caps) = Regex::new(&name_pattern).ok()?.captures(html) {
        return Some(decode_html_entities(caps.get(1)?.as_str()));
    }

    // Try content first, then name
    let name_pattern2 = format!(
        r#"<meta[^>]*content=["']([^"']+)["'][^>]*name=["']{}["']"#,
        regex_lite::escape(name)
    );
    if let Some(caps) = Regex::new(&name_pattern2).ok()?.captures(html) {
        return Some(decode_html_entities(caps.get(1)?.as_str()));
    }

    None
}

/// Extract title from <title> tag
fn extract_title_tag(html: &str) -> Option<String> {
    static TITLE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"<title[^>]*>([^<]+)</title>").unwrap());

    TITLE_RE
        .captures(html)
        .and_then(|c| c.get(1))
        .map(|m| decode_html_entities(m.as_str().trim()))
}

/// Decode HTML entities
fn decode_html_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
}

/// Extract domain from a URL string
fn extract_domain(url: &str) -> Option<String> {
    // Simple URL parsing without external crate
    let url = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let domain = url.split('/').next()?;
    let domain = domain.split(':').next()?; // Remove port if present
    Some(domain.trim_start_matches("www.").to_string())
}

/// Remove clutter from HTML using the patterns from clutter.json
fn remove_clutter(html: &str) -> String {
    let mut result = html.to_string();

    // Remove script and style tags first
    result = remove_tags(&result, &["script", "style", "noscript", "svg"]);

    // Remove elements by tag name
    let remove_tags_list = [
        "nav", "header", "footer", "aside", "form", "button", "input", "select", "textarea",
        "iframe",
    ];
    result = remove_tags(&result, &remove_tags_list);

    // Remove elements with clutter classes/ids
    result = remove_elements_by_attribute(&result);

    // Try to find and extract main content
    result = extract_main_content(&result);

    // Clean up whitespace
    result = clean_whitespace(&result);

    result
}

/// Remove specified HTML tags and their content
fn remove_tags(html: &str, tags: &[&str]) -> String {
    let mut result = html.to_string();

    for tag in tags {
        // Remove both self-closing and paired tags
        let pattern = format!(r"(?is)<{0}[^>]*>.*?</{0}>|<{0}[^>]*/?>", tag);
        if let Ok(re) = Regex::new(&pattern) {
            result = re.replace_all(&result, "").to_string();
        }
    }

    result
}

/// Remove elements that have clutter-indicating classes or IDs
fn remove_elements_by_attribute(html: &str) -> String {
    // Build a set of non-content patterns for quick lookup
    let non_content: HashSet<&str> = CLUTTER
        .scoring
        .non_content_patterns
        .iter()
        .map(|s| s.as_str())
        .collect();

    let mut result = html.to_string();

    // Remove elements whose class or id contains clutter patterns
    // Process each tag type separately to avoid backreferences
    let tags = ["div", "section", "aside", "span", "ul", "ol", "article"];

    for tag in tags {
        // Pattern to match the complete element with its content
        let element_pattern = format!(
            r#"(?is)<{tag}[^>]*(class|id)=["']([^"']+)["'][^>]*>.*?</{tag}>"#,
            tag = tag
        );

        if let Ok(element_re) = Regex::new(&element_pattern) {
            // Process from innermost to outermost by iterating multiple times
            for _ in 0..3 {
                let mut changed = false;

                // Use replace_all with a closure to check each match
                let new_result = element_re.replace_all(&result, |caps: &regex_lite::Captures| {
                    if let Some(attr_value) = caps.get(2) {
                        let attr_lower = attr_value.as_str().to_lowercase();

                        // Check if this element should be removed
                        let should_remove = non_content.iter().any(|p| attr_lower.contains(p))
                            || PARTIAL_PATTERN_REGEX.is_match(&attr_lower);

                        if should_remove {
                            changed = true;
                            return "".to_string();
                        }
                    }
                    caps.get(0)
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default()
                });

                result = new_result.into_owned();

                if !changed {
                    break;
                }
            }
        }
    }

    result
}

/// Try to extract the main content area
fn extract_main_content(html: &str) -> String {
    // Try content selectors in order
    for selector in &CLUTTER.content_selectors.selectors {
        if let Some(content) = try_extract_by_selector(html, selector) {
            // Skip if it's just the body tag or very short
            if content.len() > 100 {
                return content;
            }
        }
    }

    // Fall back to body content
    if let Some(body) = extract_body(html) {
        return body;
    }

    html.to_string()
}

/// Try to extract content matching a CSS-like selector
fn try_extract_by_selector(html: &str, selector: &str) -> Option<String> {
    // Handle simple selectors: tag, .class, #id, [role="..."]
    // Note: regex_lite doesn't support backreferences, so we handle tags individually

    if let Some(id) = selector.strip_prefix('#') {
        // ID selector - try common tags
        for tag in ["div", "article", "section", "main", "aside"] {
            let pattern = format!(
                r#"(?is)<{tag}[^>]*id=["']{id}["'][^>]*>(.*?)</{tag}>"#,
                tag = tag,
                id = regex_lite::escape(id)
            );
            if let Ok(re) = Regex::new(&pattern) {
                if let Some(caps) = re.captures(html) {
                    if let Some(m) = caps.get(1) {
                        return Some(m.as_str().to_string());
                    }
                }
            }
        }
        None
    } else if let Some(class) = selector.strip_prefix('.') {
        // Class selector - try common tags
        for tag in ["div", "article", "section", "main", "aside"] {
            let pattern = format!(
                r#"(?is)<{tag}[^>]*class=["'][^"']*\b{class}\b[^"']*["'][^>]*>(.*?)</{tag}>"#,
                tag = tag,
                class = regex_lite::escape(class)
            );
            if let Ok(re) = Regex::new(&pattern) {
                if let Some(caps) = re.captures(html) {
                    if let Some(m) = caps.get(1) {
                        return Some(m.as_str().to_string());
                    }
                }
            }
        }
        None
    } else if selector.starts_with('[') && selector.contains("role=") {
        // Role attribute selector
        if let Some(role) = selector
            .strip_prefix("[role=\"")
            .and_then(|s| s.strip_suffix("\"]"))
        {
            for tag in ["div", "article", "section", "main", "aside"] {
                let pattern = format!(
                    r#"(?is)<{tag}[^>]*role=["']{role}["'][^>]*>(.*?)</{tag}>"#,
                    tag = tag,
                    role = regex_lite::escape(role)
                );
                if let Ok(re) = Regex::new(&pattern) {
                    if let Some(caps) = re.captures(html) {
                        if let Some(m) = caps.get(1) {
                            return Some(m.as_str().to_string());
                        }
                    }
                }
            }
        }
        None
    } else if selector.chars().all(|c| c.is_alphanumeric()) {
        // Tag selector
        let pattern = format!(r#"(?is)<{0}[^>]*>(.*?)</{0}>"#, selector);
        if let Ok(re) = Regex::new(&pattern) {
            if let Some(caps) = re.captures(html) {
                if let Some(m) = caps.get(1) {
                    return Some(m.as_str().to_string());
                }
            }
        }
        None
    } else {
        None
    }
}

/// Extract body content
fn extract_body(html: &str) -> Option<String> {
    static BODY_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?is)<body[^>]*>(.*)</body>").unwrap());

    BODY_RE
        .captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// Clean up whitespace
fn clean_whitespace(html: &str) -> String {
    static MULTI_SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]+").unwrap());
    static MULTI_NEWLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{2,}").unwrap());

    let result = MULTI_SPACE.replace_all(html, " ");
    let result = MULTI_NEWLINE.replace_all(&result, "\n");
    result.trim().to_string()
}

/// Check if a line is entirely junk text that should be filtered.
/// Only filters if the line contains ONLY junk patterns (possibly with whitespace/punctuation).
fn is_junk_line(line: &str) -> bool {
    // Remove all junk patterns from the line
    let mut remaining = line.to_string();
    for pattern in &CLUTTER.junk_text.exact {
        // Case-insensitive replacement
        let pattern_lower = pattern.to_lowercase();
        let mut result = String::new();
        let mut remaining_lower = remaining.to_lowercase();
        while let Some(pos) = remaining_lower.find(&pattern_lower) {
            result.push_str(&remaining[..pos]);
            remaining = remaining[pos + pattern.len()..].to_string();
            remaining_lower = remaining.to_lowercase();
        }
        result.push_str(&remaining);
        remaining = result;
    }

    // If only whitespace and common separators remain, it's a junk line
    remaining
        .trim()
        .chars()
        .all(|c| c.is_whitespace() || "/-•·|:".contains(c))
}

/// Convert HTML to plain text
fn html_to_text(html: &str) -> String {
    static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
    static MULTI_SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]+").unwrap());
    static MULTI_NEWLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{2,}").unwrap());

    // Add newlines before/after block elements
    let mut result = html.to_string();
    for tag in [
        "p", "div", "br", "h1", "h2", "h3", "h4", "h5", "h6", "li", "tr",
    ] {
        let open_pattern = format!("<{}", tag);
        result = result.replace(&open_pattern, &format!("\n<{}", tag));
    }

    // Remove all HTML tags
    let result = TAG_RE.replace_all(&result, "");

    // Decode HTML entities
    let result = decode_html_entities(&result);

    // Clean up whitespace
    let result = MULTI_SPACE.replace_all(&result, " ");
    let result = MULTI_NEWLINE.replace_all(&result, "\n");

    // Trim lines, filter empty ones and junk text
    result
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !is_junk_line(l))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Convert HTML to Markdown (basic conversion)
fn html_to_markdown(html: &str) -> String {
    let mut result = html.to_string();

    // Headers
    for i in 1..=6 {
        let hashes = "#".repeat(i);
        let open = format!("<h{}", i);
        let close = format!("</h{}>", i);
        result = Regex::new(&format!(r"(?i){}\s*[^>]*>", regex_lite::escape(&open)))
            .unwrap()
            .replace_all(&result, &format!("\n{} ", hashes))
            .to_string();
        result = result.replace(&close, "\n");
    }

    // Bold and italic
    result = Regex::new(r"(?i)<strong[^>]*>([^<]*)</strong>")
        .unwrap()
        .replace_all(&result, "**$1**")
        .to_string();
    result = Regex::new(r"(?i)<b[^>]*>([^<]*)</b>")
        .unwrap()
        .replace_all(&result, "**$1**")
        .to_string();
    result = Regex::new(r"(?i)<em[^>]*>([^<]*)</em>")
        .unwrap()
        .replace_all(&result, "*$1*")
        .to_string();
    result = Regex::new(r"(?i)<i[^>]*>([^<]*)</i>")
        .unwrap()
        .replace_all(&result, "*$1*")
        .to_string();

    // Links
    result = Regex::new(r#"(?i)<a[^>]*href=["']([^"']+)["'][^>]*>([^<]*)</a>"#)
        .unwrap()
        .replace_all(&result, "[$2]($1)")
        .to_string();

    // Images
    result = Regex::new(r#"(?i)<img[^>]*src=["']([^"']+)["'][^>]*alt=["']([^"']*)["'][^>]*/?>"#)
        .unwrap()
        .replace_all(&result, "![$2]($1)")
        .to_string();
    result = Regex::new(r#"(?i)<img[^>]*alt=["']([^"']*)["'][^>]*src=["']([^"']+)["'][^>]*/?>"#)
        .unwrap()
        .replace_all(&result, "![$1]($2)")
        .to_string();

    // Paragraphs and line breaks
    result = Regex::new(r"(?i)<p[^>]*>")
        .unwrap()
        .replace_all(&result, "\n\n")
        .to_string();
    result = result.replace("</p>", "\n");
    result = Regex::new(r"(?i)<br\s*/?>")
        .unwrap()
        .replace_all(&result, "\n")
        .to_string();

    // Lists
    result = Regex::new(r"(?i)<li[^>]*>")
        .unwrap()
        .replace_all(&result, "\n- ")
        .to_string();
    result = result.replace("</li>", "");
    result = Regex::new(r"(?i)</?[uo]l[^>]*>")
        .unwrap()
        .replace_all(&result, "\n")
        .to_string();

    // Code
    result = Regex::new(r"(?i)<code[^>]*>([^<]*)</code>")
        .unwrap()
        .replace_all(&result, "`$1`")
        .to_string();
    result = Regex::new(r"(?i)<pre[^>]*>([^<]*)</pre>")
        .unwrap()
        .replace_all(&result, "\n```\n$1\n```\n")
        .to_string();

    // Blockquotes
    result = Regex::new(r"(?i)<blockquote[^>]*>")
        .unwrap()
        .replace_all(&result, "\n> ")
        .to_string();
    result = result.replace("</blockquote>", "\n");

    // Remove remaining tags
    result = Regex::new(r"<[^>]+>")
        .unwrap()
        .replace_all(&result, "")
        .to_string();

    // Decode entities and clean up
    result = decode_html_entities(&result);

    // Clean up whitespace
    static MULTI_SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]+").unwrap());
    static MULTI_NEWLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{2,}").unwrap());
    // Remove empty markdown headers (# followed by only whitespace/newline)
    static EMPTY_HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#{1,6}\s*$").unwrap());

    let result = MULTI_SPACE.replace_all(&result, " ");
    let result = MULTI_NEWLINE.replace_all(&result, "\n");

    // Trim lines, filter empty ones, empty headers, and junk text
    result
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !EMPTY_HEADER.is_match(l) && !is_junk_line(l))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title() {
        let html = r#"<html><head><title>Test Title</title></head></html>"#;
        let title = extract_title_tag(html);
        assert_eq!(title, Some("Test Title".to_string()));
    }

    #[test]
    fn test_extract_og_title() {
        let html = r#"<meta property="og:title" content="OG Title">"#;
        let title = extract_meta_content(html, "og:title");
        assert_eq!(title, Some("OG Title".to_string()));
    }

    #[test]
    fn test_html_to_text() {
        let html = "<p>Hello <strong>World</strong>!</p>";
        let text = html_to_text(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn test_remove_script_tags() {
        let html = "<div>Before<script>alert('x');</script>After</div>";
        let cleaned = remove_tags(html, &["script"]);
        assert!(!cleaned.contains("alert"));
        assert!(cleaned.contains("Before"));
        assert!(cleaned.contains("After"));
    }

    #[test]
    fn test_decode_entities() {
        assert_eq!(decode_html_entities("&amp;"), "&");
        assert_eq!(decode_html_entities("&lt;"), "<");
        assert_eq!(decode_html_entities("Hello&nbsp;World"), "Hello World");
    }

    #[test]
    fn test_clutter_patterns_loaded() {
        // Verify the clutter patterns are loaded correctly
        assert!(!CLUTTER.content_selectors.selectors.is_empty());
        assert!(!CLUTTER.remove.exact_selectors.is_empty());
        assert!(!CLUTTER.scoring.non_content_patterns.is_empty());
    }

    #[test]
    fn test_junk_line_detection() {
        // Pure junk lines should be filtered
        assert!(is_junk_line("NaN"));
        assert!(is_junk_line("NaN / NaN"));
        assert!(is_junk_line("undefined"));
        assert!(is_junk_line("[object Object]"));

        // Junk with only separators should be filtered
        assert!(is_junk_line("NaN / NaN / NaN"));
        assert!(is_junk_line("  NaN  "));

        // Real content should NOT be filtered (no false positives)
        assert!(!is_junk_line("The value is NaN due to division"));
        assert!(!is_junk_line("undefined behavior in C++"));
        assert!(!is_junk_line("null pointer exception"));
        assert!(!is_junk_line("Hello World"));
        assert!(!is_junk_line("10:30 AM"));
    }
}
