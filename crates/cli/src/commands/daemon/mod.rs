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

pub async fn start(foreground: bool, format: OutputFormat) -> Result<()> {
	if foreground {
		let result = ResultBuilder::new("daemon start")
			.data(json!({
				"started": true,
				"foreground": true
			}))
			.build();
		print_result(&result, format);

		let daemon = Daemon::start().await?;
		daemon.run().await?;
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
		// Spawn a new process for the daemon rather than forking.
		// This avoids issues with tokio runtime after fork and keeps stdio working.
		let exe = std::env::current_exe().map_err(|e| PwError::Anyhow(anyhow!("Failed to get executable path: {e}")))?;

		let child = std::process::Command::new(&exe)
			.arg("daemon")
			.arg("start")
			.arg("--foreground")
			.stdin(std::process::Stdio::null())
			.stdout(std::process::Stdio::null())
			.stderr(std::process::Stdio::null())
			.spawn()
			.map_err(|e| PwError::Anyhow(anyhow!("Failed to spawn daemon: {e}")))?;

		// Write PID file.
		let pid_path = daemon_pid_path();
		if let Some(parent) = pid_path.parent() {
			let _ = std::fs::create_dir_all(parent);
		}
		std::fs::write(&pid_path, child.id().to_string())?;

		// Wait a bit for daemon to start.
		tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

		// Check if it's running.
		let running = matches!(daemon::ping().await?, Some(true));

		let result = ResultBuilder::new("daemon start")
			.data(json!({
				"started": running,
				"foreground": false,
				"pid_file": pid_path.display().to_string(),
				"pid": child.id()
			}))
			.build();
		print_result(&result, format);

		if !running {
			return Err(PwError::Anyhow(anyhow!("Daemon failed to start")));
		}

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
