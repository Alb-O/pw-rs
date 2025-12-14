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
mod text;
mod wait;

use crate::cli::{AuthAction, Cli, Commands};
use crate::context::CommandContext;
use crate::context_store::{ContextState, ContextUpdate};
use crate::error::{PwError, Result};
use crate::relay;
use crate::session_broker::SessionBroker;
use std::path::Path;

pub async fn dispatch(cli: Cli) -> Result<()> {
    let Cli {
        verbose: _,
        auth,
        browser,
        cdp_endpoint,
        no_project,
        context,
        no_context,
        no_save_context,
        refresh_context,
        base_url,
        command,
    } = cli;

    match command {
        Commands::Relay { host, port } => relay::run_relay_server(&host, port)
            .await
            .map_err(PwError::Anyhow),
        command => {
            let ctx = CommandContext::new(browser, no_project, auth, cdp_endpoint);
            let project_root = ctx.project.as_ref().map(|p| p.paths.root.clone());
            let mut ctx_state = ContextState::new(
                project_root,
                context,
                base_url,
                no_context,
                no_save_context,
                refresh_context,
            )?;

            let mut broker = SessionBroker::new(&ctx);
            let result = dispatch_command(command, &ctx, &mut ctx_state, &mut broker).await;

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
) -> Result<()> {
    match command {
        Commands::Navigate { url } => {
            let final_url = ctx_state.resolve_url(url)?;
            let outcome = navigate::execute(&final_url, ctx, broker).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Console { url, timeout_ms } => {
            let final_url = ctx_state.resolve_url(url)?;
            let outcome = console::execute(&final_url, timeout_ms, ctx, broker).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Eval { expression, url } => {
            let final_url = ctx_state.resolve_url(url)?;
            let outcome = eval::execute(&final_url, &expression, ctx, broker).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Html { url, selector } => {
            let final_url = ctx_state.resolve_url(url)?;
            let final_selector = ctx_state.resolve_selector(selector, Some("html"))?;
            let outcome = html::execute(&final_url, &final_selector, ctx, broker).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    selector: Some(&final_selector),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Coords { url, selector } => {
            let final_url = ctx_state.resolve_url(url)?;
            let final_selector = ctx_state.resolve_selector(selector, None)?;
            let outcome = coords::execute_single(&final_url, &final_selector, ctx, broker).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    selector: Some(&final_selector),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::CoordsAll { url, selector } => {
            let final_url = ctx_state.resolve_url(url)?;
            let final_selector = ctx_state.resolve_selector(selector, None)?;
            let outcome = coords::execute_all(&final_url, &final_selector, ctx, broker).await;
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
        } => {
            let final_url = ctx_state.resolve_url(url)?;
            let resolved_output = ctx_state.resolve_output(ctx, output);
            let outcome =
                screenshot::execute(&final_url, &resolved_output, full_page, ctx, broker).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    output: Some(&resolved_output),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Click { url, selector } => {
            let final_url = ctx_state.resolve_url(url)?;
            let final_selector = ctx_state.resolve_selector(selector, None)?;
            let outcome = click::execute(&final_url, &final_selector, ctx, broker).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    selector: Some(&final_selector),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Text { url, selector } => {
            let final_url = ctx_state.resolve_url(url)?;
            let final_selector = ctx_state.resolve_selector(selector, None)?;
            let outcome = text::execute(&final_url, &final_selector, ctx, broker).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    selector: Some(&final_selector),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Elements { url } => {
            let final_url = ctx_state.resolve_url(url)?;
            let outcome = elements::execute(&final_url, ctx, broker).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: Some(&final_url),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Wait { url, condition } => {
            let final_url = ctx_state.resolve_url(url)?;
            let outcome = wait::execute(&final_url, &condition, ctx, broker).await;
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
