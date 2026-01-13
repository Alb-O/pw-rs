//! Page snapshot command for AI agent workflows.
//!
//! Extracts a comprehensive "page model" containing metadata, interactive elements,
//! and visible text content. This reduces agent tool-chaining by providing full
//! page context in a single call.
//!
//! # Main Types
//!
//! - [`SnapshotRaw`] - Unresolved inputs from CLI or batch JSON
//! - [`SnapshotResolved`] - Validated inputs ready for execution
//! - [`SnapshotData`](crate::output::SnapshotData) - Command output structure
//!
//! # Output Contents
//!
//! - Page metadata (URL, title, viewport dimensions)
//! - Interactive elements (buttons, links, inputs) with stable CSS selectors
//! - Visible text content (configurable length limit)
//!
//! # Example
//!
//! ```bash
//! pw snapshot https://example.com
//! pw snapshot --text-only   # Skip interactive elements (faster)
//! pw snapshot --full        # Include all text, not just visible
//! pw snapshot --max-text-length 10000
//! ```

use std::path::Path;

use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::output::{
    CommandInputs, FailureWithArtifacts, InteractiveElement, OutputFormat, ResultBuilder,
    SnapshotData, print_failure_with_artifacts, print_result,
};
use crate::session_broker::{SessionBroker, SessionHandle, SessionRequest};
use crate::target::{Resolve, ResolveEnv, ResolvedTarget, TargetPolicy};
use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Raw inputs from CLI or batch JSON before resolution.
///
/// Accepts both camelCase (JSON) and snake_case (CLI) field names via serde aliases.
/// Use [`Resolve::resolve`] to convert to [`SnapshotResolved`] for execution.
///
/// # Example
///
/// ```
/// # use pw_cli::commands::snapshot::SnapshotRaw;
/// let raw = SnapshotRaw {
///     url: Some("https://example.com".into()),
///     text_only: Some(true),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRaw {
    /// Target URL, resolved from context if not provided.
    #[serde(default)]
    pub url: Option<String>,

    /// When `true`, skips interactive element extraction for faster text-only snapshots.
    #[serde(default)]
    pub text_only: Option<bool>,

    /// When `true`, includes all page text rather than just viewport-visible content.
    #[serde(default)]
    pub full: Option<bool>,

    /// Maximum characters of text to extract. Defaults to 5000.
    #[serde(default, alias = "max_text_length")]
    pub max_text_length: Option<usize>,
}

impl SnapshotRaw {
    /// Constructs [`SnapshotRaw`] from CLI arguments.
    ///
    /// The `url_flag` takes precedence over `url` when both are provided,
    /// matching the CLI convention where `--url` overrides positional args.
    pub fn from_cli(
        url: Option<String>,
        url_flag: Option<String>,
        text_only: bool,
        full: bool,
        max_text_length: Option<usize>,
    ) -> Self {
        Self {
            url: url_flag.or(url),
            text_only: Some(text_only),
            full: Some(full),
            max_text_length,
        }
    }
}

/// Resolved inputs ready for execution.
///
/// All optional fields have been validated and defaults applied.
/// The [`target`](Self::target) field contains either an explicit URL
/// or [`CurrentPage`](crate::target::Target::CurrentPage) for CDP mode without navigation.
#[derive(Debug, Clone)]
pub struct SnapshotResolved {
    /// Navigation target (URL to navigate to, or current page in CDP mode).
    pub target: ResolvedTarget,

    /// Skip interactive element extraction for faster execution.
    pub text_only: bool,

    /// Include full page text instead of viewport-visible only.
    pub full: bool,

    /// Maximum text length to extract in characters.
    pub max_text_length: usize,
}

impl SnapshotResolved {
    /// Returns the URL for session page-matching, preferring the target URL
    /// if navigating, or `last_url` as a hint for [`Target::CurrentPage`].
    pub fn preferred_url<'a>(&'a self, last_url: Option<&'a str>) -> Option<&'a str> {
        self.target.preferred_url(last_url)
    }
}

impl Resolve for SnapshotRaw {
    type Output = SnapshotResolved;

    fn resolve(self, env: &ResolveEnv<'_>) -> Result<SnapshotResolved> {
        let target = env.resolve_target(self.url, TargetPolicy::AllowCurrentPage)?;

        Ok(SnapshotResolved {
            target,
            text_only: self.text_only.unwrap_or(false),
            full: self.full.unwrap_or(false),
            max_text_length: self.max_text_length.unwrap_or(5000),
        })
    }
}

