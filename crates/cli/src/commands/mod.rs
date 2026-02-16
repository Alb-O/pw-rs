mod auth;
pub(crate) mod click;
mod connect;
pub(crate) mod contract;
mod daemon;
pub(crate) mod def;
pub(crate) mod exec_flow;
pub(crate) mod fill;
pub(crate) mod flow;
pub(crate) mod graph;
mod har;
pub mod init;
pub(crate) mod navigate;
pub(crate) mod page;
mod protect;
pub(crate) mod registry;
pub(crate) mod screenshot;
mod session;
mod tabs;
pub mod test;
pub(crate) mod wait;

use std::io::Write;
use std::path::Path;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::cli::{BatchArgs, Cli, Commands, DaemonAction, ExecArgs, ProfileAction, ProfileArgs};
use crate::commands::def::{ExecCtx, ExecMode};
use crate::commands::registry::{command_name, lookup_command_exact, run_command};
use crate::context_store::storage::StatePaths;
use crate::context_store::types::CliConfig;
use crate::error::{PwError, Result};
use crate::output::{CommandError, ErrorCode, OutputFormat};
use crate::protocol::{CommandRequest, CommandResponse, EffectiveRuntime, RuntimeSpec, SCHEMA_VERSION, print_response};
use crate::runtime::{RuntimeConfig, build_runtime};
use crate::session_broker::SessionBroker;
use crate::workspace::normalize_profile;

pub async fn dispatch(cli: Cli) -> Result<()> {
	match cli.command {
		Commands::Exec(args) => {
			let request = parse_exec_request(&args)?;
			let response = execute_request(request, Some(args.profile), ExecMode::Cli, args.artifacts_dir.as_deref()).await;
			print_response(&response, cli.format);
		}
		Commands::Batch(args) => {
			run_batch(args, cli.format).await?;
		}
		Commands::Profile(args) => {
			let response = run_profile_action(args)?;
			print_response(&response, cli.format);
		}
		Commands::Daemon(args) => {
			let (op, input) = match args.action {
				DaemonAction::Start { foreground } => ("daemon.start".to_string(), json!({ "foreground": foreground })),
				DaemonAction::Stop => ("daemon.stop".to_string(), json!({})),
				DaemonAction::Status => ("daemon.status".to_string(), json!({})),
			};

			let request = CommandRequest {
				schema_version: SCHEMA_VERSION,
				request_id: None,
				op,
				input,
				runtime: Some(RuntimeSpec {
					profile: Some("default".to_string()),
					overrides: None,
				}),
			};
			let response = execute_request(request, Some("default".to_string()), ExecMode::Cli, None).await;
			print_response(&response, cli.format);
		}
	}

	Ok(())
}

fn parse_exec_request(args: &ExecArgs) -> Result<CommandRequest> {
	if let Some(file) = &args.file {
		let content = std::fs::read_to_string(file)?;
		return serde_json::from_str::<CommandRequest>(&content).map_err(PwError::Json);
	}

	let op = args
		.op
		.clone()
		.ok_or_else(|| PwError::Context("missing operation: use `pw exec <op>` or `--file`".to_string()))?;

	let input = match &args.input {
		Some(raw) => serde_json::from_str::<Value>(raw)?,
		None => Value::Object(Default::default()),
	};

	Ok(CommandRequest {
		schema_version: SCHEMA_VERSION,
		request_id: None,
		op,
		input,
		runtime: Some(RuntimeSpec {
			profile: Some(args.profile.clone()),
			overrides: None,
		}),
	})
}

