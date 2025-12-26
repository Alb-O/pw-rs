mod auth;
mod click;
mod console;
mod coords;
mod elements;
mod eval;
mod html;
pub mod init;
mod navigate;
mod screenshot;
mod session;
mod text;
mod wait;

use crate::cli::{AuthAction, Cli, Commands, SessionAction};
use crate::context::CommandContext;
use crate::context_store::{ContextState, ContextUpdate};
use crate::error::{PwError, Result};
use crate::output::OutputFormat;
use crate::relay;
use crate::session_broker::SessionBroker;
use std::path::Path;

pub async fn dispatch(cli: Cli, format: OutputFormat) -> Result<()> {
    let Cli {
        verbose: _,
        format: _cli_format,
        auth,
        browser,
        cdp_endpoint,
        launch_server,
        no_project,
        context,
        no_context,
        no_save_context,
        refresh_context,
        base_url,
        artifacts_dir,
        command,
    } = cli;



    match command {
        Commands::Relay { host, port } => relay::run_relay_server(&host, port)
            .await
            .map_err(PwError::Anyhow),
        command => {
            let ctx = CommandContext::new(browser, no_project, auth, cdp_endpoint, launch_server);
            let project_root = ctx.project.as_ref().map(|p| p.paths.root.clone());
            let mut ctx_state = ContextState::new(
                project_root,
                context,
                base_url,
                no_context,
                no_save_context,
                refresh_context,
            )?;

            let mut broker = SessionBroker::new(
                &ctx,
                ctx_state.session_descriptor_path(),
                ctx_state.refresh_requested(),
            );
            let result = dispatch_command(
                command,
                &ctx,
                &mut ctx_state,
                &mut broker,
                format,
                artifacts_dir.as_deref(),
            )
            .await;

            if result.is_ok() {
                ctx_state.persist()?;
            }

            result
        }
    }
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
    match command {
        Commands::Navigate { url, url_flag } => {
            let final_url = ctx_state.resolve_url(url_flag.or(url))?;
            let outcome = navigate::execute(&final_url, ctx, broker, format).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Console { url, timeout_ms, url_flag } => {
            let final_url = ctx_state.resolve_url(url_flag.or(url))?;
            let outcome = console::execute(&final_url, timeout_ms, ctx, broker, format).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Eval { expression, url, expression_flag, url_flag } => {
            // Named flags take precedence over positional args
            let final_expr = expression_flag.or(expression).ok_or_else(|| {
                PwError::Context("expression is required (provide positionally or via --expr)".into())
            })?;
            let final_url = ctx_state.resolve_url(url_flag.or(url))?;
            let outcome = eval::execute(&final_url, &final_expr, ctx, broker, format).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Html { url, selector, url_flag, selector_flag } => {
            let final_url = ctx_state.resolve_url(url_flag.or(url))?;
            let final_selector = ctx_state.resolve_selector(selector_flag.or(selector), Some("html"))?;
            let outcome = html::execute(&final_url, &final_selector, ctx, broker, format).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    selector: Some(&final_selector),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Coords { url, selector, url_flag, selector_flag } => {
            let final_url = ctx_state.resolve_url(url_flag.or(url))?;
            let final_selector = ctx_state.resolve_selector(selector_flag.or(selector), None)?;
            let outcome = coords::execute_single(&final_url, &final_selector, ctx, broker, format).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    selector: Some(&final_selector),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::CoordsAll { url, selector, url_flag, selector_flag } => {
            let final_url = ctx_state.resolve_url(url_flag.or(url))?;
            let final_selector = ctx_state.resolve_selector(selector_flag.or(selector), None)?;
            let outcome = coords::execute_all(&final_url, &final_selector, ctx, broker, format).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    selector: Some(&final_selector),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Screenshot {
            url,
            output,
            full_page,
            url_flag,
        } => {
            let final_url = ctx_state.resolve_url(url_flag.or(url))?;
            let resolved_output = ctx_state.resolve_output(ctx, output);
            let outcome =
                screenshot::execute(&final_url, &resolved_output, full_page, ctx, broker, format).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    output: Some(&resolved_output),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Click { url, selector, url_flag, selector_flag } => {
            let final_url = ctx_state.resolve_url(url_flag.or(url))?;
            let final_selector = ctx_state.resolve_selector(selector_flag.or(selector), None)?;
            let outcome = click::execute(&final_url, &final_selector, ctx, broker, format, artifacts_dir).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    selector: Some(&final_selector),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Text { url, selector, url_flag, selector_flag } => {
            let final_url = ctx_state.resolve_url(url_flag.or(url))?;
            let final_selector = ctx_state.resolve_selector(selector_flag.or(selector), None)?;
            let outcome = text::execute(&final_url, &final_selector, ctx, broker, format, artifacts_dir).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    selector: Some(&final_selector),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Elements { url, wait, timeout_ms, url_flag } => {
            let final_url = ctx_state.resolve_url(url_flag.or(url))?;
            let outcome = elements::execute(&final_url, wait, timeout_ms, ctx, broker, format, artifacts_dir).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Wait { url, condition, url_flag } => {
            let final_url = ctx_state.resolve_url(url_flag.or(url))?;
            let outcome = wait::execute(&final_url, &condition, ctx, broker, format).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Auth { action } => match action {
            AuthAction::Login {
                url,
                output,
                timeout,
            } => {
                let final_url = ctx_state.resolve_url(url)?;
                let resolved_output = resolve_auth_output(ctx, &output);
                let outcome = auth::login(&final_url, &resolved_output, timeout, ctx, broker).await;
                if outcome.is_ok() {
                    ctx_state.record(ContextUpdate {
                        url: Some(&final_url),
                        output: Some(&resolved_output),
                        ..Default::default()
                    });
                }
                outcome
            }
            AuthAction::Cookies { url, format } => {
                let final_url = ctx_state.resolve_url(url)?;
                let outcome = auth::cookies(&final_url, &format, ctx, broker).await;
                if outcome.is_ok() {
                    ctx_state.record(ContextUpdate {
                        url: Some(&final_url),
                        ..Default::default()
                    });
                }
                outcome
            }
            AuthAction::Show { file } => auth::show(&file).await,
        },
        Commands::Session { action } => match action {
            SessionAction::Status => session::status(ctx_state, format).await,
            SessionAction::Clear => session::clear(ctx_state, format).await,
            SessionAction::Start { headful } => session::start(ctx_state, broker, headful, format).await,
            SessionAction::Stop => session::stop(ctx_state, broker, format).await,
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
    }
}

fn resolve_auth_output(ctx: &CommandContext, output: &Path) -> std::path::PathBuf {
    if output.is_absolute() || output.parent().map_or(false, |p| !p.as_os_str().is_empty()) {
        return output.to_path_buf();
    }

    if let Some(ref proj) = ctx.project {
        proj.paths.auth_file(output.to_string_lossy().as_ref())
    } else {
        output.to_path_buf()
    }
}
