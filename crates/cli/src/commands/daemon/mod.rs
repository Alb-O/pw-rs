use std::path::PathBuf;

use anyhow::anyhow;
use serde_json::json;

use crate::daemon::{self, Daemon};
use crate::error::{PwError, Result};
use crate::output::{OutputFormat, ResultBuilder, print_result};

/// Get the daemon PID file path for the current user.
///
/// Uses XDG runtime directory when available and falls back to temp dir.
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

pub async fn start(foreground: bool, format: OutputFormat) -> Result<()> {
	if foreground {
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

		let result = ResultBuilder::new("daemon start")
			.data(json!({
				"started": true,
				"foreground": true
			}))
			.build();
		print_result(&result, format);

		run_task.await.map_err(|e| PwError::Anyhow(anyhow!("Daemon task join failed: {e}")))??;
		return Ok(());
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
			let result = ResultBuilder::new("daemon start")
				.data(json!({
					"started": false,
					"running": true,
					"already_running": true,
					"foreground": false,
					"pid_file": pid_path.display().to_string(),
					"pid": read_pid_file(&pid_path),
					"message": "daemon already running"
				}))
				.build();
			print_result(&result, format);
			return Ok(());
		}

		// Spawn a new process for the daemon rather than forking.
		// This avoids issues with tokio runtime after fork and keeps stdio working.
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

		// Wait a bit for daemon to start.
		tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

		// Check if it's running.
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

		let result = ResultBuilder::new("daemon start")
			.data(json!({
				"started": child_alive,
				"running": true,
				"already_running": already_running,
				"foreground": false,
				"pid_file": pid_path.display().to_string(),
				"pid": pid
			}))
			.build();
		print_result(&result, format);

		Ok(())
	}
}

pub async fn stop(format: OutputFormat) -> Result<()> {
	match daemon::shutdown().await? {
		None => {
			let result = ResultBuilder::new("daemon stop")
				.data(json!({
					"stopped": false,
					"message": "daemon not running"
				}))
				.build();
			print_result(&result, format);
			Ok(())
		}
		Some(()) => {
			#[cfg(unix)]
			{
				let _ = std::fs::remove_file(daemon_pid_path());
			}
			let result = ResultBuilder::new("daemon stop").data(json!({ "stopped": true })).build();
			print_result(&result, format);
			Ok(())
		}
	}
}

pub async fn status(format: OutputFormat) -> Result<()> {
	let Some(true) = daemon::ping().await? else {
		let result = ResultBuilder::new("daemon status")
			.data(json!({
				"running": false,
				"message": "daemon not running"
			}))
			.build();
		print_result(&result, format);
		return Ok(());
	};

	let list = daemon::list_browsers().await?.unwrap_or_default();
	let result = ResultBuilder::new("daemon status")
		.data(json!({
			"running": true,
			"browsers": list
		}))
		.build();
	print_result(&result, format);
	Ok(())
}