async fn run_batch(args: BatchArgs, format: OutputFormat) -> Result<()> {
	let stdin = tokio::io::stdin();
	let mut reader = BufReader::new(stdin);
	let mut line = String::new();
	let mut stdout = std::io::stdout();
	let default_profile = args.profile;

	loop {
		line.clear();
		match reader.read_line(&mut line).await {
			Ok(0) => break,
			Ok(_) => {}
			Err(err) => {
				tracing::error!(target = "pw.batch", error = %err, "stdin read failed");
				break;
			}
		}

		let line = line.trim();
		if line.is_empty() {
			continue;
		}

		let request: CommandRequest = match serde_json::from_str(line) {
			Ok(value) => value,
			Err(err) => {
				let response = error_response(
					None,
					"unknown".to_string(),
					CommandError {
						code: ErrorCode::InvalidInput,
						message: format!("Invalid request JSON: {err}"),
						details: None,
					},
					None,
				);
				write_batch_response(&mut stdout, &response, format);
				continue;
			}
		};

		if request.op == "quit" || request.op == "exit" {
			let response = CommandResponse {
				schema_version: SCHEMA_VERSION,
				request_id: request.request_id,
				op: "quit".to_string(),
				ok: true,
				inputs: None,
				data: Some(json!({ "quit": true })),
				error: None,
				duration_ms: None,
				artifacts: Vec::new(),
				diagnostics: Vec::new(),
				context_delta: None,
				effective_runtime: None,
			};
			write_batch_response(&mut stdout, &response, format);
			break;
		}

		if request.op == "ping" {
			let response = CommandResponse {
				schema_version: SCHEMA_VERSION,
				request_id: request.request_id,
				op: "ping".to_string(),
				ok: true,
				inputs: None,
				data: Some(json!({ "alive": true })),
				error: None,
				duration_ms: None,
				artifacts: Vec::new(),
				diagnostics: Vec::new(),
				context_delta: None,
				effective_runtime: None,
			};
			write_batch_response(&mut stdout, &response, format);
			continue;
		}

		let response = execute_request(request, Some(default_profile.clone()), ExecMode::Batch, None).await;
		write_batch_response(&mut stdout, &response, format);
	}

	Ok(())
}

fn write_batch_response(stdout: &mut std::io::Stdout, response: &CommandResponse, format: OutputFormat) {
	match format {
		OutputFormat::Ndjson => {
			if let Ok(line) = serde_json::to_string(response) {
				let _ = writeln!(stdout, "{line}");
			}
		}
		_ => {
			print_response(response, format);
		}
	}
}

async fn execute_request(request: CommandRequest, fallback_profile: Option<String>, mode: ExecMode, artifacts_dir: Option<&Path>) -> CommandResponse {
	if request.schema_version != SCHEMA_VERSION {
		return error_response(
			request.request_id,
			request.op,
			CommandError {
				code: ErrorCode::InvalidInput,
				message: format!("unsupported schemaVersion {} (expected {})", request.schema_version, SCHEMA_VERSION),
				details: None,
			},
			None,
		);
	}

	let runtime = request.runtime.clone().unwrap_or_default();
	let profile = normalize_profile(runtime.profile.as_deref().or(fallback_profile.as_deref()).unwrap_or("default"));
	let overrides = runtime.overrides.unwrap_or_default();

	let runtime_config = RuntimeConfig {
		profile: profile.clone(),
		overrides,
	};

	let crate::runtime::RuntimeContext { ctx, mut ctx_state, info } = match build_runtime(&runtime_config) {
		Ok(runtime) => runtime,
		Err(err) => {
			return error_response(request.request_id, request.op, err.to_command_error(), None);
		}
	};

	let effective_runtime = EffectiveRuntime {
		profile: info.profile.clone(),
		browser: Some(info.browser.to_string()),
		cdp_endpoint: info.cdp_endpoint.clone(),
		timeout_ms: info.timeout_ms,
	};

	let mut broker = SessionBroker::new(
		&ctx,
		ctx_state.session_descriptor_path(),
		Some(ctx_state.profile_id()),
		ctx_state.refresh_requested(),
	);

	let Some(cmd_id) = lookup_command_exact(request.op.as_str()) else {
		let unknown_op = request.op.clone();
		return error_response(
			request.request_id,
			unknown_op.clone(),
			CommandError {
				code: ErrorCode::InvalidInput,
				message: format!("unknown operation: {unknown_op}"),
				details: None,
			},
			Some(effective_runtime.clone()),
		);
	};

	let has_cdp = ctx.cdp_endpoint().is_some();
	let last_url = ctx_state.last_url().map(str::to_string);
	let exec = ExecCtx {
		mode,
		ctx: &ctx,
		ctx_state: &mut ctx_state,
		broker: &mut broker,
		format: OutputFormat::Json,
		artifacts_dir,
		last_url: last_url.as_deref(),
	};

	match run_command(cmd_id, request.input, has_cdp, exec).await {
		Ok(outcome) => {
			let op = outcome.command.to_string();
			let request_id = request.request_id;
			let delta = outcome.delta.clone();
			delta.clone().apply(&mut ctx_state);
			if let Err(err) = ctx_state.persist() {
				return error_response(request_id, op, err.to_command_error(), Some(effective_runtime.clone()));
			}

			CommandResponse::success(request_id, op, outcome.inputs, outcome.data, delta, effective_runtime)
		}
		Err(err) => error_response(
			request.request_id,
			command_name(cmd_id).to_string(),
			err.to_command_error(),
			Some(effective_runtime),
		),
	}
}

