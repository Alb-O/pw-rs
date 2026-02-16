mod auth;
pub(crate) mod click;
mod connect;
pub(crate) mod contract;
mod daemon;
pub(crate) mod def;
pub(crate) mod dispatch;
pub(crate) mod exec_flow;
pub(crate) mod fill;
mod har;
pub mod init;
pub(crate) mod invocation;
pub(crate) mod navigate;
pub(crate) mod page;
mod protect;
pub(crate) mod registry;
mod run;
pub(crate) mod screenshot;
mod session;
mod tabs;
pub mod test;
pub(crate) mod wait;

use std::path::Path;

use crate::cli::{Cli, Commands};
use crate::commands::def::ExecMode;
use crate::context::CommandContext;
use crate::context_store::ContextState;
use crate::error::{PwError, Result};
use crate::output::OutputFormat;
use crate::relay;
use crate::runtime::{RuntimeConfig, RuntimeContext, build_runtime};
use crate::session_broker::SessionBroker;

pub async fn dispatch(cli: Cli, format: OutputFormat) -> Result<()> {
	if let Commands::Relay { ref host, port } = cli.command {
		return relay::run_relay_server(host, port).await.map_err(PwError::Anyhow);
	}

	if let Commands::Test { ref args } = cli.command {
		return test::execute(args.clone());
	}

	let config = RuntimeConfig::from(&cli);
	let RuntimeContext { ctx, mut ctx_state } = build_runtime(&config)?;
	let mut broker = SessionBroker::new(
		&ctx,
		ctx_state.session_descriptor_path(),
		Some(ctx_state.namespace_id()),
		ctx_state.refresh_requested(),
	);

	let result = match cli.command {
		Commands::Run => run::execute(&ctx, &mut ctx_state, &mut broker).await,
		Commands::Relay { .. } => unreachable!(),
		command => dispatch_command(command, &ctx, &mut ctx_state, &mut broker, format, cli.artifacts_dir.as_deref()).await,
	};

	if result.is_ok() {
		ctx_state.persist()?;
	}
	result
}

async fn dispatch_command<'ctx>(
	command: Commands,
	ctx: &'ctx CommandContext,
	ctx_state: &mut ContextState,
	broker: &mut SessionBroker<'ctx>,
	format: OutputFormat,
	artifacts_dir: Option<&'ctx Path>,
) -> Result<()> {
	let command = match command {
		Commands::Screenshot(mut args) => {
			args.output = Some(ctx_state.resolve_output(ctx, args.output));
			Commands::Screenshot(args)
		}
		cmd => cmd,
	};

	let name = format!("{command:?}");
	if let Some(invoke) = invocation::from_cli_command(command)? {
		return dispatch::dispatch_registry_command(invoke.id, invoke.args, ExecMode::Cli, ctx, ctx_state, broker, format, artifacts_dir).await;
	}

	Err(PwError::Context(format!("command is not registered for unified dispatch: {name}")))
}