/// Element data returned by the browser extraction script.
#[derive(Debug, Deserialize)]
struct RawElement {
    kind: String,
    label: String,
    selector: String,
    extra: Option<String>,
    #[serde(default)]
    x: i32,
    #[serde(default)]
    y: i32,
    #[serde(default)]
    width: i32,
    #[serde(default)]
    height: i32,
}

/// Page metadata returned by the browser extraction script.
#[derive(Debug, Deserialize)]
struct PageMeta {
    url: String,
    title: String,
    viewport_width: i32,
    viewport_height: i32,
}

/// JavaScript that extracts page metadata (URL, title, viewport size).
const EXTRACT_META_JS: &str = r#"
(() => {
    return {
        url: window.location.href,
        title: document.title || '',
        viewport_width: window.innerWidth,
        viewport_height: window.innerHeight
    };
})()
"#;

/// JavaScript that extracts visible text content using TreeWalker.
///
/// Accepts `maxLength` (character limit) and `full` (include non-visible text) parameters.
/// Skips script, style, noscript, iframe, and SVG elements.
const EXTRACT_TEXT_JS: &str = r#"
((maxLength, full) => {
    const texts = [];
    let totalLength = 0;
    
    const ignoreTags = new Set(['SCRIPT', 'STYLE', 'NOSCRIPT', 'IFRAME', 'SVG', 'PATH']);
    
    function isVisible(el) {
        if (!el || el.nodeType !== Node.ELEMENT_NODE) return true;
        const style = window.getComputedStyle(el);
        if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') {
            return false;
        }
        if (!full) {
            const rect = el.getBoundingClientRect();
            if (rect.width === 0 || rect.height === 0) return false;
        }
        return true;
    }
    
    const walker = document.createTreeWalker(
        document.body || document.documentElement,
        NodeFilter.SHOW_TEXT,
        {
            acceptNode: (node) => {
                const parent = node.parentElement;
                if (!parent) return NodeFilter.FILTER_REJECT;
                if (ignoreTags.has(parent.tagName)) return NodeFilter.FILTER_REJECT;
                if (!isVisible(parent)) return NodeFilter.FILTER_REJECT;
                const text = node.textContent.trim();
                if (!text) return NodeFilter.FILTER_REJECT;
                return NodeFilter.FILTER_ACCEPT;
            }
        }
    );
    
    while (walker.nextNode() && totalLength < maxLength) {
        const text = walker.currentNode.textContent.trim();
        if (text) {
            texts.push(text);
            totalLength += text.length + 1;
        }
    }
    
    return texts.join(' ').substring(0, maxLength);
})
"#;

