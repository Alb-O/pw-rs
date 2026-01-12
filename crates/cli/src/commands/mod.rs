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
mod tabs;
mod text;
mod wait;

use crate::args;
use crate::cli::{
    AuthAction, Cli, Commands, DaemonAction, ProtectAction, SessionAction, TabsAction,
};
use crate::context::CommandContext;
use crate::context_store::{ContextState, ContextUpdate, is_current_page_sentinel};
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
        no_daemon,
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
        Commands::Run => {
            // Batch mode - create context and run in NDJSON mode
            let project = if no_project {
                None
            } else {
                crate::project::Project::detect()
            };
            let project_root = project.as_ref().map(|p| p.paths.root.clone());
            let mut ctx_state = ContextState::new(
                project_root.clone(),
                context,
                base_url,
                no_context,
                no_save_context,
                refresh_context,
            )?;

            let resolved_cdp = cdp_endpoint.or_else(|| ctx_state.cdp_endpoint().map(String::from));

            let ctx = CommandContext::new(
                browser,
                no_project,
                auth,
                resolved_cdp,
                launch_server,
                no_daemon,
            );

            let mut broker = SessionBroker::new(
                &ctx,
                ctx_state.session_descriptor_path(),
                ctx_state.refresh_requested(),
            );

            let result = run::execute(&ctx, &mut ctx_state, &mut broker).await;

            if result.is_ok() {
                ctx_state.persist()?;
            }

            result
        }
        command => {
            // Create context state first to check for stored CDP endpoint
            let project = if no_project {
                None
            } else {
                crate::project::Project::detect()
            };
            let project_root = project.as_ref().map(|p| p.paths.root.clone());
            let mut ctx_state = ContextState::new(
                project_root.clone(),
                context,
                base_url,
                no_context,
                no_save_context,
                refresh_context,
            )?;

            // Use CLI cdp_endpoint if provided, otherwise fall back to stored context
            let resolved_cdp = cdp_endpoint.or_else(|| ctx_state.cdp_endpoint().map(String::from));

            let ctx = CommandContext::new(
                browser,
                no_project,
                auth,
                resolved_cdp,
                launch_server,
                no_daemon,
            );

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

/// Compute the preferred_url for page selection.
/// When the resolved URL is the sentinel (__CURRENT_PAGE__), fall back to last_url from context.
fn compute_preferred_url<'a>(final_url: &'a str, ctx_state: &'a ContextState) -> Option<&'a str> {
    if is_current_page_sentinel(final_url) {
        ctx_state.last_url()
    } else {
        Some(final_url)
    }
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
            let final_url = ctx_state.resolve_url_with_cdp(url_flag.or(url), has_cdp)?;
            let preferred_url = compute_preferred_url(&final_url, ctx_state);
            let actual_url =
                navigate::execute(&final_url, ctx, broker, format, preferred_url).await?;
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
            let final_url = ctx_state.resolve_url_with_cdp(url_flag.or(url), has_cdp)?;
            let preferred_url = compute_preferred_url(&final_url, ctx_state);
            let outcome =
                console::execute(&final_url, timeout_ms, ctx, broker, format, preferred_url).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: if is_current_page_sentinel(&final_url) {
                        None
                    } else {
                        Some(&final_url)
                    },
                    ..Default::default()
                });
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
                std::fs::read_to_string(&path).map_err(|e| {
                    PwError::Context(format!(
                        "failed to read expression from {}: {}",
                        path.display(),
                        e
                    ))
                })?
            } else {
                expression_flag.or(expression).ok_or_else(|| {
                    PwError::Context(
                        "expression is required (provide positionally, via --expr, or via --file)"
                            .into(),
                    )
                })?
            };
            let final_url = ctx_state.resolve_url_with_cdp(url_flag.or(url), has_cdp)?;
            let preferred_url = compute_preferred_url(&final_url, ctx_state);
            let outcome =
                eval::execute(&final_url, &final_expr, ctx, broker, format, preferred_url).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: if is_current_page_sentinel(&final_url) {
                        None
                    } else {
                        Some(&final_url)
                    },
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Html {
            url,
            selector,
            url_flag,
            selector_flag,
        } => {
            let resolved =
                args::resolve_url_and_selector(url.clone(), url_flag, selector_flag.or(selector));
            let final_url = ctx_state.resolve_url_with_cdp(resolved.url, has_cdp)?;
            let final_selector = ctx_state.resolve_selector(resolved.selector, Some("html"))?;
            let preferred_url = compute_preferred_url(&final_url, ctx_state);
            let outcome = html::execute(
                &final_url,
                &final_selector,
                ctx,
                broker,
                format,
                preferred_url,
            )
            .await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: if is_current_page_sentinel(&final_url) {
                        None
                    } else {
                        Some(&final_url)
                    },
                    selector: Some(&final_selector),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Coords {
            url,
            selector,
            url_flag,
            selector_flag,
        } => {
            let final_url = ctx_state.resolve_url_with_cdp(url_flag.or(url), has_cdp)?;
            let final_selector = ctx_state.resolve_selector(selector_flag.or(selector), None)?;
            let preferred_url = compute_preferred_url(&final_url, ctx_state);
            let outcome = coords::execute_single(
                &final_url,
                &final_selector,
                ctx,
                broker,
                format,
                preferred_url,
            )
            .await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: if is_current_page_sentinel(&final_url) {
                        None
                    } else {
                        Some(&final_url)
                    },
                    selector: Some(&final_selector),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::CoordsAll {
            url,
            selector,
            url_flag,
            selector_flag,
        } => {
            let final_url = ctx_state.resolve_url_with_cdp(url_flag.or(url), has_cdp)?;
            let final_selector = ctx_state.resolve_selector(selector_flag.or(selector), None)?;
            let preferred_url = compute_preferred_url(&final_url, ctx_state);
            let outcome = coords::execute_all(
                &final_url,
                &final_selector,
                ctx,
                broker,
                format,
                preferred_url,
            )
            .await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: if is_current_page_sentinel(&final_url) {
                        None
                    } else {
                        Some(&final_url)
                    },
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
            let final_url = ctx_state.resolve_url_with_cdp(url_flag.or(url), has_cdp)?;
            let resolved_output = ctx_state.resolve_output(ctx, output);
            let preferred_url = compute_preferred_url(&final_url, ctx_state);
            let outcome = screenshot::execute(
                &final_url,
                &resolved_output,
                full_page,
                ctx,
                broker,
                format,
                preferred_url,
            )
            .await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: if is_current_page_sentinel(&final_url) {
                        None
                    } else {
                        Some(&final_url)
                    },
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
            let resolved =
                args::resolve_url_and_selector(url.clone(), url_flag, selector_flag.or(selector));
            let final_url = ctx_state.resolve_url_with_cdp(resolved.url, has_cdp)?;
            let final_selector = ctx_state.resolve_selector(resolved.selector, None)?;
            let preferred_url = compute_preferred_url(&final_url, ctx_state);
            let after_url = click::execute(
                &final_url,
                &final_selector,
                wait_ms,
                ctx,
                broker,
                format,
                artifacts_dir,
                preferred_url,
            )
            .await?;
            // Record the actual browser URL after click (may differ if click caused navigation)
            ctx_state.record(ContextUpdate {
                url: Some(&after_url),
                selector: Some(&final_selector),
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
            let resolved =
                args::resolve_url_and_selector(url.clone(), url_flag, selector_flag.or(selector));
            let final_url = ctx_state.resolve_url_with_cdp(resolved.url, has_cdp)?;
            let final_selector = ctx_state.resolve_selector(resolved.selector, None)?;
            let preferred_url = compute_preferred_url(&final_url, ctx_state);
            let outcome = text::execute(
                &final_url,
                &final_selector,
                ctx,
                broker,
                format,
                artifacts_dir,
                preferred_url,
            )
            .await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: if is_current_page_sentinel(&final_url) {
                        None
                    } else {
                        Some(&final_url)
                    },
                    selector: Some(&final_selector),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Fill {
            text,
            selector,
            url,
        } => {
            let final_url = ctx_state.resolve_url_with_cdp(url, has_cdp)?;
            let final_selector = ctx_state.resolve_selector(selector, None)?;
            let preferred_url = compute_preferred_url(&final_url, ctx_state);
            let outcome = fill::execute(
                &final_url,
                &final_selector,
                &text,
                ctx,
                broker,
                format,
                preferred_url,
            )
            .await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: if is_current_page_sentinel(&final_url) {
                        None
                    } else {
                        Some(&final_url)
                    },
                    selector: Some(&final_selector),
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Read {
            url,
            url_flag,
            output_format,
            metadata,
        } => {
            let final_url = ctx_state.resolve_url_with_cdp(url_flag.or(url), has_cdp)?;
            let preferred_url = compute_preferred_url(&final_url, ctx_state);
            let outcome = read::execute(
                &final_url,
                output_format,
                metadata,
                ctx,
                broker,
                format,
                preferred_url,
            )
            .await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: if is_current_page_sentinel(&final_url) {
                        None
                    } else {
                        Some(&final_url)
                    },
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Elements {
            url,
            wait,
            timeout_ms,
            url_flag,
        } => {
            let final_url = ctx_state.resolve_url_with_cdp(url_flag.or(url), has_cdp)?;
            let preferred_url = compute_preferred_url(&final_url, ctx_state);
            let outcome = elements::execute(
                &final_url,
                wait,
                timeout_ms,
                ctx,
                broker,
                format,
                artifacts_dir,
                preferred_url,
            )
            .await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: if is_current_page_sentinel(&final_url) {
                        None
                    } else {
                        Some(&final_url)
                    },
                    ..Default::default()
                });
            }
            outcome
        }
        Commands::Wait {
            url,
            condition,
            url_flag,
        } => {
            let final_url = ctx_state.resolve_url_with_cdp(url_flag.or(url), has_cdp)?;
            let preferred_url = compute_preferred_url(&final_url, ctx_state);
            let outcome =
                wait::execute(&final_url, &condition, ctx, broker, format, preferred_url).await;
            if outcome.is_ok() {
                ctx_state.record(ContextUpdate {
                    url: if is_current_page_sentinel(&final_url) {
                        None
                    } else {
                        Some(&final_url)
                    },
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
                let final_url = ctx_state.resolve_url_with_cdp(url, has_cdp)?;
                let resolved_output = resolve_auth_output(ctx, &output);
                let preferred_url = compute_preferred_url(&final_url, ctx_state);
                let outcome = auth::login(
                    &final_url,
                    &resolved_output,
                    timeout,
                    ctx,
                    broker,
                    preferred_url,
                )
                .await;
                if outcome.is_ok() {
                    ctx_state.record(ContextUpdate {
                        url: if is_current_page_sentinel(&final_url) {
                            None
                        } else {
                            Some(&final_url)
                        },
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
                let final_url = ctx_state.resolve_url_with_cdp(url, has_cdp)?;
                let preferred_url = compute_preferred_url(&final_url, ctx_state);
                let outcome =
                    auth::cookies(&final_url, &cookie_format, ctx, broker, preferred_url).await;
                if outcome.is_ok() {
                    ctx_state.record(ContextUpdate {
                        url: if is_current_page_sentinel(&final_url) {
                            None
                        } else {
                            Some(&final_url)
                        },
                        ..Default::default()
                    });
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
            port,
            profile,
        } => {
            connect::run(
                ctx_state, format, endpoint, clear, launch, discover, port, profile,
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
