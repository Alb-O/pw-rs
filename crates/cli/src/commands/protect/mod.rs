use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ContextDelta, ExecCtx};
use crate::error::Result;
use crate::output::CommandInputs;
use crate::target::ResolveEnv;

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectAddRaw {
	#[arg(value_name = "PATTERN")]
	pub pattern: String,
}

#[derive(Debug, Clone)]
pub struct ProtectAddResolved {
	pub pattern: String,
}

pub struct ProtectAddCommand;

impl CommandDef for ProtectAddCommand {
	const NAME: &'static str = "protect.add";

	type Raw = ProtectAddRaw;
	type Resolved = ProtectAddResolved;
	type Data = serde_json::Value;

	fn resolve(raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(ProtectAddResolved { pattern: raw.pattern })
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let added = exec.ctx_state.add_protected(args.pattern.clone());
			let protected = exec.ctx_state.protected_urls().to_vec();
			let data = json!({
				"added": added,
				"pattern": args.pattern,
				"protected": protected,
			});

			Ok(CommandOutcome {
				inputs: CommandInputs {
					extra: Some(json!({ "pattern": args.pattern })),
					..Default::default()
				},
				data,
				delta: ContextDelta::default(),
			})
		})
	}
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectRemoveRaw {
	#[arg(value_name = "PATTERN")]
	pub pattern: String,
}

#[derive(Debug, Clone)]
pub struct ProtectRemoveResolved {
	pub pattern: String,
}

pub struct ProtectRemoveCommand;

impl CommandDef for ProtectRemoveCommand {
	const NAME: &'static str = "protect.remove";

	type Raw = ProtectRemoveRaw;
	type Resolved = ProtectRemoveResolved;
	type Data = serde_json::Value;

	fn resolve(raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(ProtectRemoveResolved { pattern: raw.pattern })
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let removed = exec.ctx_state.remove_protected(&args.pattern);
			let protected = exec.ctx_state.protected_urls().to_vec();
			let data = json!({
				"removed": removed,
				"pattern": args.pattern,
				"protected": protected,
			});

			Ok(CommandOutcome {
				inputs: CommandInputs {
					extra: Some(json!({ "pattern": args.pattern })),
					..Default::default()
				},
				data,
				delta: ContextDelta::default(),
			})
		})
	}
}

#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectListRaw {}

#[derive(Debug, Clone)]
pub struct ProtectListResolved;

pub struct ProtectListCommand;

impl CommandDef for ProtectListCommand {
	const NAME: &'static str = "protect.list";

	type Raw = ProtectListRaw;
	type Resolved = ProtectListResolved;
	type Data = serde_json::Value;

	fn resolve(_raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(ProtectListResolved)
	}

	fn execute<'exec, 'ctx>(_args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let protected = exec.ctx_state.protected_urls().to_vec();
			let count = protected.len();
			let data = json!({
				"protected": protected,
				"count": count,
			});

			Ok(CommandOutcome {
				inputs: CommandInputs::default(),
				data,
				delta: ContextDelta::default(),
			})
		})
	}
}
