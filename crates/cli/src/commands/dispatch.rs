//! Unified command dispatch for CLI and batch modes.

use std::path::Path;

use crate::commands::def::{ExecCtx, ExecMode};
use crate::commands::registry::{self, CommandId, emit_success};
use crate::context::CommandContext;
use crate::context_store::ContextState;
use crate::error::Result;
use crate::output::OutputFormat;
use crate::session_broker::SessionBroker;
use crate::target::ResolveEnv;

/// Executes a registry-backed command through the unified pipeline.
///
/// Builds resolution environment, runs the command via [`registry::run_command`],
/// emits output, and applies context delta.
#[allow(clippy::too_many_arguments)]
pub async fn dispatch_registry_command<'ctx>(
	id: CommandId,
	args: serde_json::Value,
	mode: ExecMode,
	ctx: &'ctx CommandContext,
	ctx_state: &mut ContextState,
	broker: &mut SessionBroker<'ctx>,
	format: OutputFormat,
	artifacts_dir: Option<&'ctx Path>,
) -> Result<()> {
	let has_cdp = ctx.cdp_endpoint().is_some();
	let env = ResolveEnv::new(ctx_state, has_cdp, registry::command_name(id));
	let last_url = ctx_state.last_url().map(str::to_string);

	let exec = ExecCtx {
		mode,
		ctx,
		broker,
		format,
		artifacts_dir,
		last_url: last_url.as_deref(),
	};

	let out = registry::run_command(id, args, &env, exec).await?;
	emit_success(out.command, out.inputs, out.data, format);
	out.delta.apply(ctx_state);
	Ok(())
}
