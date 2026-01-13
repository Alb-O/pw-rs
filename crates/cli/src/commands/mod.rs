mod auth;
mod click;
mod connect;
mod console;
mod coords;
mod daemon;
mod elements;
mod eval;
mod fill;
mod html;
pub mod init;
mod navigate;
mod protect;
mod read;
mod run;
mod screenshot;
mod session;
mod snapshot;
mod tabs;
mod text;
mod wait;

use crate::cli::{
    AuthAction, Cli, Commands, DaemonAction, ProtectAction, SessionAction, TabsAction,
};
use crate::context::CommandContext;
use crate::context_store::{ContextState, ContextUpdate};
use crate::error::{PwError, Result};
use crate::output::OutputFormat;
use crate::relay;
use crate::runtime::{RuntimeConfig, RuntimeContext, build_runtime};
use crate::session_broker::SessionBroker;
use crate::target::{Resolve, ResolveEnv};
use std::path::Path;

pub async fn dispatch(cli: Cli, format: OutputFormat) -> Result<()> {
    // Handle relay separately - doesn't need runtime
    if let Commands::Relay { ref host, port } = cli.command {
        return relay::run_relay_server(host, port)
            .await
            .map_err(PwError::Anyhow);
    }

    // Build runtime once (single source of truth for setup)
    let config = RuntimeConfig::from(&cli);
    let RuntimeContext { ctx, mut ctx_state } = build_runtime(&config)?;
    let mut broker = SessionBroker::new(
        &ctx,
        ctx_state.session_descriptor_path(),
        ctx_state.refresh_requested(),
    );

    let result = match cli.command {
        Commands::Run => run::execute(&ctx, &mut ctx_state, &mut broker).await,
        Commands::Relay { .. } => unreachable!("handled above"),
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

async fn dispatch_command(
    command: Commands,
    ctx: &CommandContext,
    ctx_state: &mut ContextState,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    artifacts_dir: Option<&Path>,
) -> Result<()> {
    dispatch_command_inner(command, ctx, ctx_state, broker, format, artifacts_dir).await
}

async fn dispatch_command_inner(
    command: Commands,
    ctx: &CommandContext,
    ctx_state: &mut ContextState,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    artifacts_dir: Option<&Path>,
) -> Result<()> {
    // Whether we have a CDP endpoint (enables --no-context mode to operate on current page)
    let has_cdp = ctx.cdp_endpoint().is_some();

    match command {
        Commands::Navigate { url, url_flag } => {
            let raw = navigate::NavigateRaw::from_cli(url, url_flag);
            let env = ResolveEnv::new(ctx_state, has_cdp, "navigate");
            let resolved = raw.resolve(&env)?;
            let last_url = ctx_state.last_url();
            let actual_url =
                navigate::execute_resolved(&resolved, ctx, broker, format, last_url).await?;
            // Record the actual browser URL (may differ from input due to redirects)
            ctx_state.record(ContextUpdate {
                url: Some(&actual_url),
                ..Default::default()
            });
            Ok(())
        }
        Commands::Console {
            url,
            timeout_ms,
            url_flag,
        } => {
            let raw = console::ConsoleRaw::from_cli(url_flag.or(url), timeout_ms);
            let env = ResolveEnv::new(ctx_state, has_cdp, "console");
            let resolved = raw.resolve(&env)?;
            let last_url = ctx_state.last_url();
            let outcome = console::execute_resolved(&resolved, ctx, broker, format, last_url).await;
            if outcome.is_ok() {
                ctx_state.record_from_target(&resolved.target, None);
            }
            outcome
        }
        Commands::Eval {
            expression,
            url,
            expression_flag,
            file,
            url_flag,
        } => {
            // Priority: --file > --expr > positional
            let final_expr = if let Some(path) = file {
                Some(std::fs::read_to_string(&path).map_err(|e| {
                    PwError::Context(format!(
                        "failed to read expression from {}: {}",
                        path.display(),
                        e
                    ))
                })?)
            } else {
                expression_flag.or(expression)
            };
            let raw = eval::EvalRaw::from_cli(url, url_flag, final_expr.clone(), None);
            let env = ResolveEnv::new(ctx_state, has_cdp, "eval");
            let resolved = raw.resolve(&env)?;
            let last_url = ctx_state.last_url();
            let outcome = eval::execute_resolved(&resolved, ctx, broker, format, last_url).await;
            if outcome.is_ok() {
                ctx_state.record_from_target(&resolved.target, None);
            }
            outcome
        }
        Commands::Html {
            url,
            selector,
            url_flag,
            selector_flag,
        } => {
            // Build raw args from CLI
            let raw = html::HtmlRaw::from_cli(url, selector, url_flag, selector_flag);

            // Resolve using typed target system
            let env = ResolveEnv::new(ctx_state, has_cdp, "html");
            let resolved = raw.resolve(&env)?;

            // Execute with resolved args
            let last_url = ctx_state.last_url();
            let outcome = html::execute_resolved(&resolved, ctx, broker, format, last_url).await;

            // Record context from typed target
            if outcome.is_ok() {
                ctx_state.record_from_target(&resolved.target, Some(&resolved.selector));
            }
            outcome
        }
        Commands::Coords {
            url,
            selector,
            url_flag,
            selector_flag,
        } => {
            let raw = coords::CoordsRaw::from_cli(url_flag.or(url), selector_flag.or(selector));
            let env = ResolveEnv::new(ctx_state, has_cdp, "coords");
            let resolved = raw.resolve(&env)?;
            let last_url = ctx_state.last_url();
            let outcome =
                coords::execute_single_resolved(&resolved, ctx, broker, format, last_url).await;
            if outcome.is_ok() {
                ctx_state.record_from_target(&resolved.target, Some(&resolved.selector));
            }
            outcome
        }
        Commands::CoordsAll {
            url,
            selector,
            url_flag,
            selector_flag,
        } => {
            let raw = coords::CoordsAllRaw::from_cli(url_flag.or(url), selector_flag.or(selector));
            let env = ResolveEnv::new(ctx_state, has_cdp, "coords-all");
            let resolved = raw.resolve(&env)?;
            let last_url = ctx_state.last_url();
            let outcome =
                coords::execute_all_resolved(&resolved, ctx, broker, format, last_url).await;
            if outcome.is_ok() {
                ctx_state.record_from_target(&resolved.target, Some(&resolved.selector));
            }
            outcome
        }
        Commands::Screenshot {
            url,
            output,
            full_page,
            url_flag,
        } => {
            // Resolve output path with project context
            let resolved_output = ctx_state.resolve_output(ctx, output);
            let raw = screenshot::ScreenshotRaw::from_cli(
                url,
                url_flag,
                Some(resolved_output.clone()),
                full_page,
            );
            let env = ResolveEnv::new(ctx_state, has_cdp, "screenshot");
            let resolved = raw.resolve(&env)?;
            let last_url = ctx_state.last_url();
            let outcome =
                screenshot::execute_resolved(&resolved, ctx, broker, format, last_url).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: resolved.target.url_str(),
                    output: Some(&resolved_output),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Click {
            url,
            selector,
            url_flag,
            selector_flag,
            wait_ms,
        } => {
            let raw =
                click::ClickRaw::from_cli(url, selector, url_flag, selector_flag, Some(wait_ms));
            let env = ResolveEnv::new(ctx_state, has_cdp, "click");
            let resolved = raw.resolve(&env)?;
            let last_url = ctx_state.last_url();
            let after_url =
                click::execute_resolved(&resolved, ctx, broker, format, artifacts_dir, last_url)
                    .await?;
            // Record the actual browser URL after click (may differ if click caused navigation)
            ctx_state.record(ContextUpdate {
                url: Some(&after_url),
                selector: Some(&resolved.selector),
                ..Default::default()
            });
            Ok(())
        }
        Commands::Text {
            url,
            selector,
            url_flag,
            selector_flag,
        } => {
            let raw = text::TextRaw::from_cli(url, selector, url_flag, selector_flag);
            let env = ResolveEnv::new(ctx_state, has_cdp, "text");
            let resolved = raw.resolve(&env)?;
            let last_url = ctx_state.last_url();
            let outcome =
                text::execute_resolved(&resolved, ctx, broker, format, artifacts_dir, last_url)
                    .await;
            if outcome.is_ok() {
                ctx_state.record_from_target(&resolved.target, Some(&resolved.selector));
            }
            outcome
        }
        Commands::Fill {
            text,
            selector,
            url,
        } => {
            let raw = fill::FillRaw::from_cli(url, selector, Some(text));
            let env = ResolveEnv::new(ctx_state, has_cdp, "fill");
            let resolved = raw.resolve(&env)?;
            let last_url = ctx_state.last_url();
            let outcome = fill::execute_resolved(&resolved, ctx, broker, format, last_url).await;
            if outcome.is_ok() {
                ctx_state.record_from_target(&resolved.target, Some(&resolved.selector));
            }
            outcome
        }
        Commands::Read {
            url,
            url_flag,
            output_format,
            metadata,
        } => {
            let raw = read::ReadRaw::from_cli(url_flag.or(url), output_format, metadata);
            let env = ResolveEnv::new(ctx_state, has_cdp, "read");
            let resolved = raw.resolve(&env)?;
            let last_url = ctx_state.last_url();
            let outcome = read::execute_resolved(&resolved, ctx, broker, format, last_url).await;
            if outcome.is_ok() {
                ctx_state.record_from_target(&resolved.target, None);
            }
            outcome
        }
        Commands::Elements {
            url,
            wait,
            timeout_ms,
            url_flag,
        } => {
            let raw = elements::ElementsRaw::from_cli(url_flag.or(url), wait, timeout_ms);
            let env = ResolveEnv::new(ctx_state, has_cdp, "elements");
            let resolved = raw.resolve(&env)?;
            let last_url = ctx_state.last_url();
            let outcome =
                elements::execute_resolved(&resolved, ctx, broker, format, artifacts_dir, last_url)
                    .await;
            if outcome.is_ok() {
                ctx_state.record_from_target(&resolved.target, None);
            }
            outcome
        }
        Commands::Snapshot {
            url,
            url_flag,
            text_only,
            full,
            max_text_length,
        } => {
            let raw = snapshot::SnapshotRaw::from_cli(
                url,
                url_flag,
                text_only,
                full,
                Some(max_text_length),
            );
            let env = ResolveEnv::new(ctx_state, has_cdp, "snapshot");
            let resolved = raw.resolve(&env)?;
            let last_url = ctx_state.last_url();
            let outcome =
                snapshot::execute_resolved(&resolved, ctx, broker, format, artifacts_dir, last_url)
                    .await;
            if outcome.is_ok() {
                ctx_state.record_from_target(&resolved.target, None);
            }
            outcome
        }
        Commands::Wait {
            url,
            condition,
            url_flag,
        } => {
            let raw = wait::WaitRaw::from_cli(url_flag.or(url), Some(condition));
            let env = ResolveEnv::new(ctx_state, has_cdp, "wait");
            let resolved = raw.resolve(&env)?;
            let last_url = ctx_state.last_url();
            let outcome = wait::execute_resolved(&resolved, ctx, broker, format, last_url).await;
            if outcome.is_ok() {
                ctx_state.record_from_target(&resolved.target, None);
            }
            outcome
        }
        Commands::Auth { action } => match action {
            AuthAction::Login {
                url,
                output,
                timeout,
            } => {
                // Resolve output path with project context
                let resolved_output = resolve_auth_output(ctx, &output);
                let raw = auth::LoginRaw::from_cli(url, resolved_output.clone(), timeout);
                let env = ResolveEnv::new(ctx_state, has_cdp, "auth-login");
                let resolved = raw.resolve(&env)?;
                let last_url = ctx_state.last_url();
                let outcome = auth::login_resolved(&resolved, ctx, broker, last_url).await;
                if outcome.is_ok() {
                    ctx_state.record(ContextUpdate {
                        url: resolved.target.url_str(),
                        output: Some(&resolved_output),
                        ..Default::default()
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
            profile,
        } => {
            connect::run(
                ctx_state, format, endpoint, clear, launch, discover, kill, port, profile,
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
    }
}

fn resolve_auth_output(ctx: &CommandContext, output: &Path) -> std::path::PathBuf {
    if output.is_absolute() || output.parent().is_some_and(|p| !p.as_os_str().is_empty()) {
        return output.to_path_buf();
    }

    if let Some(ref proj) = ctx.project {
        proj.paths.auth_file(output.to_string_lossy().as_ref())
    } else {
        output.to_path_buf()
    }
}