/// JavaScript that extracts interactive elements with stable selectors.
///
/// Generates selectors preferring: ID > name attribute > text content >
/// aria-label > class combination > nth-of-type fallback. Duplicated from
/// [`elements`](super::elements) module for bundle isolation.
const EXTRACT_ELEMENTS_JS: &str = r#"
(() => {
    const elements = [];
    const seen = new Set();
    
    function getStableSelector(el) {
        if (el.id) return '#' + CSS.escape(el.id);
        
        if (el.name && (el.tagName === 'INPUT' || el.tagName === 'SELECT' || el.tagName === 'TEXTAREA')) {
            const sel = el.tagName.toLowerCase() + '[name="' + el.name + '"]';
            if (document.querySelectorAll(sel).length === 1) return sel;
        }
        
        const text = (el.textContent || '').trim().substring(0, 50);
        if (text && (el.tagName === 'BUTTON' || el.tagName === 'A' || el.role === 'button')) {
            const shortText = text.split('\n')[0].trim();
            if (shortText.length > 0 && shortText.length < 40) {
                const sel = el.tagName.toLowerCase() + ':has-text("' + shortText.replace(/"/g, '\\"') + '")';
                return sel;
            }
        }
        
        if (el.getAttribute('aria-label')) {
            const sel = '[aria-label="' + el.getAttribute('aria-label').replace(/"/g, '\\"') + '"]';
            if (document.querySelectorAll(sel).length === 1) return sel;
        }
        
        if (el.className && typeof el.className === 'string') {
            const classes = el.className.split(/\s+/).filter(c => c && !c.match(/^(hover|active|focus|disabled)/));
            if (classes.length > 0 && classes.length <= 3) {
                const sel = el.tagName.toLowerCase() + '.' + classes.slice(0, 2).join('.');
                if (document.querySelectorAll(sel).length === 1) return sel;
            }
        }
        
        const parent = el.parentElement;
        if (parent) {
            const siblings = Array.from(parent.children).filter(c => c.tagName === el.tagName);
            const idx = siblings.indexOf(el) + 1;
            if (siblings.length > 1) {
                return el.tagName.toLowerCase() + ':nth-of-type(' + idx + ')';
            }
        }
        
        return el.tagName.toLowerCase();
    }
    
    function getLabel(el) {
         if (el.id) {
             const label = document.querySelector('label[for="' + el.id + '"]');
             if (label) return cleanText(label.textContent);
         }
         
         const ariaLabel = el.getAttribute('aria-label');
         if (ariaLabel) return ariaLabel.trim().substring(0, 40);
         
         if (el.placeholder) return el.placeholder.trim().substring(0, 40);
         
         if (el.title) return el.title.trim().substring(0, 40);
         
         if (el.value && (el.type === 'submit' || el.type === 'button')) {
             return el.value.trim().substring(0, 40);
         }
         
         return cleanText(el.textContent);
    }
    
    function cleanText(str) {
        if (!str) return '';
        return str.replace(/\s+/g, ' ').trim().substring(0, 40);
    }
    
    function isVisible(el) {
        const rect = el.getBoundingClientRect();
        if (rect.width === 0 || rect.height === 0) return false;
        const style = window.getComputedStyle(el);
        if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') return false;
        return true;
    }
    
    function addElement(el, kind, extra) {
        if (!isVisible(el)) return;
        const selector = getStableSelector(el);
        const key = kind + ':' + selector;
        if (seen.has(key)) return;
        seen.add(key);
        
        const label = getLabel(el) || '(unlabeled)';
        const rect = el.getBoundingClientRect();
        
        elements.push({
            kind: kind,
            label: label.substring(0, 60),
            selector: selector,
            extra: extra || null,
            x: Math.round(rect.x),
            y: Math.round(rect.y),
            width: Math.round(rect.width),
            height: Math.round(rect.height)
        });
    }
    
    document.querySelectorAll('button, [role="button"], input[type="submit"], input[type="button"]').forEach(el => {
        addElement(el, 'button', null);
    });
    
    document.querySelectorAll('a[href]').forEach(el => {
        const href = el.getAttribute('href');
        if (href && !href.startsWith('javascript:') && !href.startsWith('#')) {
            addElement(el, 'link', null);
        }
    });
    
    document.querySelectorAll('input:not([type="hidden"]):not([type="submit"]):not([type="button"]):not([type="checkbox"]):not([type="radio"])').forEach(el => {
        addElement(el, 'input', el.type || 'text');
    });
    
    document.querySelectorAll('textarea').forEach(el => {
        addElement(el, 'textarea', null);
    });
    
    document.querySelectorAll('select').forEach(el => {
        addElement(el, 'select', null);
    });
    
    document.querySelectorAll('input[type="checkbox"]').forEach(el => {
        addElement(el, 'checkbox', el.checked ? 'checked' : 'unchecked');
    });
    
    document.querySelectorAll('input[type="radio"]').forEach(el => {
        addElement(el, 'radio', el.checked ? 'checked' : 'unchecked');
    });
    
    return elements;
})()
"#;

/// Executes the snapshot command with resolved arguments.
///
/// Navigates to the target (or stays on current page in CDP mode), then extracts
/// page metadata, visible text content, and interactive elements via JavaScript
/// evaluation.
///
/// # Arguments
///
/// * `args` - Resolved snapshot parameters
/// * `ctx` - Command context with browser configuration
/// * `broker` - Session broker for browser connection
/// * `format` - Output format (JSON, TOON, etc.)
/// * `artifacts_dir` - Directory to save diagnostic artifacts on failure
/// * `last_url` - Last visited URL for session page matching
///
/// # Errors
///
/// - [`PwError::Navigation`](crate::error::PwError) if page navigation fails
/// - [`PwError::JsEval`](crate::error::PwError) if JavaScript evaluation fails
/// - [`PwError::Parse`](crate::error::PwError) if response JSON is malformed
///
/// On failure with `artifacts_dir` set, saves screenshot and HTML for debugging.
pub async fn execute_resolved(
    args: &SnapshotResolved,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    artifacts_dir: Option<&Path>,
    last_url: Option<&str>,
) -> Result<()> {
    let url_display = args.target.url_str().unwrap_or("<current page>");
    info!(target = "pw", url = %url_display, text_only = %args.text_only, full = %args.full, browser = %ctx.browser, "snapshot");

    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(args.preferred_url(last_url)),
        )
        .await?;

    match extract_snapshot(&session, args, format).await {
        Ok(()) => session.close().await,
        Err(e) => {
            let artifacts = session
                .collect_failure_artifacts(artifacts_dir, "snapshot")
                .await;

            if !artifacts.is_empty() {
                let failure = FailureWithArtifacts::new(e.to_command_error())
                    .with_artifacts(artifacts.artifacts);
                print_failure_with_artifacts("snapshot", &failure, format);
                let _ = session.close().await;
                return Err(PwError::OutputAlreadyPrinted);
            }

            let _ = session.close().await;
            Err(e)
        }
    }
}

/// Performs the actual snapshot extraction after session acquisition.
async fn extract_snapshot(
    session: &SessionHandle,
    args: &SnapshotResolved,
    format: OutputFormat,
) -> Result<()> {
    session.goto_target(&args.target.target).await?;

    let meta_js = format!("JSON.stringify({})", EXTRACT_META_JS);
    let meta: PageMeta = serde_json::from_str(&session.page().evaluate_value(&meta_js).await?)?;

    let text_js = format!(
        "JSON.stringify({}({}, {}))",
        EXTRACT_TEXT_JS, args.max_text_length, args.full
    );
    let text: String = serde_json::from_str(&session.page().evaluate_value(&text_js).await?)?;

    let elements = extract_elements_if_needed(session, args.text_only).await?;
    let element_count = elements.len();

    let result = ResultBuilder::new("snapshot")
        .inputs(CommandInputs {
            url: args.target.url_str().map(String::from),
            ..Default::default()
        })
        .data(SnapshotData {
            url: meta.url,
            title: meta.title,
            viewport_width: meta.viewport_width,
            viewport_height: meta.viewport_height,
            text,
            elements,
            element_count,
        })
        .build();

    print_result(&result, format);
    Ok(())
}

/// Extracts interactive elements unless `text_only` mode is enabled.
async fn extract_elements_if_needed(
    session: &SessionHandle,
    text_only: bool,
) -> Result<Vec<InteractiveElement>> {
    if text_only {
        return Ok(Vec::new());
    }

    let elements_js = format!("JSON.stringify({})", EXTRACT_ELEMENTS_JS);
    let raw_elements: Vec<RawElement> =
        serde_json::from_str(&session.page().evaluate_value(&elements_js).await?)?;

    Ok(raw_elements.into_iter().map(Into::into).collect())
}

impl From<RawElement> for InteractiveElement {
    fn from(e: RawElement) -> Self {
        Self {
            tag: e.kind,
            selector: e.selector,
            text: if e.label.is_empty() || e.label == "(unlabeled)" {
                None
            } else {
                Some(e.label)
            },
            href: None,
            name: e.extra,
            id: None,
            x: e.x,
            y: e.y,
            width: e.width,
            height: e.height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_camel_case() {
        let json = r#"{"url": "https://example.com", "textOnly": true, "maxTextLength": 1000}"#;
        let raw: SnapshotRaw = serde_json::from_str(json).unwrap();

        assert_eq!(raw.url, Some("https://example.com".into()));
        assert_eq!(raw.text_only, Some(true));
        assert_eq!(raw.max_text_length, Some(1000));
    }

    #[test]
    fn deserialize_snake_case_alias() {
        let json = r#"{"max_text_length": 2000}"#;
        let raw: SnapshotRaw = serde_json::from_str(json).unwrap();

        assert_eq!(raw.max_text_length, Some(2000));
    }

    #[test]
    fn deserialize_empty_uses_defaults() {
        let raw: SnapshotRaw = serde_json::from_str("{}").unwrap();

        assert_eq!(raw.url, None);
        assert_eq!(raw.text_only, None);
        assert_eq!(raw.full, None);
        assert_eq!(raw.max_text_length, None);
    }
}
