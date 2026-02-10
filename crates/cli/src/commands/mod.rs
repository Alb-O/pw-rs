mod auth;
pub(crate) mod click;
mod connect;
mod daemon;
pub(crate) mod def;
pub(crate) mod dispatch;
pub(crate) mod fill;
pub mod init;
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

use crate::cli::{
	AuthAction, Cli, Commands, DaemonAction, ProtectAction, SessionAction, TabsAction,
};
use crate::commands::def::ExecMode;
use crate::context::CommandContext;
use crate::context_store::ContextState;
use crate::error::{PwError, Result};
use crate::output::OutputFormat;
use crate::relay;
use crate::runtime::{RuntimeConfig, RuntimeContext, build_runtime};
use crate::session_broker::SessionBroker;
use crate::target::{Resolve, ResolveEnv};

pub async fn dispatch(cli: Cli, format: OutputFormat) -> Result<()> {
	if let Commands::Relay { ref host, port } = cli.command {
		return relay::run_relay_server(host, port)
			.await
			.map_err(PwError::Anyhow);
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
		command => {
			dispatch_command(
				command,
				&ctx,
				&mut ctx_state,
				&mut broker,
				format,
				cli.artifacts_dir.as_deref(),
			)
			.await
		}
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
	let has_cdp = ctx.cdp_endpoint().is_some();

	let command = match command {
		Commands::Screenshot(mut args) => {
			args.output = Some(ctx_state.resolve_output(ctx, args.output));
			Commands::Screenshot(args)
		}
		cmd => cmd,
	};

	if let Some((id, args)) = command.into_registry_args() {
		return dispatch::dispatch_registry_command(
			id,
			args,
			ExecMode::Cli,
			ctx,
			ctx_state,
			broker,
			format,
			artifacts_dir,
		)
		.await;
	}

	dispatch_ad_hoc(command, ctx, ctx_state, broker, format, has_cdp).await
}

async fn dispatch_ad_hoc<'ctx>(
	command: Commands,
	ctx: &'ctx CommandContext,
	ctx_state: &mut ContextState,
	broker: &mut SessionBroker<'ctx>,
	format: OutputFormat,
	has_cdp: bool,
) -> Result<()> {
	match command {
		Commands::Auth { action } => match action {
			AuthAction::Login {
				url,
				output,
				timeout,
			} => {
				let resolved_output = resolve_auth_output(ctx, &output);
				let raw = auth::LoginRaw::from_cli(url, resolved_output.clone(), timeout);
				let env = ResolveEnv::new(ctx_state, has_cdp, "auth-login");
				let resolved = raw.resolve(&env)?;
				let last_url = ctx_state.last_url();
				let outcome = auth::login_resolved(&resolved, ctx, broker, last_url).await;
				if outcome.is_ok() {
					ctx_state.apply_delta(def::ContextDelta {
						url: resolved.target.url_str().map(String::from),
						output: Some(resolved_output),
						selector: None,
					});
				}
				outcome
			}
			AuthAction::Cookies {
				url,
				format: cookie_format,
			} => {
				let raw = auth::CookiesRaw::from_cli(url, cookie_format);
				let env = ResolveEnv::new(ctx_state, has_cdp, "auth-cookies");
				let resolved = raw.resolve(&env)?;
				let last_url = ctx_state.last_url();
				let outcome = auth::cookies_resolved(&resolved, ctx, broker, last_url).await;
				if outcome.is_ok() {
					ctx_state.record_from_target(&resolved.target, None);
				}
				outcome
			}
			AuthAction::Show { file } => auth::show(&file).await,
			AuthAction::Listen { host, port } => auth::listen(&host, port, ctx).await,
		},
		Commands::Session { action } => match action {
			SessionAction::Status => session::status(ctx_state, format).await,
			SessionAction::Clear => session::clear(ctx_state, format).await,
			SessionAction::Start { headful } => {
				session::start(ctx_state, broker, headful, format).await
			}
			SessionAction::Stop => session::stop(ctx_state, broker, format).await,
		},
		Commands::Daemon { action } => match action {
			DaemonAction::Start { foreground } => daemon::start(foreground, format).await,
			DaemonAction::Stop => daemon::stop(format).await,
			DaemonAction::Status => daemon::status(format).await,
		},
		Commands::Init {
			path,
			template,
			no_config,
			no_example,
			typescript,
			force,
			nix,
		} => init::execute(init::InitOptions {
			path,
			template,
			no_config,
			no_example,
			typescript,
			force,
			nix,
		}),
		Commands::Relay { .. } => unreachable!("handled earlier"),
		Commands::Run => unreachable!("handled earlier"),
		Commands::Connect {
			endpoint,
			clear,
			launch,
			discover,
			kill,
			port,
			user_data_dir,
		} => {
			connect::run(
				ctx_state,
				format,
				connect::ConnectOptions {
					endpoint,
					clear,
					launch,
					discover,
					kill,
					port,
					user_data_dir,
				},
			)
			.await
		}
		Commands::Tabs(action) => {
			let protected = ctx_state.protected_urls();
			match action {
				TabsAction::List => tabs::list(ctx, broker, format, protected).await,
				TabsAction::Switch { target } => {
					tabs::switch(&target, ctx, broker, format, protected).await
				}
				TabsAction::Close { target } => {
					tabs::close_tab(&target, ctx, broker, format, protected).await
				}
				TabsAction::New { url } => tabs::new_tab(url.as_deref(), ctx, broker, format).await,
			}
		}
		Commands::Protect(action) => match action {
			ProtectAction::Add { pattern } => protect::add(ctx_state, format, pattern),
			ProtectAction::Remove { pattern } => protect::remove(ctx_state, format, &pattern),
			ProtectAction::List => protect::list(ctx_state, format),
		},
		Commands::Test { .. } => unreachable!("handled earlier"),
		// Registry-backed commands should have been handled above
		Commands::Navigate(_)
		| Commands::Screenshot(_)
		| Commands::Click(_)
		| Commands::Fill(_)
		| Commands::Wait(_)
		| Commands::Page(_) => {
			unreachable!("registry command reached ad-hoc dispatch")
		}
	}
}

fn resolve_auth_output(ctx: &CommandContext, output: &Path) -> std::path::PathBuf {
	if output.is_absolute() || output.parent().is_some_and(|p| !p.as_os_str().is_empty()) {
		return output.to_path_buf();
	}

	ctx.namespace_auth_dir().join(output)
}
