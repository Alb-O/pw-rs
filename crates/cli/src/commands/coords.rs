//! Element coordinates extraction command.
//!
//! Returns the bounding box coordinates (x, y, width, height) and center point
//! of elements matching a CSS selector. Useful for visual automation and
//! click coordinate calculation.
//!
//! # Commands
//!
//! - `coords`: Get coordinates of the first matching element
//! - `coords-all`: Get coordinates of all matching elements with indices
//!
//! # Example
//!
//! ```bash
//! pw coords --selector "button.submit"
//! pw coords-all --selector "li.item"
//! ```

use crate::browser::js;
use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::output::{CommandInputs, ErrorCode, OutputFormat, ResultBuilder, print_result};
use crate::session_broker::{SessionBroker, SessionRequest};
use crate::target::{Resolve, ResolveEnv, ResolvedTarget, TargetPolicy};
use crate::types::{ElementCoords, IndexedElementCoords};
use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Raw inputs from CLI or batch JSON before resolution.
///
/// Use [`Resolve::resolve`] to convert to [`CoordsResolved`] for execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoordsRaw {
    /// Target URL, resolved from context if not provided.
    #[serde(default)]
    pub url: Option<String>,

    /// CSS selector for the target element(s).
    #[serde(default)]
    pub selector: Option<String>,
}

impl CoordsRaw {
    /// Creates a [`CoordsRaw`] from CLI arguments.
    pub fn from_cli(url: Option<String>, selector: Option<String>) -> Self {
        Self { url, selector }
    }
}

/// Resolved inputs ready for execution.
///
/// The [`selector`](Self::selector) has been validated as present.
#[derive(Debug, Clone)]
pub struct CoordsResolved {
    /// Navigation target (URL or current page).
    pub target: ResolvedTarget,

    /// CSS selector for the target element(s).
    pub selector: String,
}

impl CoordsResolved {
    /// Returns the URL for page preference matching.
    ///
    /// For [`Navigate`](crate::target::Target::Navigate) targets, returns the URL.
    /// For [`CurrentPage`](crate::target::Target::CurrentPage), returns `last_url` as a hint.
    pub fn preferred_url<'a>(&'a self, last_url: Option<&'a str>) -> Option<&'a str> {
        self.target.preferred_url(last_url)
    }
}

impl Resolve for CoordsRaw {
    type Output = CoordsResolved;

    fn resolve(self, env: &ResolveEnv<'_>) -> Result<CoordsResolved> {
        let target = env.resolve_target(self.url, TargetPolicy::AllowCurrentPage)?;
        let selector = env.resolve_selector(self.selector, None)?;

        Ok(CoordsResolved { target, selector })
    }
}

/// Alias for [`CoordsRaw`] used by the `coords-all` command.
pub type CoordsAllRaw = CoordsRaw;

/// Alias for [`CoordsResolved`] used by the `coords-all` command.
pub type CoordsAllResolved = CoordsResolved;

/// Output for single element coordinates.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CoordsData {
    coords: ElementCoords,
    selector: String,
}

/// Output for multiple element coordinates.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CoordsAllData {
    coords: Vec<IndexedElementCoords>,
    selector: String,
    count: usize,
}

/// Executes the coords command for a single element.
///
/// Returns the bounding box and center point of the first element matching
/// the selector.
///
/// # Errors
///
/// Returns an error if:
/// - Navigation fails
/// - The selector matches no visible elements ([`PwError::ElementNotFound`])
/// - JavaScript evaluation fails
pub async fn execute_single_resolved(
    args: &CoordsResolved,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    last_url: Option<&str>,
) -> Result<()> {
    let url_display = args.target.url_str().unwrap_or("<current page>");
    info!(target = "pw", url = %url_display, selector = %args.selector, browser = %ctx.browser, "coords single");

    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(args.preferred_url(last_url)),
        )
        .await?;
    session
        .goto_target(&args.target.target, ctx.timeout_ms())
        .await?;

    let result_json = session
        .page()
        .evaluate_value(&js::get_element_coords_js(&args.selector))
        .await?;

    if result_json == "null" {
        let result = ResultBuilder::<CoordsData>::new("coords")
            .inputs(CommandInputs {
                url: args.target.url_str().map(String::from),
                selector: Some(args.selector.clone()),
                ..Default::default()
            })
            .error(
                ErrorCode::SelectorNotFound,
                format!("Element not found or not visible: {}", args.selector),
            )
            .build();

        print_result(&result, format);
        session.close().await?;
        return Err(PwError::ElementNotFound {
            selector: args.selector.clone(),
        });
    }

    let coords: ElementCoords = serde_json::from_str(&result_json)?;

    let result = ResultBuilder::new("coords")
        .inputs(CommandInputs {
            url: args.target.url_str().map(String::from),
            selector: Some(args.selector.clone()),
            ..Default::default()
        })
        .data(CoordsData {
            coords,
            selector: args.selector.clone(),
        })
        .build();

    print_result(&result, format);
    session.close().await
}

/// Executes the coords-all command for multiple elements.
///
/// Returns the bounding box and center point of all elements matching
/// the selector, each with an index for identification.
///
/// # Errors
///
/// Returns an error if:
/// - Navigation fails
/// - JavaScript evaluation fails
pub async fn execute_all_resolved(
    args: &CoordsAllResolved,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    last_url: Option<&str>,
) -> Result<()> {
    let url_display = args.target.url_str().unwrap_or("<current page>");
    info!(target = "pw", url = %url_display, selector = %args.selector, browser = %ctx.browser, "coords all");

    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(args.preferred_url(last_url)),
        )
        .await?;
    session
        .goto_target(&args.target.target, ctx.timeout_ms())
        .await?;

    let results_json = session
        .page()
        .evaluate_value(&js::get_all_element_coords_js(&args.selector))
        .await?;

    let coords: Vec<IndexedElementCoords> = serde_json::from_str(&results_json)?;
    let count = coords.len();

    let result = ResultBuilder::new("coords-all")
        .inputs(CommandInputs {
            url: args.target.url_str().map(String::from),
            selector: Some(args.selector.clone()),
            ..Default::default()
        })
        .data(CoordsAllData {
            coords,
            selector: args.selector.clone(),
            count,
        })
        .build();

    print_result(&result, format);
    session.close().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coords_raw_deserialize_from_json() {
        let json = r#"{"url": "https://example.com", "selector": "button"}"#;
        let raw: CoordsRaw = serde_json::from_str(json).unwrap();
        assert_eq!(raw.url, Some("https://example.com".into()));
        assert_eq!(raw.selector, Some("button".into()));
    }
}
