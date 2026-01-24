//! Command dispatch for batch execution.

use std::path::PathBuf;

use super::{BatchRequest, BatchResponse, click, fill, navigate, page, screenshot, wait};
use crate::context::CommandContext;
use crate::context_store::{ContextState, ContextUpdate};
use crate::output::{CommandInputs, OutputFormat};
use crate::session_broker::SessionBroker;
use crate::target::{Resolve, ResolveEnv};

/// Dispatches a single batch command and returns the response.
///
/// This handles URL/selector resolution from context state, delegates to the
/// appropriate command module, and records state updates on success.
pub async fn execute_batch_command(
	request: &BatchRequest,
	ctx: &CommandContext,
	ctx_state: &mut ContextState,
	broker: &mut SessionBroker<'_>,
) -> BatchResponse {
	let id = request.id.clone();
	let command = request.command.as_str();
	let args = &request.args;
	let has_cdp = ctx.cdp_endpoint().is_some();

	let get_str = |key: &str| args.get(key).and_then(|v| v.as_str()).map(String::from);

	match command {
		"navigate" | "nav" => {
			let raw: navigate::NavigateRaw = match serde_json::from_value(args.clone()) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(id, "navigate", "INVALID_INPUT", &e.to_string());
				}
			};

			let env = ResolveEnv::new(ctx_state, has_cdp, "navigate");
			let resolved = match raw.resolve(&env) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(id, "navigate", "INVALID_INPUT", &e.to_string());
				}
			};

			let last_url = ctx_state.last_url();
			match navigate::execute_resolved(&resolved, ctx, broker, OutputFormat::Ndjson, last_url)
				.await
			{
				Ok(actual_url) => {
					ctx_state.record(ContextUpdate {
						url: Some(&actual_url),
						..Default::default()
					});
					BatchResponse::success(id, "navigate", serde_json::json!({ "url": actual_url }))
						.with_inputs(CommandInputs {
							url: resolved.target.url_str().map(String::from),
							..Default::default()
						})
				}
				Err(e) => BatchResponse::error(id, "navigate", "NAVIGATION_FAILED", &e.to_string()),
			}
		}

		"click" => {
			let raw: click::ClickRaw = match serde_json::from_value(args.clone()) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(id, "click", "INVALID_INPUT", &e.to_string());
				}
			};

			let env = ResolveEnv::new(ctx_state, has_cdp, "click");
			let resolved = match raw.resolve(&env) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(id, "click", "INVALID_INPUT", &e.to_string());
				}
			};

			let last_url = ctx_state.last_url();
			match click::execute_resolved(
				&resolved,
				ctx,
				broker,
				OutputFormat::Ndjson,
				None,
				last_url,
			)
			.await
			{
				Ok(after_url) => {
					ctx_state.record(ContextUpdate {
						url: Some(&after_url),
						selector: Some(&resolved.selector),
						..Default::default()
					});
					BatchResponse::success(
						id,
						"click",
						serde_json::json!({
							"beforeUrl": resolved.target.url_str(),
							"afterUrl": after_url,
							"selector": resolved.selector,
						}),
					)
				}
				Err(e) => BatchResponse::error(id, "click", "CLICK_FAILED", &e.to_string()),
			}
		}

		"page.text" => {
			let raw: page::text::TextRaw = match serde_json::from_value(args.clone()) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(id, "page.text", "INVALID_INPUT", &e.to_string());
				}
			};

			let env = ResolveEnv::new(ctx_state, has_cdp, "page.text");
			let resolved = match raw.resolve(&env) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(id, "page.text", "INVALID_INPUT", &e.to_string());
				}
			};

			let last_url = ctx_state.last_url();
			match page::text::execute_resolved(
				&resolved,
				ctx,
				broker,
				OutputFormat::Ndjson,
				None,
				last_url,
			)
			.await
			{
				Ok(()) => {
					ctx_state.record_from_target(&resolved.target, Some(&resolved.selector));
					BatchResponse::success_empty(id, "page.text")
				}
				Err(e) => BatchResponse::error(id, "page.text", "TEXT_FAILED", &e.to_string()),
			}
		}

		"page.html" => {
			let raw: page::html::HtmlRaw = match serde_json::from_value(args.clone()) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(id, "page.html", "INVALID_INPUT", &e.to_string());
				}
			};

			let env = ResolveEnv::new(ctx_state, has_cdp, "page.html");
			let resolved = match raw.resolve(&env) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(id, "page.html", "INVALID_INPUT", &e.to_string());
				}
			};

			let last_url = ctx_state.last_url();
			match page::html::execute_resolved(
				&resolved,
				ctx,
				broker,
				OutputFormat::Ndjson,
				last_url,
			)
			.await
			{
				Ok(()) => {
					ctx_state.record_from_target(&resolved.target, Some(&resolved.selector));
					BatchResponse::success_empty(id, "page.html")
				}
				Err(e) => BatchResponse::error(id, "page.html", "HTML_FAILED", &e.to_string()),
			}
		}

		"screenshot" | "ss" => {
			let resolved_output =
				ctx_state.resolve_output(ctx, get_str("output").map(PathBuf::from));

			let mut raw: screenshot::ScreenshotRaw = match serde_json::from_value(args.clone()) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(id, "screenshot", "INVALID_INPUT", &e.to_string());
				}
			};
			raw.output = Some(resolved_output.clone());

			let env = ResolveEnv::new(ctx_state, has_cdp, "screenshot");
			let resolved = match raw.resolve(&env) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(id, "screenshot", "INVALID_INPUT", &e.to_string());
				}
			};

			let last_url = ctx_state.last_url();
			match screenshot::execute_resolved(
				&resolved,
				ctx,
				broker,
				OutputFormat::Ndjson,
				last_url,
			)
			.await
			{
				Ok(()) => {
					ctx_state.record(ContextUpdate {
						url: resolved.target.url_str(),
						output: Some(&resolved_output),
						..Default::default()
					});
					BatchResponse::success(
						id,
						"screenshot",
						serde_json::json!({ "path": resolved_output }),
					)
				}
				Err(e) => {
					BatchResponse::error(id, "screenshot", "SCREENSHOT_FAILED", &e.to_string())
				}
			}
		}

		"page.eval" => {
			let raw: page::eval::EvalRaw = match serde_json::from_value(args.clone()) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(id, "page.eval", "INVALID_INPUT", &e.to_string());
				}
			};

			let env = ResolveEnv::new(ctx_state, has_cdp, "page.eval");
			let resolved = match raw.resolve(&env) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(id, "page.eval", "INVALID_INPUT", &e.to_string());
				}
			};

			let last_url = ctx_state.last_url();
			match page::eval::execute_resolved(
				&resolved,
				ctx,
				broker,
				OutputFormat::Ndjson,
				last_url,
			)
			.await
			{
				Ok(()) => {
					ctx_state.record_from_target(&resolved.target, None);
					BatchResponse::success_empty(id, "page.eval")
				}
				Err(e) => BatchResponse::error(id, "page.eval", "EVAL_FAILED", &e.to_string()),
			}
		}

		"fill" => {
			let raw: fill::FillRaw = match serde_json::from_value(args.clone()) {
				Ok(r) => r,
				Err(e) => return BatchResponse::error(id, "fill", "INVALID_INPUT", &e.to_string()),
			};

			let env = ResolveEnv::new(ctx_state, has_cdp, "fill");
			let resolved = match raw.resolve(&env) {
				Ok(r) => r,
				Err(e) => return BatchResponse::error(id, "fill", "INVALID_INPUT", &e.to_string()),
			};

			let last_url = ctx_state.last_url();
			match fill::execute_resolved(&resolved, ctx, broker, OutputFormat::Ndjson, last_url)
				.await
			{
				Ok(()) => {
					ctx_state.record_from_target(&resolved.target, Some(&resolved.selector));
					BatchResponse::success_empty(id, "fill")
				}
				Err(e) => BatchResponse::error(id, "fill", "FILL_FAILED", &e.to_string()),
			}
		}

		"wait" => {
			let raw: wait::WaitRaw = match serde_json::from_value(args.clone()) {
				Ok(r) => r,
				Err(e) => return BatchResponse::error(id, "wait", "INVALID_INPUT", &e.to_string()),
			};

			let env = ResolveEnv::new(ctx_state, has_cdp, "wait");
			let resolved = match raw.resolve(&env) {
				Ok(r) => r,
				Err(e) => return BatchResponse::error(id, "wait", "INVALID_INPUT", &e.to_string()),
			};

			let last_url = ctx_state.last_url();
			match wait::execute_resolved(&resolved, ctx, broker, OutputFormat::Ndjson, last_url)
				.await
			{
				Ok(()) => {
					ctx_state.record_from_target(&resolved.target, None);
					BatchResponse::success_empty(id, "wait")
				}
				Err(e) => BatchResponse::error(id, "wait", "WAIT_FAILED", &e.to_string()),
			}
		}

		"page.elements" | "page.els" => {
			let raw: page::elements::ElementsRaw = match serde_json::from_value(args.clone()) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(
						id,
						"page.elements",
						"INVALID_INPUT",
						&e.to_string(),
					);
				}
			};

			let env = ResolveEnv::new(ctx_state, has_cdp, "page.elements");
			let resolved = match raw.resolve(&env) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(
						id,
						"page.elements",
						"INVALID_INPUT",
						&e.to_string(),
					);
				}
			};

			let last_url = ctx_state.last_url();
			match page::elements::execute_resolved(
				&resolved,
				ctx,
				broker,
				OutputFormat::Ndjson,
				None,
				last_url,
			)
			.await
			{
				Ok(()) => {
					ctx_state.record_from_target(&resolved.target, None);
					BatchResponse::success_empty(id, "page.elements")
				}
				Err(e) => {
					BatchResponse::error(id, "page.elements", "ELEMENTS_FAILED", &e.to_string())
				}
			}
		}

		"page.snapshot" | "page.snap" => {
			let raw: page::snapshot::SnapshotRaw = match serde_json::from_value(args.clone()) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(
						id,
						"page.snapshot",
						"INVALID_INPUT",
						&e.to_string(),
					);
				}
			};

			let env = ResolveEnv::new(ctx_state, has_cdp, "page.snapshot");
			let resolved = match raw.resolve(&env) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(
						id,
						"page.snapshot",
						"INVALID_INPUT",
						&e.to_string(),
					);
				}
			};

			let last_url = ctx_state.last_url();
			match page::snapshot::execute_resolved(
				&resolved,
				ctx,
				broker,
				OutputFormat::Ndjson,
				None,
				last_url,
			)
			.await
			{
				Ok(()) => {
					ctx_state.record_from_target(&resolved.target, None);
					BatchResponse::success_empty(id, "page.snapshot")
				}
				Err(e) => {
					BatchResponse::error(id, "page.snapshot", "SNAPSHOT_FAILED", &e.to_string())
				}
			}
		}

		"page.console" | "page.con" => {
			let raw: page::console::ConsoleRaw = match serde_json::from_value(args.clone()) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(
						id,
						"page.console",
						"INVALID_INPUT",
						&e.to_string(),
					);
				}
			};

			let env = ResolveEnv::new(ctx_state, has_cdp, "page.console");
			let resolved = match raw.resolve(&env) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(
						id,
						"page.console",
						"INVALID_INPUT",
						&e.to_string(),
					);
				}
			};

			let last_url = ctx_state.last_url();
			match page::console::execute_resolved(
				&resolved,
				ctx,
				broker,
				OutputFormat::Ndjson,
				last_url,
			)
			.await
			{
				Ok(()) => {
					ctx_state.record_from_target(&resolved.target, None);
					BatchResponse::success_empty(id, "page.console")
				}
				Err(e) => {
					BatchResponse::error(id, "page.console", "CONSOLE_FAILED", &e.to_string())
				}
			}
		}

		"page.read" => {
			let raw: page::read::ReadRaw = match serde_json::from_value(args.clone()) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(id, "page.read", "INVALID_INPUT", &e.to_string());
				}
			};

			let env = ResolveEnv::new(ctx_state, has_cdp, "page.read");
			let resolved = match raw.resolve(&env) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(id, "page.read", "INVALID_INPUT", &e.to_string());
				}
			};

			let last_url = ctx_state.last_url();
			match page::read::execute_resolved(
				&resolved,
				ctx,
				broker,
				OutputFormat::Ndjson,
				last_url,
			)
			.await
			{
				Ok(()) => {
					ctx_state.record_from_target(&resolved.target, None);
					BatchResponse::success_empty(id, "page.read")
				}
				Err(e) => BatchResponse::error(id, "page.read", "READ_FAILED", &e.to_string()),
			}
		}

		"page.coords" => {
			let raw: page::coords::CoordsRaw = match serde_json::from_value(args.clone()) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(
						id,
						"page.coords",
						"INVALID_INPUT",
						&e.to_string(),
					);
				}
			};

			let env = ResolveEnv::new(ctx_state, has_cdp, "page.coords");
			let resolved = match raw.resolve(&env) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(
						id,
						"page.coords",
						"INVALID_INPUT",
						&e.to_string(),
					);
				}
			};

			let last_url = ctx_state.last_url();
			match page::coords::execute_single_resolved(
				&resolved,
				ctx,
				broker,
				OutputFormat::Ndjson,
				last_url,
			)
			.await
			{
				Ok(()) => {
					ctx_state.record_from_target(&resolved.target, Some(&resolved.selector));
					BatchResponse::success_empty(id, "page.coords")
				}
				Err(e) => BatchResponse::error(id, "page.coords", "COORDS_FAILED", &e.to_string()),
			}
		}

		"page.coords_all" => {
			let raw: page::coords::CoordsAllRaw = match serde_json::from_value(args.clone()) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(
						id,
						"page.coords_all",
						"INVALID_INPUT",
						&e.to_string(),
					);
				}
			};

			let env = ResolveEnv::new(ctx_state, has_cdp, "page.coords_all");
			let resolved = match raw.resolve(&env) {
				Ok(r) => r,
				Err(e) => {
					return BatchResponse::error(
						id,
						"page.coords_all",
						"INVALID_INPUT",
						&e.to_string(),
					);
				}
			};

			let last_url = ctx_state.last_url();
			match page::coords::execute_all_resolved(
				&resolved,
				ctx,
				broker,
				OutputFormat::Ndjson,
				last_url,
			)
			.await
			{
				Ok(()) => {
					ctx_state.record_from_target(&resolved.target, Some(&resolved.selector));
					BatchResponse::success_empty(id, "page.coords_all")
				}
				Err(e) => {
					BatchResponse::error(id, "page.coords_all", "COORDS_FAILED", &e.to_string())
				}
			}
		}

		_ => BatchResponse::error(
			id,
			command,
			"UNKNOWN_COMMAND",
			&format!("Unknown command: {}", command),
		),
	}
}