fn run_profile_action(args: ProfileArgs) -> Result<CommandResponse> {
	let cwd = std::env::current_dir()?;

	match args.action {
		ProfileAction::List => {
			let root = cwd.join("playwright").join(crate::workspace::STATE_VERSION_DIR).join("profiles");
			let mut profiles = Vec::new();
			if root.exists() {
				for entry in std::fs::read_dir(root)? {
					let entry = entry?;
					if entry.file_type()?.is_dir() {
						profiles.push(entry.file_name().to_string_lossy().to_string());
					}
				}
			}
			profiles.sort();
			Ok(CommandResponse {
				schema_version: SCHEMA_VERSION,
				request_id: None,
				op: "profile.list".to_string(),
				ok: true,
				inputs: None,
				data: Some(json!({ "profiles": profiles })),
				error: None,
				duration_ms: None,
				artifacts: Vec::new(),
				diagnostics: Vec::new(),
				context_delta: None,
				effective_runtime: None,
			})
		}
		ProfileAction::Show { name } => {
			let profile = normalize_profile(&name);
			let paths = StatePaths::new(&cwd, &profile);
			let cfg = if paths.config.exists() {
				let content = std::fs::read_to_string(paths.config)?;
				serde_json::from_str::<CliConfig>(&content)?
			} else {
				CliConfig::new()
			};
			Ok(CommandResponse {
				schema_version: SCHEMA_VERSION,
				request_id: None,
				op: "profile.show".to_string(),
				ok: true,
				inputs: None,
				data: Some(serde_json::to_value(cfg)?),
				error: None,
				duration_ms: None,
				artifacts: Vec::new(),
				diagnostics: Vec::new(),
				context_delta: None,
				effective_runtime: None,
			})
		}
		ProfileAction::Set { name, file } => {
			let profile = normalize_profile(&name);
			let paths = StatePaths::new(&cwd, &profile);
			let content = std::fs::read_to_string(file)?;
			let mut cfg = serde_json::from_str::<CliConfig>(&content)?;
			if cfg.schema == 0 {
				cfg.schema = crate::context_store::types::SCHEMA_VERSION;
			}
			if let Some(parent) = paths.config.parent() {
				std::fs::create_dir_all(parent)?;
			}
			std::fs::write(paths.config, serde_json::to_string_pretty(&cfg)?)?;
			Ok(CommandResponse {
				schema_version: SCHEMA_VERSION,
				request_id: None,
				op: "profile.set".to_string(),
				ok: true,
				inputs: None,
				data: Some(json!({ "profile": profile, "written": true })),
				error: None,
				duration_ms: None,
				artifacts: Vec::new(),
				diagnostics: Vec::new(),
				context_delta: None,
				effective_runtime: None,
			})
		}
		ProfileAction::Delete { name } => {
			let profile = normalize_profile(&name);
			let paths = StatePaths::new(&cwd, &profile);
			let removed = if paths.profile_dir.exists() {
				std::fs::remove_dir_all(paths.profile_dir)?;
				true
			} else {
				false
			};
			Ok(CommandResponse {
				schema_version: SCHEMA_VERSION,
				request_id: None,
				op: "profile.delete".to_string(),
				ok: true,
				inputs: None,
				data: Some(json!({ "profile": profile, "removed": removed })),
				error: None,
				duration_ms: None,
				artifacts: Vec::new(),
				diagnostics: Vec::new(),
				context_delta: None,
				effective_runtime: None,
			})
		}
	}
}

fn error_response(request_id: Option<String>, op: String, error: CommandError, effective_runtime: Option<EffectiveRuntime>) -> CommandResponse {
	CommandResponse::error(request_id, op, error, effective_runtime)
}
