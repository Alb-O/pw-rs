use crate::context::CommandContext;
use crate::error::Result;
use crate::session_broker::{SessionBroker, SessionRequest};
use pw::WaitUntil;
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
struct Element {
    kind: String,
    label: String,
    selector: String,
    extra: Option<String>,
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
        
        elements.push({
            kind: kind,
            label: label.substring(0, 60),
            selector: selector,
            extra: extra || null
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
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
) -> Result<()> {
    info!(target = "pw", %url, browser = %ctx.browser, "list elements");

    let session = broker
        .session(SessionRequest::from_context(WaitUntil::NetworkIdle, ctx))
        .await?;
    session.goto(url).await?;

    let js = format!("JSON.stringify({})", EXTRACT_ELEMENTS_JS);
    let result = session.page().evaluate_value(&js).await?;
    let elements: Vec<Element> = serde_json::from_str(&result)?;

    if elements.is_empty() {
        println!("No interactive elements found");
        return session.close().await;
    }

    // Calculate column widths
    let max_kind = elements.iter().map(|e| e.kind.len()).max().unwrap_or(6);
    let max_label = elements
        .iter()
        .map(|e| e.label.len())
        .max()
        .unwrap_or(20)
        .min(40);

    for el in &elements {
        let kind = format!("[{}]", el.kind);
        let label = if el.label.len() > 40 {
            format!("{}...", &el.label[..37])
        } else {
            el.label.clone()
        };

        let extra = el
            .extra
            .as_ref()
            .map(|e| format!(" ({})", e))
            .unwrap_or_default();

        println!(
            "{:<width_k$} {:<width_l$}{} → {}",
            kind,
            label,
            extra,
            el.selector,
            width_k = max_kind + 2,
            width_l = max_label,
        );
    }

    session.close().await
}
