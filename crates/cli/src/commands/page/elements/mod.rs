//! Interactive elements discovery command.
//!
//! Extracts all interactive elements (buttons, links, inputs, etc.) from a page
//! with stable CSS selectors suitable for automation. Each element includes its
//! bounding box coordinates for visual reference.
//!
//! # Extracted Element Types
//!
//! * Buttons (including `role="button"`)
//! * Links (`<a href="...">`)
//! * Text inputs, textareas, selects
//! * Checkboxes and radio buttons
//!
//! # Example
//!
//! ```bash
//! pw elements https://example.com --wait --timeout-ms 5000
//! ```

use clap::Args;
use pw_rs::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::commands::contract::{resolve_target_from_url_pair, standard_delta, standard_inputs};
use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ExecCtx};
use crate::commands::exec_flow::navigation_plan;
use crate::error::Result;
use crate::output::{ElementsData, InteractiveElement};
use crate::session_broker::SessionHandle;
use crate::session_helpers::{ArtifactsPolicy, with_session};
use crate::target::{ResolveEnv, ResolvedTarget, TargetPolicy};

/// Raw inputs from CLI or batch JSON before resolution.
#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementsRaw {
	/// Target URL (positional)
	#[serde(default)]
	pub url: Option<String>,

	/// Wait for elements with polling (useful for dynamic pages)
	#[arg(long)]
	#[serde(default)]
	pub wait: Option<bool>,

	/// Timeout in milliseconds for --wait mode (default: 10000)
	#[arg(long, default_value = "10000")]
	#[serde(default, alias = "timeout_ms")]
	pub timeout_ms: Option<u64>,

	/// Target URL (named alternative)
	#[arg(long = "url", short = 'u', value_name = "URL")]
	#[serde(default, alias = "url_flag")]
	pub url_flag: Option<String>,
}

/// Resolved inputs ready for execution.
///
/// All arguments have been validated with defaults applied.
#[derive(Debug, Clone)]
pub struct ElementsResolved {
	/// Navigation target (URL or current page).
	pub target: ResolvedTarget,

	/// Whether to poll until elements appear.
	pub wait: bool,

	/// Timeout in milliseconds when waiting.
	pub timeout_ms: u64,
}

pub struct ElementsCommand;

impl CommandDef for ElementsCommand {
	const NAME: &'static str = "page.elements";

	type Raw = ElementsRaw;
	type Resolved = ElementsResolved;
	type Data = ElementsData;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		let target = resolve_target_from_url_pair(raw.url, raw.url_flag, env, TargetPolicy::AllowCurrentPage)?;

		Ok(ElementsResolved {
			target,
			wait: raw.wait.unwrap_or(false),
			timeout_ms: raw.timeout_ms.unwrap_or(10000),
		})
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, mut exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let url_display = args.target.url_str().unwrap_or("<current page>");
			info!(target = "pw", url = %url_display, wait = %args.wait, timeout_ms = %args.timeout_ms, browser = %exec.ctx.browser, "list elements");

			let plan = navigation_plan(exec.ctx, exec.last_url, &args.target, WaitUntil::NetworkIdle);
			let timeout_ms = plan.timeout_ms;
			let target = plan.target;
			let wait = args.wait;
			let poll_timeout_ms = args.timeout_ms;

			let data = with_session(&mut exec, plan.request, ArtifactsPolicy::OnError { command: "elements" }, move |session| {
				Box::pin(async move {
					session.goto_target(&target, timeout_ms).await?;

					let js = format!("JSON.stringify({})", EXTRACT_ELEMENTS_JS);

					let raw_elements: Vec<RawElement> = if wait {
						poll_for_elements(session, &js, poll_timeout_ms).await?
					} else {
						let raw_result = session.page().evaluate_value(&js).await?;
						serde_json::from_str(&raw_result)?
					};

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
							href: None,
							name: e.extra,
							id: None,
							x: e.x,
							y: e.y,
							width: e.width,
							height: e.height,
						})
						.collect();

					let count = elements.len();

					Ok(ElementsData { elements, count })
				})
			})
			.await?;

			let inputs = standard_inputs(&args.target, None, None, None, None);

			Ok(CommandOutcome {
				inputs,
				data,
				delta: standard_delta(&args.target, None, None),
			})
		})
	}
}

/// Element data as returned by the extraction JavaScript.
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

/// JavaScript that extracts interactive elements from the page.
///
/// Generates stable selectors preferring: ID > name attribute > text content >
/// aria-label > class combination > nth-of-type fallback.
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

/// Polls for elements until some appear or timeout is reached.
async fn poll_for_elements(session: &SessionHandle, js: &str, timeout_ms: u64) -> Result<Vec<RawElement>> {
	let start = std::time::Instant::now();
	let poll_interval = std::time::Duration::from_millis(500);
	let timeout = std::time::Duration::from_millis(timeout_ms);

	loop {
		let raw_result = session.page().evaluate_value(js).await?;
		let elements: Vec<RawElement> = serde_json::from_str(&raw_result)?;

		if !elements.is_empty() {
			return Ok(elements);
		}

		if start.elapsed() >= timeout {
			return Ok(vec![]);
		}

		tokio::time::sleep(poll_interval).await;
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn elements_raw_deserialize_from_json() {
		let json = r#"{"url": "https://example.com", "wait": true, "timeout_ms": 5000}"#;
		let raw: ElementsRaw = serde_json::from_str(json).unwrap();
		assert_eq!(raw.url, Some("https://example.com".into()));
		assert_eq!(raw.wait, Some(true));
		assert_eq!(raw.timeout_ms, Some(5000));
	}

	#[test]
	fn elements_raw_defaults() {
		let json = r#"{}"#;
		let raw: ElementsRaw = serde_json::from_str(json).unwrap();
		assert_eq!(raw.wait, None);
		assert_eq!(raw.timeout_ms, None);
	}
}
