use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use tracing::debug;

use super::{CdpVersionInfo, fetch_cdp_endpoint};
use crate::context_store::ContextState;
use crate::error::{PwError, Result};

const WSL_POWERSHELL_PATH: &str = "/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe";
pub(super) const WSL_MANAGED_USER_DATA_ROOT: &str = "/mnt/c/temp/pw-cli/connect-user-data";

pub(super) fn is_wsl() -> bool {
	let osrelease = std::fs::read_to_string("/proc/sys/kernel/osrelease").ok();
	let wsl_distro = std::env::var("WSL_DISTRO_NAME").ok();
	is_wsl_with_inputs(osrelease.as_deref(), wsl_distro.as_deref())
}

fn is_wsl_with_inputs(osrelease: Option<&str>, wsl_distro: Option<&str>) -> bool {
	if wsl_distro.is_some() {
		return true;
	}
	osrelease
		.map(str::to_ascii_lowercase)
		.is_some_and(|value| value.contains("microsoft") || value.contains("wsl"))
}

pub(super) fn resolve_wsl_user_data_dir(ctx_state: &ContextState, user_data_dir: Option<&Path>) -> PathBuf {
	if let Some(dir) = user_data_dir {
		if let Some(raw) = dir.to_str() {
			if let Some(converted) = windows_path_to_wsl_mount(raw) {
				return converted;
			}
		}
		if dir.is_absolute() {
			return dir.to_path_buf();
		}
		return ctx_state.workspace_root().join(dir);
	}

	PathBuf::from(WSL_MANAGED_USER_DATA_ROOT)
		.join(ctx_state.workspace_id())
		.join(ctx_state.namespace())
}

fn windows_path_to_wsl_mount(path: &str) -> Option<PathBuf> {
	let bytes = path.as_bytes();
	if bytes.len() < 3 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' || (bytes[2] != b'\\' && bytes[2] != b'/') {
		return None;
	}

	let drive = (bytes[0] as char).to_ascii_lowercase();
	let rest = path[3..].replace('\\', "/");
	let mut converted = format!("/mnt/{drive}");
	let trimmed = rest.trim_start_matches('/');
	if !trimmed.is_empty() {
		converted.push('/');
		converted.push_str(trimmed);
	}

	Some(PathBuf::from(converted))
}

fn wsl_mount_path_to_windows(path: &Path) -> Option<String> {
	let path = path.to_str()?;
	let mut parts = path.split('/');
	if !parts.next()?.is_empty() || parts.next()? != "mnt" {
		return None;
	}
	let drive = parts.next()?;
	if drive.len() != 1 || !drive.as_bytes()[0].is_ascii_alphabetic() {
		return None;
	}

	let mut windows_path = format!("{}:\\", drive.to_ascii_uppercase());
	let rest: Vec<&str> = parts.collect();
	if !rest.is_empty() {
		windows_path.push_str(&rest.join("\\"));
	}
	Some(windows_path)
}

fn wsl_unc_path(path: &Path, distro_name: Option<&str>) -> Option<String> {
	let distro = distro_name?;
	let normalized = path.to_str()?.replace('/', "\\");
	let trimmed = normalized.trim_start_matches('\\');
	Some(format!(r"\\wsl.localhost\{distro}\{trimmed}"))
}

fn wsl_path_to_windows(path: &Path) -> Option<String> {
	wsl_mount_path_to_windows(path).or_else(|| wsl_unc_path(path, std::env::var("WSL_DISTRO_NAME").ok().as_deref()))
}

fn find_powershell_executable_wsl() -> Option<String> {
	if Path::new(WSL_POWERSHELL_PATH).exists() {
		return Some(WSL_POWERSHELL_PATH.to_string());
	}

	which::which("powershell.exe").ok().and_then(|path| path.to_str().map(ToOwned::to_owned))
}

fn find_windows_chrome_executable_wsl() -> Option<String> {
	let candidates = [
		(
			"/mnt/c/Program Files/Google/Chrome/Application/chrome.exe",
			r"C:\Program Files\Google\Chrome\Application\chrome.exe",
		),
		(
			"/mnt/c/Program Files (x86)/Google/Chrome/Application/chrome.exe",
			r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
		),
		(
			"/mnt/c/Program Files/Microsoft/Edge/Application/msedge.exe",
			r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
		),
		(
			"/mnt/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
			r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
		),
		(
			"/mnt/c/Program Files/BraveSoftware/Brave-Browser/Application/brave.exe",
			r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
		),
		(
			"/mnt/c/Program Files (x86)/BraveSoftware/Brave-Browser/Application/brave.exe",
			r"C:\Program Files (x86)\BraveSoftware\Brave-Browser\Application\brave.exe",
		),
	];

	for (wsl_path, win_path) in candidates {
		if Path::new(wsl_path).exists() {
			return Some(win_path.to_string());
		}
	}

	None
}

fn ps_single_quote(value: &str) -> String {
	format!("'{}'", value.replace('\'', "''"))
}

