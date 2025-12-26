use std::path::Path;

use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::output::{
    CommandInputs, ElementsData, FailureWithArtifacts, InteractiveElement, OutputFormat,
    ResultBuilder, print_failure_with_artifacts, print_result,
};
use crate::session_broker::{SessionBroker, SessionHandle, SessionRequest};
use pw::WaitUntil;
use serde::Deserialize;
use tracing::info;

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

const EXTRACT_ELEMENTS_JS: &str = r#"
(() => {
    const elements = [];
    const seen = new Set();
    
    function getStableSelector(el) {
        // Prefer id
        if (el.id) return '#' + CSS.escape(el.id);
        
        // For inputs, prefer name
        if (el.name && (el.tagName === 'INPUT' || el.tagName === 'SELECT' || el.tagName === 'TEXTAREA')) {
            const sel = el.tagName.toLowerCase() + '[name="' + el.name + '"]';
            if (document.querySelectorAll(sel).length === 1) return sel;
        }
        
        // For buttons/links with unique text, use text selector
        const text = (el.textContent || '').trim().substring(0, 50);
        if (text && (el.tagName === 'BUTTON' || el.tagName === 'A' || el.role === 'button')) {
            const shortText = text.split('\n')[0].trim();
            if (shortText.length > 0 && shortText.length < 40) {
                const sel = el.tagName.toLowerCase() + ':has-text("' + shortText.replace(/"/g, '\\"') + '")';
                return sel;
            }
        }
        
        // Try aria-label
        if (el.getAttribute('aria-label')) {
            const sel = '[aria-label="' + el.getAttribute('aria-label').replace(/"/g, '\\"') + '"]';
            if (document.querySelectorAll(sel).length === 1) return sel;
        }
        
        // Try class combination
        if (el.className && typeof el.className === 'string') {
            const classes = el.className.split(/\s+/).filter(c => c && !c.match(/^(hover|active|focus|disabled)/));
            if (classes.length > 0 && classes.length <= 3) {
                const sel = el.tagName.toLowerCase() + '.' + classes.slice(0, 2).join('.');
                if (document.querySelectorAll(sel).length === 1) return sel;
            }
        }
        
        // Fallback: tag with nth-of-type
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
        // Check for associated label
        if (el.id) {
            const label = document.querySelector('label[for="' + el.id + '"]');
            if (label) return cleanText(label.textContent);
        }
        
        // Check aria-label
        const ariaLabel = el.getAttribute('aria-label');
        if (ariaLabel) return ariaLabel.trim().substring(0, 40);
        
        // Check placeholder
        if (el.placeholder) return el.placeholder.trim().substring(0, 40);
        
        // Check title
        if (el.title) return el.title.trim().substring(0, 40);
        
        // Use value for submit buttons
        if (el.value && (el.type === 'submit' || el.type === 'button')) {
            return el.value.trim().substring(0, 40);
        }
        
        // Use text content
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
    
    // Buttons
    document.querySelectorAll('button, [role="button"], input[type="submit"], input[type="button"]').forEach(el => {
        addElement(el, 'button', null);
    });
    
    // Links
    document.querySelectorAll('a[href]').forEach(el => {
        const href = el.getAttribute('href');
        if (href && !href.startsWith('javascript:') && !href.startsWith('#')) {
            addElement(el, 'link', null);
        }
    });
    
    // Text inputs
    document.querySelectorAll('input:not([type="hidden"]):not([type="submit"]):not([type="button"]):not([type="checkbox"]):not([type="radio"])').forEach(el => {
        addElement(el, 'input', el.type || 'text');
    });
    
    // Textareas
    document.querySelectorAll('textarea').forEach(el => {
        addElement(el, 'textarea', null);
    });
    
    // Selects
    document.querySelectorAll('select').forEach(el => {
        addElement(el, 'select', null);
    });
    
    // Checkboxes
    document.querySelectorAll('input[type="checkbox"]').forEach(el => {
        addElement(el, 'checkbox', el.checked ? 'checked' : 'unchecked');
    });
    
    // Radio buttons
    document.querySelectorAll('input[type="radio"]').forEach(el => {
        addElement(el, 'radio', el.checked ? 'checked' : 'unchecked');
    });
    
    return elements;
})()
"#;

pub async fn execute(
    url: &str,
    wait: bool,
    timeout_ms: u64,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    artifacts_dir: Option<&Path>,
) -> Result<()> {
    info!(target = "pw", %url, %wait, %timeout_ms, browser = %ctx.browser, "list elements");

    let session = broker
        .session(SessionRequest::from_context(WaitUntil::NetworkIdle, ctx))
        .await?;

    match execute_inner(&session, url, wait, timeout_ms, format).await {
        Ok(()) => session.close().await,
        Err(e) => {
            let artifacts = session
                .collect_failure_artifacts(artifacts_dir, "elements")
                .await;

            if !artifacts.is_empty() {
                // Print failure with artifacts and signal that output is complete
                let failure = FailureWithArtifacts::new(e.to_command_error())
                    .with_artifacts(artifacts.artifacts);
                print_failure_with_artifacts("elements", &failure, format);
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
    wait: bool,
    timeout_ms: u64,
    format: OutputFormat,
) -> Result<()> {
    session.goto(url).await?;

    let js = format!("JSON.stringify({})", EXTRACT_ELEMENTS_JS);

    // If wait mode, poll until we find elements or timeout
    let raw_elements: Vec<RawElement> = if wait {
        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_millis(500);
        let timeout = std::time::Duration::from_millis(timeout_ms);

        loop {
            let raw_result = session.page().evaluate_value(&js).await?;
            let elements: Vec<RawElement> = serde_json::from_str(&raw_result)?;

            if !elements.is_empty() {
                break elements;
            }

            if start.elapsed() >= timeout {
                // Timeout reached, return empty (not an error)
                break vec![];
            }

            tokio::time::sleep(poll_interval).await;
        }
    } else {
        let raw_result = session.page().evaluate_value(&js).await?;
        serde_json::from_str(&raw_result)?
    };

    // Convert to output format
    let elements: Vec<InteractiveElement> = raw_elements
        .into_iter()
        .map(|e| InteractiveElement {
            tag: e.kind,
            selector: e.selector,
            text: if e.label.is_empty() || e.label == "(unlabeled)" {
                None
            } else {
                Some(e.label)
            },
            href: None, // TODO: extract for links
            name: e.extra,
            id: None,
            x: e.x,
            y: e.y,
            width: e.width,
            height: e.height,
        })
        .collect();

    let count = elements.len();

    let result = ResultBuilder::new("elements")
        .inputs(CommandInputs {
            url: Some(url.to_string()),
            ..Default::default()
        })
        .data(ElementsData { elements, count })
        .build();

    print_result(&result, format);
    Ok(())
}
