//! Element coordinates extraction command.
//!
//! Returns the bounding box coordinates (x, y, width, height) and center point
//! of elements matching a CSS selector. Useful for visual automation and
//! click coordinate calculation.
//!
//! # Commands
//!
//! * `coords`: Get coordinates of the first matching element
//! * `coords-all`: Get coordinates of all matching elements with indices
//!
//! # Example
//!
//! ```bash
//! pw coords --selector "button.submit"
//! pw coords-all --selector "li.item"
//! ```

use clap::Args;
use pw_rs::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::browser::js;
use crate::commands::contract::{resolve_target_and_explicit_selector, standard_delta, standard_inputs};
use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ExecCtx};
use crate::commands::exec_flow::navigation_plan;
use crate::error::{PwError, Result};
use crate::session_helpers::{ArtifactsPolicy, with_session};
use crate::target::{ResolveEnv, ResolvedTarget};
use crate::types::{ElementCoords, IndexedElementCoords};

/// Raw inputs from CLI or batch JSON before resolution.
#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoordsRaw {
	/// Target URL (positional)
	#[serde(default)]
	pub url: Option<String>,

	/// CSS selector (positional)
	#[serde(default)]
	pub selector: Option<String>,

	/// Target URL (named alternative)
	#[arg(long = "url", short = 'u', value_name = "URL")]
	#[serde(default, alias = "url_flag")]
	pub url_flag: Option<String>,

	/// CSS selector (named alternative)
	#[arg(long = "selector", short = 's', value_name = "SELECTOR")]
	#[serde(default, alias = "selector_flag")]
	pub selector_flag: Option<String>,
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

/// Alias for [`CoordsRaw`] used by the `coords-all` command.
pub type CoordsAllRaw = CoordsRaw;

/// Alias for [`CoordsResolved`] used by the `coords-all` command.
pub type CoordsAllResolved = CoordsResolved;

/// Output for single element coordinates.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoordsData {
	pub coords: ElementCoords,
	pub selector: String,
}

/// Output for multiple element coordinates.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoordsAllData {
	pub coords: Vec<IndexedElementCoords>,
	pub selector: String,
	pub count: usize,
}

pub struct CoordsCommand;

impl CommandDef for CoordsCommand {
	const NAME: &'static str = "page.coords";

	type Raw = CoordsRaw;
	type Resolved = CoordsResolved;
	type Data = CoordsData;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		let (target, selector) = resolve_target_and_explicit_selector(raw.url, raw.url_flag, raw.selector, raw.selector_flag, env, None)?;

		Ok(CoordsResolved { target, selector })
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, mut exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let url_display = args.target.url_str().unwrap_or("<current page>");
			info!(target = "pw", url = %url_display, selector = %args.selector, browser = %exec.ctx.browser, "coords single");

			let plan = navigation_plan(exec.ctx, exec.last_url, &args.target, WaitUntil::NetworkIdle);
			let timeout_ms = plan.timeout_ms;
			let target = plan.target;
			let selector = args.selector.clone();

			let data = with_session(&mut exec, plan.request, ArtifactsPolicy::Never, move |session| {
				let selector = selector.clone();
				Box::pin(async move {
					session.goto_target(&target, timeout_ms).await?;

					let result_json = session.page().evaluate_value(&js::get_element_coords_js(&selector)).await?;

					if result_json == "null" {
						return Err(PwError::ElementNotFound { selector: selector.clone() });
					}

					let coords: ElementCoords = serde_json::from_str(&result_json)?;

					Ok(CoordsData { coords, selector })
				})
			})
			.await?;

			let inputs = standard_inputs(&args.target, Some(&args.selector), None, None, None);

			Ok(CommandOutcome {
				inputs,
				data,
				delta: standard_delta(&args.target, Some(&args.selector), None),
			})
		})
	}
}

pub struct CoordsAllCommand;

impl CommandDef for CoordsAllCommand {
	const NAME: &'static str = "page.coords-all";

	type Raw = CoordsAllRaw;
	type Resolved = CoordsAllResolved;
	type Data = CoordsAllData;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		let (target, selector) = resolve_target_and_explicit_selector(raw.url, raw.url_flag, raw.selector, raw.selector_flag, env, None)?;

		Ok(CoordsAllResolved { target, selector })
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, mut exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let url_display = args.target.url_str().unwrap_or("<current page>");
			info!(target = "pw", url = %url_display, selector = %args.selector, browser = %exec.ctx.browser, "coords all");

			let plan = navigation_plan(exec.ctx, exec.last_url, &args.target, WaitUntil::NetworkIdle);
			let timeout_ms = plan.timeout_ms;
			let target = plan.target;
			let selector = args.selector.clone();

			let data = with_session(&mut exec, plan.request, ArtifactsPolicy::Never, move |session| {
				let selector = selector.clone();
				Box::pin(async move {
					session.goto_target(&target, timeout_ms).await?;

					let results_json = session.page().evaluate_value(&js::get_all_element_coords_js(&selector)).await?;

					let coords: Vec<IndexedElementCoords> = serde_json::from_str(&results_json)?;
					let count = coords.len();

					Ok(CoordsAllData { coords, selector, count })
				})
			})
			.await?;

			let inputs = standard_inputs(&args.target, Some(&args.selector), None, None, None);

			Ok(CommandOutcome {
				inputs,
				data,
				delta: standard_delta(&args.target, Some(&args.selector), None),
			})
		})
	}
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