pub(super) async fn launch_windows_chrome_from_wsl(port: u16, user_data_dir: Option<&Path>) -> Result<CdpVersionInfo> {
	let chrome_path = find_windows_chrome_executable_wsl().ok_or_else(|| {
		PwError::Context(
			"Could not find a Windows Chromium browser from WSL (Chrome/Edge/Brave). \
             Install one on Windows or connect manually with `pw connect ws://...`."
				.into(),
		)
	})?;

	let powershell = find_powershell_executable_wsl().ok_or_else(|| {
		PwError::Context(
			"Could not locate powershell.exe from WSL. \
             Ensure `/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe` exists."
				.into(),
		)
	})?;

	let mut args = vec![
		format!("--remote-debugging-port={port}"),
		"--no-first-run".to_string(),
		"--no-default-browser-check".to_string(),
	];

	if let Some(dir) = user_data_dir {
		let win_dir =
			wsl_path_to_windows(dir).ok_or_else(|| PwError::Context(format!("Failed to convert WSL user-data-dir `{}` to a Windows path", dir.display())))?;
		if win_dir.starts_with(r"\\wsl.localhost\") {
			debug!(
				target = "pw",
				user_data_dir = %win_dir,
				"using WSL UNC profile path; Chromium may show a warning dialog"
			);
		}
		args.push(format!("--user-data-dir={win_dir}"));
	}

	let argument_list = format!("@({})", args.iter().map(|arg| ps_single_quote(arg)).collect::<Vec<_>>().join(", "));
	let script = format!(
		"Start-Process -FilePath {} -ArgumentList {} | Out-Null",
		ps_single_quote(&chrome_path),
		argument_list
	);

	let status = Command::new(&powershell)
		.args(["-NoProfile", "-Command", &script])
		.stdin(Stdio::null())
		.stdout(Stdio::null())
		.stderr(Stdio::null())
		.status()
		.map_err(|e| PwError::Context(format!("Failed to launch Windows browser via PowerShell: {}", e)))?;

	if !status.success() {
		return Err(PwError::Context(format!("PowerShell failed to launch Windows browser (exit code: {})", status)));
	}

	let max_attempts = 12;
	let mut last_error = "endpoint not reachable".to_string();
	for _ in 0..max_attempts {
		tokio::time::sleep(Duration::from_millis(250)).await;
		match fetch_cdp_endpoint(port).await {
			Ok(info) => return Ok(info),
			Err(e) => {
				last_error = match e {
					PwError::Context(msg) => msg,
					other => other.to_string(),
				};
			}
		}
	}

	Err(PwError::Context(format!(
		"Windows browser launched from WSL but debugging endpoint not available on port {}. \
         Last error: {}",
		port, last_error
	)))
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	use tempfile::TempDir;

	use super::*;

	#[test]
	fn is_wsl_detection_works_for_env_or_osrelease() {
		assert!(is_wsl_with_inputs(None, Some("Ubuntu")));
		assert!(is_wsl_with_inputs(Some("6.6.87.2-microsoft-standard-WSL2"), None));
		assert!(!is_wsl_with_inputs(Some("6.8.0-generic"), None));
	}

	#[test]
	fn windows_path_to_wsl_mount_converts_drive_paths() {
		let converted = windows_path_to_wsl_mount(r"C:\temp\pw-profile").unwrap();
		assert_eq!(converted, PathBuf::from("/mnt/c/temp/pw-profile"));
	}

	#[test]
	fn wsl_mount_path_to_windows_converts_paths() {
		let converted = wsl_mount_path_to_windows(Path::new("/mnt/d/work/profile")).unwrap();
		assert_eq!(converted, r"D:\work\profile");
	}

	#[test]
	fn wsl_unc_path_uses_distro_name() {
		let converted = wsl_unc_path(Path::new("/home/albert/profile"), Some("NixOS")).unwrap();
		assert_eq!(converted, r"\\wsl.localhost\NixOS\home\albert\profile");
	}

	#[test]
	fn resolve_wsl_user_data_dir_converts_windows_input() {
		let temp = TempDir::new().unwrap();
		let ctx_state = ContextState::new(
			temp.path().to_path_buf(),
			"workspace-id".to_string(),
			"default".to_string(),
			None,
			false,
			true,
			false,
		)
		.unwrap();
		let resolved = resolve_wsl_user_data_dir(&ctx_state, Some(Path::new(r"C:\temp\profile")));
		assert_eq!(resolved, PathBuf::from("/mnt/c/temp/profile"));
	}

	#[test]
	fn resolve_wsl_user_data_dir_defaults_to_managed_path() {
		let temp = TempDir::new().unwrap();
		let ctx_state = ContextState::new(
			temp.path().to_path_buf(),
			"workspace-id".to_string(),
			"agent-a".to_string(),
			None,
			false,
			true,
			false,
		)
		.unwrap();
		let resolved = resolve_wsl_user_data_dir(&ctx_state, None);
		assert_eq!(resolved, PathBuf::from(WSL_MANAGED_USER_DATA_ROOT).join("workspace-id").join("agent-a"));
	}
}
