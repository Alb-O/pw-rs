use std::path::PathBuf;

use anyhow::anyhow;
use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ContextDelta, ExecCtx, ExecMode};
use crate::daemon::{self, Daemon};
use crate::error::{PwError, Result};
use crate::output::CommandInputs;
use crate::target::ResolveEnv;

#[cfg(unix)]
fn daemon_pid_path() -> PathBuf {
	if let Ok(xdg_runtime) = std::env::var("XDG_RUNTIME_DIR") {
		return PathBuf::from(xdg_runtime).join("pw-daemon.pid");
	}
	std::env::temp_dir().join("pw-daemon.pid")
}

#[cfg(unix)]
fn read_pid_file(path: &std::path::Path) -> Option<u32> {
	std::fs::read_to_string(path).ok()?.trim().parse::<u32>().ok()
}

#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonStartRaw {
	#[arg(long)]
	#[serde(default)]
	pub foreground: bool,
}

#[derive(Debug, Clone)]
pub struct DaemonStartResolved {
	pub foreground: bool,
}

pub struct DaemonStartCommand;

impl CommandDef for DaemonStartCommand {
	const NAME: &'static str = "daemon.start";

	type Raw = DaemonStartRaw;
	type Resolved = DaemonStartResolved;
	type Data = serde_json::Value;

	fn validate_mode(raw: &Self::Raw, mode: ExecMode) -> Result<()> {
		if mode == ExecMode::Batch && raw.foreground {
			return Err(PwError::UnsupportedMode(
				"command 'daemon.start' with --foreground is not available in batch/ndjson mode".to_string(),
			));
		}
		Ok(())
	}

	fn resolve(raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(DaemonStartResolved { foreground: raw.foreground })
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, _exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			if args.foreground {
				if matches!(daemon::ping().await?, Some(true)) {
					return Err(PwError::Context(
						"daemon already running; use `pw daemon status` or `pw daemon stop`".to_string(),
					));
				}

				let daemon = Daemon::start().await?;
				let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
				let run_task = tokio::spawn(async move { daemon.run_with_ready(Some(ready_tx)).await });

				if ready_rx.await.is_err() {
					run_task
						.await
						.map_err(|e| PwError::Anyhow(anyhow!("Daemon task join failed before startup: {e}")))??;
					return Err(PwError::Anyhow(anyhow!("Daemon exited before reporting startup readiness")));
				}

				eprintln!("Daemon started in foreground. Press Ctrl+C to stop.");
				run_task.await.map_err(|e| PwError::Anyhow(anyhow!("Daemon task join failed: {e}")))??;
				return Ok(CommandOutcome {
					inputs: CommandInputs {
						extra: Some(json!({ "foreground": true })),
						..Default::default()
					},
					data: json!({
						"started": true,
						"foreground": true
					}),
					delta: ContextDelta::default(),
				});
			}

			#[cfg(windows)]
			{
				return Err(PwError::Context(
					"Background daemon mode is not available on Windows; use --foreground".to_string(),
				));
			}

			#[cfg(unix)]
			{
				let pid_path = daemon_pid_path();
				if matches!(daemon::ping().await?, Some(true)) {
					return Ok(CommandOutcome {
						inputs: CommandInputs {
							extra: Some(json!({ "foreground": false })),
							..Default::default()
						},
						data: json!({
							"started": false,
							"running": true,
							"already_running": true,
							"foreground": false,
							"pid_file": pid_path.display().to_string(),
							"pid": read_pid_file(&pid_path),
							"message": "daemon already running"
						}),
						delta: ContextDelta::default(),
					});
				}

				let exe = std::env::current_exe().map_err(|e| PwError::Anyhow(anyhow!("Failed to get executable path: {e}")))?;

				let mut child = std::process::Command::new(&exe)
					.arg("daemon")
					.arg("start")
					.arg("--foreground")
					.stdin(std::process::Stdio::null())
					.stdout(std::process::Stdio::null())
					.stderr(std::process::Stdio::null())
					.spawn()
					.map_err(|e| PwError::Anyhow(anyhow!("Failed to spawn daemon: {e}")))?;

				tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

				let running = matches!(daemon::ping().await?, Some(true));
				if !running {
					return Err(PwError::Anyhow(anyhow!("Daemon failed to start")));
				}

				let child_alive = child
					.try_wait()
					.map_err(|e| PwError::Anyhow(anyhow!("Failed to inspect daemon process state: {e}")))?
					.is_none();
				let already_running = !child_alive;

				if child_alive {
					if let Some(parent) = pid_path.parent() {
						let _ = std::fs::create_dir_all(parent);
					}
					std::fs::write(&pid_path, child.id().to_string())?;
				}
				let pid = if child_alive { Some(child.id()) } else { read_pid_file(&pid_path) };

				Ok(CommandOutcome {
					inputs: CommandInputs {
						extra: Some(json!({ "foreground": false })),
						..Default::default()
					},
					data: json!({
						"started": child_alive,
						"running": true,
						"already_running": already_running,
						"foreground": false,
						"pid_file": pid_path.display().to_string(),
						"pid": pid
					}),
					delta: ContextDelta::default(),
				})
			}
		})
	}
}

#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonStopRaw {}

#[derive(Debug, Clone)]
pub struct DaemonStopResolved;

pub struct DaemonStopCommand;

impl CommandDef for DaemonStopCommand {
	const NAME: &'static str = "daemon.stop";

	type Raw = DaemonStopRaw;
	type Resolved = DaemonStopResolved;
	type Data = serde_json::Value;

	fn resolve(_raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(DaemonStopResolved)
	}

	fn execute<'exec, 'ctx>(_args: &'exec Self::Resolved, _exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let data = match daemon::shutdown().await? {
				None => json!({
					"stopped": false,
					"message": "daemon not running"
				}),
				Some(()) => {
					#[cfg(unix)]
					{
						let _ = std::fs::remove_file(daemon_pid_path());
					}
					json!({ "stopped": true })
				}
			};

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
pub struct DaemonStatusRaw {}

#[derive(Debug, Clone)]
pub struct DaemonStatusResolved;

pub struct DaemonStatusCommand;

impl CommandDef for DaemonStatusCommand {
	const NAME: &'static str = "daemon.status";

	type Raw = DaemonStatusRaw;
	type Resolved = DaemonStatusResolved;
	type Data = serde_json::Value;

	fn resolve(_raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(DaemonStatusResolved)
	}

	fn execute<'exec, 'ctx>(_args: &'exec Self::Resolved, _exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let data = if let Some(true) = daemon::ping().await? {
				let list = daemon::list_browsers().await?.unwrap_or_default();
				json!({
					"running": true,
					"browsers": list
				})
			} else {
				json!({
					"running": false,
					"message": "daemon not running"
				})
			};

			Ok(CommandOutcome {
				inputs: CommandInputs::default(),
				data,
				delta: ContextDelta::default(),
			})
		})
	}
}
