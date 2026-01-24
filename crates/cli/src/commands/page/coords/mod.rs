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

use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::browser::js;
use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ContextDelta, ExecCtx};
use crate::error::{PwError, Result};
use crate::output::CommandInputs;
use crate::session_broker::SessionRequest;
use crate::session_helpers::{ArtifactsPolicy, with_session};
use crate::target::{ResolveEnv, ResolvedTarget, TargetPolicy};
use crate::types::{ElementCoords, IndexedElementCoords};

/// Raw inputs from CLI or batch JSON before resolution.
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
	const NAME: &'static str = "coords";

	type Raw = CoordsRaw;
	type Resolved = CoordsResolved;
	type Data = CoordsData;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		let target = env.resolve_target(raw.url, TargetPolicy::AllowCurrentPage)?;
		let selector = env.resolve_selector(raw.selector, None)?;

		Ok(CoordsResolved { target, selector })
	}

	fn execute<'exec, 'ctx>(
		args: &'exec Self::Resolved,
		mut exec: ExecCtx<'exec, 'ctx>,
	) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let url_display = args.target.url_str().unwrap_or("<current page>");
			info!(target = "pw", url = %url_display, selector = %args.selector, browser = %exec.ctx.browser, "coords single");

			let preferred_url = args.target.preferred_url(exec.last_url);
			let timeout_ms = exec.ctx.timeout_ms();
			let target = args.target.target.clone();
			let selector = args.selector.clone();

			let req = SessionRequest::from_context(WaitUntil::NetworkIdle, exec.ctx)
				.with_preferred_url(preferred_url);

			let data = with_session(&mut exec, req, ArtifactsPolicy::Never, move |session| {
				let selector = selector.clone();
				Box::pin(async move {
					session.goto_target(&target, timeout_ms).await?;

					let result_json = session
						.page()
						.evaluate_value(&js::get_element_coords_js(&selector))
						.await?;

					if result_json == "null" {
						return Err(PwError::ElementNotFound {
							selector: selector.clone(),
						});
					}

					let coords: ElementCoords = serde_json::from_str(&result_json)?;

					Ok(CoordsData { coords, selector })
				})
			})
			.await?;

			let inputs = CommandInputs {
				url: args.target.url_str().map(String::from),
				selector: Some(args.selector.clone()),
				..Default::default()
			};

			Ok(CommandOutcome {
				inputs,
				data,
				delta: ContextDelta {
					url: args.target.url_str().map(String::from),
					selector: Some(args.selector.clone()),
					output: None,
				},
			})
		})
	}
}

pub struct CoordsAllCommand;

impl CommandDef for CoordsAllCommand {
	const NAME: &'static str = "coords-all";

	type Raw = CoordsAllRaw;
	type Resolved = CoordsAllResolved;
	type Data = CoordsAllData;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		let target = env.resolve_target(raw.url, TargetPolicy::AllowCurrentPage)?;
		let selector = env.resolve_selector(raw.selector, None)?;

		Ok(CoordsAllResolved { target, selector })
	}

	fn execute<'exec, 'ctx>(
		args: &'exec Self::Resolved,
		mut exec: ExecCtx<'exec, 'ctx>,
	) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let url_display = args.target.url_str().unwrap_or("<current page>");
			info!(target = "pw", url = %url_display, selector = %args.selector, browser = %exec.ctx.browser, "coords all");

			let preferred_url = args.target.preferred_url(exec.last_url);
			let timeout_ms = exec.ctx.timeout_ms();
			let target = args.target.target.clone();
			let selector = args.selector.clone();

			let req = SessionRequest::from_context(WaitUntil::NetworkIdle, exec.ctx)
				.with_preferred_url(preferred_url);

			let data = with_session(&mut exec, req, ArtifactsPolicy::Never, move |session| {
				let selector = selector.clone();
				Box::pin(async move {
					session.goto_target(&target, timeout_ms).await?;

					let results_json = session
						.page()
						.evaluate_value(&js::get_all_element_coords_js(&selector))
						.await?;

					let coords: Vec<IndexedElementCoords> = serde_json::from_str(&results_json)?;
					let count = coords.len();

					Ok(CoordsAllData {
						coords,
						selector,
						count,
					})
				})
			})
			.await?;

			let inputs = CommandInputs {
				url: args.target.url_str().map(String::from),
				selector: Some(args.selector.clone()),
				..Default::default()
			};

			Ok(CommandOutcome {
				inputs,
				data,
				delta: ContextDelta {
					url: args.target.url_str().map(String::from),
					selector: Some(args.selector.clone()),
					output: None,
				},
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
