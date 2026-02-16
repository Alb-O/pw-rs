use std::path::PathBuf;

use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::cli::{CliHarContentPolicy, CliHarMode};
use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ContextDelta, ExecCtx};
use crate::context_store::types::HarDefaults;
use crate::error::Result;
use crate::output::CommandInputs;
use crate::target::ResolveEnv;

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarSetRaw {
	#[arg(value_name = "FILE")]
	pub file: PathBuf,
	#[arg(long, value_enum, default_value = "attach")]
	pub content: CliHarContentPolicy,
	#[arg(long, value_enum, default_value = "full")]
	pub mode: CliHarMode,
	#[arg(long)]
	pub omit_content: bool,
	#[arg(long, value_name = "PATTERN")]
	pub url_filter: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HarSetResolved {
	pub har: HarDefaults,
}

pub struct HarSetCommand;

impl CommandDef for HarSetCommand {
	const NAME: &'static str = "har.set";

	type Raw = HarSetRaw;
	type Resolved = HarSetResolved;
	type Data = serde_json::Value;

	fn resolve(raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		let har = HarDefaults {
			path: raw.file,
			content_policy: raw.content.into(),
			mode: raw.mode.into(),
			omit_content: raw.omit_content,
			url_filter: raw.url_filter,
		};
		Ok(HarSetResolved { har })
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let changed = exec.ctx_state.set_har_defaults(args.har.clone());
			let data = json!({
				"enabled": true,
				"changed": changed,
				"har": har_payload(&args.har),
			});

			Ok(CommandOutcome {
				inputs: CommandInputs {
					extra: Some(json!({
						"path": args.har.path,
						"contentPolicy": args.har.content_policy,
						"mode": args.har.mode,
						"omitContent": args.har.omit_content,
						"urlFilter": args.har.url_filter,
					})),
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
pub struct HarShowRaw {}

#[derive(Debug, Clone)]
pub struct HarShowResolved;

pub struct HarShowCommand;

impl CommandDef for HarShowCommand {
	const NAME: &'static str = "har.show";

	type Raw = HarShowRaw;
	type Resolved = HarShowResolved;
	type Data = serde_json::Value;

	fn resolve(_raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(HarShowResolved)
	}

	fn execute<'exec, 'ctx>(_args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let har = exec.ctx_state.har_defaults().cloned();
			let data = json!({
				"enabled": har.is_some(),
				"har": har.as_ref().map(har_payload),
			});

			Ok(CommandOutcome {
				inputs: CommandInputs::default(),
				data,
				delta: ContextDelta::default(),
			})
		})
	}
}

#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarClearRaw {}

#[derive(Debug, Clone)]
pub struct HarClearResolved;

pub struct HarClearCommand;

impl CommandDef for HarClearCommand {
	const NAME: &'static str = "har.clear";

	type Raw = HarClearRaw;
	type Resolved = HarClearResolved;
	type Data = serde_json::Value;

	fn resolve(_raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(HarClearResolved)
	}

	fn execute<'exec, 'ctx>(_args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let cleared = exec.ctx_state.clear_har_defaults();
			let data = json!({
				"cleared": cleared,
				"enabled": exec.ctx_state.har_defaults().is_some(),
			});

			Ok(CommandOutcome {
				inputs: CommandInputs::default(),
				data,
				delta: ContextDelta::default(),
			})
		})
	}
}

fn har_payload(har: &HarDefaults) -> serde_json::Value {
	json!({
		"path": har.path,
		"contentPolicy": har.content_policy,
		"mode": har.mode,
		"omitContent": har.omit_content,
		"urlFilter": har.url_filter,
	})
}
