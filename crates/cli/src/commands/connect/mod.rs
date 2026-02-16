//! Connect to or launch a browser with remote debugging enabled.
//!
//! This command enables control of a real browser (with your cookies, extensions, etc.)
//! to bypass bot detection systems like Cloudflare.

use std::path::PathBuf;

use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ContextDelta, ExecCtx};
use crate::error::Result;
use crate::output::CommandInputs;
use crate::session::connector::{
	clear_cdp_endpoint, discover_and_connect, kill_browser_on_port, launch_and_connect, resolve_connect_port, set_cdp_endpoint, show_cdp_endpoint,
};
use crate::target::ResolveEnv;

#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectRaw {
	/// Explicit CDP endpoint to store.
	#[arg(value_name = "URL")]
	#[serde(default)]
	pub endpoint: Option<String>,
	/// Clears stored endpoint.
	#[arg(long)]
	#[serde(default)]
	pub clear: bool,
	/// Launches a browser with remote debugging.
	#[arg(long)]
	#[serde(default)]
	pub launch: bool,
	/// Discovers an already-running remote-debugging browser.
	#[arg(long)]
	#[serde(default)]
	pub discover: bool,
	/// Kills browser process bound to the resolved debug port.
	#[arg(long)]
	#[serde(default)]
	pub kill: bool,
	/// Explicit remote-debugging port.
	#[arg(long, short)]
	#[serde(default)]
	pub port: Option<u16>,
	/// Optional user-data-dir used for launched browser profiles.
	#[arg(long)]
	#[serde(default)]
	pub user_data_dir: Option<PathBuf>,
}

/// Parsed and validated inputs for `connect`.
#[derive(Debug, Clone)]
pub struct ConnectResolved {
	/// Explicit CDP endpoint to store.
	pub endpoint: Option<String>,
	/// Clears stored endpoint.
	pub clear: bool,
	/// Launches a browser with remote debugging.
	pub launch: bool,
	/// Discovers an already-running remote-debugging browser.
	pub discover: bool,
	/// Kills browser process bound to the resolved debug port.
	pub kill: bool,
	/// Explicit remote-debugging port.
	pub port: Option<u16>,
	/// Optional user-data-dir used for launched browser profiles.
	pub user_data_dir: Option<PathBuf>,
}

/// Command implementation for `connect`.
pub struct ConnectCommand;

impl CommandDef for ConnectCommand {
	const NAME: &'static str = "connect";

	type Raw = ConnectRaw;
	type Resolved = ConnectResolved;
	type Data = serde_json::Value;

	fn resolve(raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(ConnectResolved {
			endpoint: raw.endpoint,
			clear: raw.clear,
			launch: raw.launch,
			discover: raw.discover,
			kill: raw.kill,
			port: raw.port,
			user_data_dir: raw.user_data_dir,
		})
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let port = resolve_connect_port(exec.ctx_state, args.port);
			let auth_file = exec.ctx.auth_file();

			let data = if args.kill {
				kill_browser_on_port(exec.ctx_state, port).await?
			} else if args.clear {
				clear_cdp_endpoint(exec.ctx_state)
			} else if args.launch {
				launch_and_connect(exec.ctx_state, port, args.user_data_dir.as_deref(), auth_file).await?
			} else if args.discover {
				discover_and_connect(exec.ctx_state, port, auth_file).await?
			} else if let Some(ep) = &args.endpoint {
				set_cdp_endpoint(exec.ctx_state, ep)
			} else {
				show_cdp_endpoint(exec.ctx_state)
			};

			Ok(CommandOutcome {
				inputs: CommandInputs {
					extra: Some(json!({
						"endpoint": args.endpoint,
						"clear": args.clear,
						"launch": args.launch,
						"discover": args.discover,
						"kill": args.kill,
						"port": args.port,
						"userDataDir": args.user_data_dir,
					})),
					..Default::default()
				},
				data,
				delta: ContextDelta::default(),
			})
		})
	}
}
