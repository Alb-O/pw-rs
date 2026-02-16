//! Shared browser connect/discover helpers for CLI commands.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use pw_rs::{Playwright, StorageState, dirs};
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::debug;

use crate::context_store::ContextState;
use crate::error::{PwError, Result};
use crate::workspace::{STATE_VERSION_DIR, compute_cdp_port, ensure_state_root_gitignore};

mod wsl;

#[derive(Debug, Clone)]
struct AuthApplySummary {
	auth_file: PathBuf,
	cookies_applied: usize,
	origins_present: usize,
}

/// `/json/version` response subset from Chrome DevTools Protocol.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdpVersionInfo {
	#[serde(rename = "webSocketDebuggerUrl")]
	web_socket_debugger_url: String,
	#[serde(rename = "Browser")]
	browser: Option<String>,
}

/// Kills the browser listening on `port` and clears stored endpoint if found.
pub async fn kill_browser_on_port(ctx_state: &mut ContextState, port: u16) -> Result<Value> {
	match kill_chrome(port).await? {
		Some(pids) => {
			ctx_state.set_cdp_endpoint(None);
			Ok(json!({
				"action": "killed",
				"port": port,
				"pids": pids,
				"message": format!("Killed Chrome process(es) on port {}: {}", port, pids)
			}))
		}
		None => Ok(json!({
			"action": "kill",
			"port": port,
			"message": format!("No Chrome process found on port {}", port)
		})),
	}
}

/// Clears the stored CDP endpoint from context defaults.
pub fn clear_cdp_endpoint(ctx_state: &mut ContextState) -> Value {
	ctx_state.set_cdp_endpoint(None);
	json!({
		"action": "cleared",
		"message": "CDP endpoint cleared"
	})
}

/// Launches a browser with remote debugging and stores discovered endpoint.
pub async fn launch_and_connect(ctx_state: &mut ContextState, port: u16, user_data_dir: Option<&Path>, auth_file: Option<&Path>) -> Result<Value> {
	let launch_data_dir = resolve_user_data_dir(ctx_state, user_data_dir)?;
	let info = launch_chrome(port, Some(launch_data_dir.as_path())).await?;
	let auth_applied = maybe_apply_auth(&info.web_socket_debugger_url, auth_file).await?;
	ctx_state.set_cdp_endpoint(Some(info.web_socket_debugger_url.clone()));

	Ok(json!({
		"action": "launched",
		"endpoint": info.web_socket_debugger_url,
		"browser": info.browser,
		"port": port,
		"user_data_dir": launch_data_dir,
		"auth": auth_applied.as_ref().map(|s| json!({
			"file": s.auth_file,
			"cookiesApplied": s.cookies_applied,
			"originsPresent": s.origins_present
		})),
		"message": if let Some(summary) = &auth_applied {
			format!(
				"Chrome launched and connected on port {} (applied {} auth cookies from {})",
				port,
				summary.cookies_applied,
				summary.auth_file.display()
			)
		} else {
			format!("Chrome launched and connected on port {}", port)
		}
	}))
}

/// Discovers an existing remote-debugging browser and stores endpoint.
pub async fn discover_and_connect(ctx_state: &mut ContextState, port: u16, auth_file: Option<&Path>) -> Result<Value> {
	let info = discover_chrome(port).await?;
	let auth_applied = maybe_apply_auth(&info.web_socket_debugger_url, auth_file).await?;
	ctx_state.set_cdp_endpoint(Some(info.web_socket_debugger_url.clone()));

	Ok(json!({
		"action": "discovered",
		"endpoint": info.web_socket_debugger_url,
		"browser": info.browser,
		"port": port,
		"auth": auth_applied.as_ref().map(|s| json!({
			"file": s.auth_file,
			"cookiesApplied": s.cookies_applied,
			"originsPresent": s.origins_present
		})),
		"message": if let Some(summary) = &auth_applied {
			format!(
				"Connected to existing Chrome instance (applied {} auth cookies from {})",
				summary.cookies_applied,
				summary.auth_file.display()
			)
		} else {
			"Connected to existing Chrome instance".to_string()
		}
	}))
}

/// Stores an explicit CDP endpoint in context defaults.
pub fn set_cdp_endpoint(ctx_state: &mut ContextState, endpoint: &str) -> Value {
	ctx_state.set_cdp_endpoint(Some(endpoint.to_string()));
	json!({
		"action": "set",
		"endpoint": endpoint,
		"message": format!("CDP endpoint set to {}", endpoint)
	})
}

/// Returns current endpoint configuration payload for command output.
pub fn show_cdp_endpoint(ctx_state: &ContextState) -> Value {
	match ctx_state.cdp_endpoint() {
		Some(ep) => json!({
			"action": "show",
			"endpoint": ep,
			"message": format!("Current CDP endpoint: {}", ep)
		}),
		None => json!({
			"action": "show",
			"endpoint": null,
			"message": "No CDP endpoint configured. Use --launch or --discover to connect."
		}),
	}
}

fn load_auth_state(auth_file: &Path) -> Result<StorageState> {
	StorageState::from_file(auth_file).map_err(|e| PwError::BrowserLaunch(format!("Failed to load auth file: {}", e)))
}

async fn apply_auth_state_to_cdp(endpoint: &str, auth_file: &Path, state: StorageState) -> Result<AuthApplySummary> {
	let cookies_applied = state.cookies.len();
	let origins_present = state.origins.len();

	let playwright = Playwright::launch()
		.await
		.map_err(|e| PwError::BrowserLaunch(format!("Failed to start Playwright: {}", e)))?;
	let connected = playwright
		.chromium()
		.connect_over_cdp(endpoint)
		.await
		.map_err(|e| PwError::Context(format!("Failed to connect over CDP for auth injection: {}", e)))?;

	let context = connected
		.default_context
		.ok_or_else(|| PwError::Context("Connected browser did not expose a default context for auth injection".into()))?;

	if cookies_applied > 0 {
		context
			.add_cookies(state.cookies)
			.await
			.map_err(|e| PwError::Context(format!("Failed to inject auth cookies from {}: {}", auth_file.display(), e)))?;
	}

	Ok(AuthApplySummary {
		auth_file: auth_file.to_path_buf(),
		cookies_applied,
		origins_present,
	})
}

async fn maybe_apply_auth(endpoint: &str, auth_file: Option<&Path>) -> Result<Option<AuthApplySummary>> {
	let Some(path) = auth_file else {
		return Ok(None);
	};
	let state = load_auth_state(path)?;
	let summary = apply_auth_state_to_cdp(endpoint, path, state).await?;
	Ok(Some(summary))
}

fn find_chrome_executable() -> Option<String> {
	let candidates: Vec<String> = if cfg!(target_os = "macos") {
		vec![
			"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
			"/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
			"/Applications/Chromium.app/Contents/MacOS/Chromium",
			"/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
		]
		.into_iter()
		.map(str::to_string)
		.collect()
	} else if cfg!(target_os = "windows") {
		windows_browser_candidates()
	} else {
		vec![
			"helium",
			"brave",
			"brave-browser",
			"google-chrome-stable",
			"google-chrome",
			"chromium-browser",
			"chromium",
			"/usr/bin/helium",
			"/usr/bin/brave",
			"/usr/bin/brave-browser",
			"/usr/bin/google-chrome-stable",
			"/usr/bin/google-chrome",
			"/usr/bin/chromium-browser",
			"/usr/bin/chromium",
			"/snap/bin/chromium",
			"/snap/bin/brave",
		]
		.into_iter()
		.map(str::to_string)
		.collect()
	};

	for candidate in candidates {
		if candidate.starts_with('/') || candidate.contains('\\') || candidate.contains(':') {
			if std::path::Path::new(&candidate).exists() {
				return Some(candidate);
			}
		} else if which::which(&candidate).is_ok() {
			return Some(candidate);
		}
	}

	None
}

fn windows_browser_candidates() -> Vec<String> {
	let mut candidates = Vec::new();

	let mut roots = Vec::new();
	for key in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"] {
		if let Ok(value) = std::env::var(key) {
			roots.push(PathBuf::from(value));
		}
	}
	if roots.is_empty() {
		roots.push(PathBuf::from(r"C:\Program Files"));
		roots.push(PathBuf::from(r"C:\Program Files (x86)"));
	}

	let suffixes: &[&[&str]] = &[
		&["Google", "Chrome", "Application", "chrome.exe"],
		&["Microsoft", "Edge", "Application", "msedge.exe"],
		&["BraveSoftware", "Brave-Browser", "Application", "brave.exe"],
		&["Chromium", "Application", "chrome.exe"],
	];

	for root in roots {
		for suffix in suffixes {
			let mut path = root.clone();
			for component in *suffix {
				path.push(component);
			}
			candidates.push(path.to_string_lossy().to_string());
		}
	}

	candidates.extend([
		"chrome".to_string(),
		"chrome.exe".to_string(),
		"msedge".to_string(),
		"msedge.exe".to_string(),
		"brave".to_string(),
		"brave.exe".to_string(),
		"chromium".to_string(),
		"chromium.exe".to_string(),
	]);

	candidates
}

/// Resolves CDP version metadata from `/json/version` on `port`.
pub async fn fetch_cdp_endpoint(port: u16) -> Result<CdpVersionInfo> {
	let client = reqwest::Client::builder()
		.timeout(Duration::from_millis(400))
		.build()
		.map_err(|e| PwError::Context(format!("Failed to create HTTP client: {}", e)))?;
	let mut last_error = "no response".to_string();

	for url in [
		format!("http://127.0.0.1:{}/json/version", port),
		format!("http://localhost:{}/json/version", port),
		format!("http://[::1]:{}/json/version", port),
	] {
		let response = match client.get(&url).send().await {
			Ok(r) => r,
			Err(e) => {
				last_error = e.to_string();
				continue;
			}
		};

		if !response.status().is_success() {
			last_error = format!("unexpected status {}", response.status());
			continue;
		}

		let info: CdpVersionInfo = response
			.json()
			.await
			.map_err(|e| PwError::Context(format!("Failed to parse CDP response: {}", e)))?;
		return Ok(info);
	}

	Err(PwError::Context(format!("Failed to connect to port {}: {}", port, last_error)))
}

async fn discover_chrome(port: u16) -> Result<CdpVersionInfo> {
	let launch_hint = if cfg!(target_os = "windows") {
		format!("msedge.exe --remote-debugging-port={}", port)
	} else {
		format!("google-chrome --remote-debugging-port={}", port)
	};

	fetch_cdp_endpoint(port).await.map_err(|e| {
		PwError::Context(format!(
			"No Chrome instance with remote debugging found on port {}. \n\
	             Last error: {}\n\
	             Try running: {}\n\
	             Or use: pw connect --launch --port {}",
			port, e, launch_hint, port
		))
	})
}

async fn launch_chrome(port: u16, user_data_dir: Option<&Path>) -> Result<CdpVersionInfo> {
	if wsl::is_wsl() {
		return wsl::launch_windows_chrome_from_wsl(port, user_data_dir).await;
	}

	let chrome_path = find_chrome_executable().ok_or_else(|| {
		PwError::Context(
			"Could not find Chrome/Chromium executable. \n\
             Please install Chrome or specify path manually."
				.into(),
		)
	})?;

	let mut args = vec![
		format!("--remote-debugging-port={}", port),
		"--no-first-run".to_string(),
		"--no-default-browser-check".to_string(),
	];

	if let Some(dir) = user_data_dir {
		args.push(format!("--user-data-dir={}", dir.display()));
	}

	let mut cmd = Command::new(&chrome_path);
	cmd.args(&args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());

	#[cfg(unix)]
	std::os::unix::process::CommandExt::process_group(&mut cmd, 0);

	let mut child = cmd
		.spawn()
		.map_err(|e| PwError::Context(format!("Failed to launch Chrome at {}: {}", chrome_path, e)))?;

	let max_attempts = 8;
	let mut last_error = "endpoint not reachable".to_string();
	for _ in 0..max_attempts {
		tokio::time::sleep(Duration::from_millis(200)).await;

		if let Ok(Some(status)) = child.try_wait() {
			return Err(PwError::Context(format!(
				"Chrome exited before debugging endpoint became available (status: {}). \
	             Launch it manually with --remote-debugging-port={} and retry `pw connect --discover`.",
				status, port
			)));
		}

		match fetch_cdp_endpoint(port).await {
			Ok(info) => return Ok(info),
			Err(e) => {
				last_error = match e {
					PwError::Context(msg) => msg,
					other => other.to_string(),
				};
				continue;
			}
		}
	}

	Err(PwError::Context(format!(
		"Chrome launched but debugging endpoint not available on port {}. \n\
         Last error: {}\n\
         If Chrome/Chromium recently updated, remote debugging may require a dedicated \
         --user-data-dir. Try: pw connect --launch --user-data-dir <path>",
		port, last_error
	)))
}

/// Resolves/creates user-data-dir used for launched browser profiles.
pub fn resolve_user_data_dir(ctx_state: &ContextState, user_data_dir: Option<&Path>) -> Result<PathBuf> {
	let resolved = if wsl::is_wsl() {
		wsl::resolve_wsl_user_data_dir(ctx_state, user_data_dir)
	} else if let Some(dir) = user_data_dir {
		if dir.is_absolute() {
			dir.to_path_buf()
		} else {
			ctx_state.workspace_root().join(dir)
		}
	} else {
		let state_root = ctx_state.workspace_root().join(dirs::PLAYWRIGHT).join(STATE_VERSION_DIR);
		ensure_state_root_gitignore(&state_root)?;
		state_root.join("profiles").join(ctx_state.namespace()).join("connect-user-data")
	};

	std::fs::create_dir_all(&resolved)?;
	Ok(resolved)
}

/// Resolves effective CDP port from explicit value or namespace identity.
pub fn resolve_connect_port(ctx_state: &ContextState, requested_port: Option<u16>) -> u16 {
	requested_port.unwrap_or_else(|| compute_cdp_port(&ctx_state.namespace_id()))
}

async fn kill_chrome(port: u16) -> Result<Option<String>> {
	if fetch_cdp_endpoint(port).await.is_err() {
		return Ok(None);
	}

	#[cfg(unix)]
	{
		let output = Command::new("lsof")
			.args(["-ti", &format!(":{}", port)])
			.output()
			.map_err(|e| PwError::Context(format!("Failed to run lsof: {}", e)))?;

		if !output.status.success() || output.stdout.is_empty() {
			return Err(PwError::Context(format!("Could not find process listening on port {}", port)));
		}

		let pids: Vec<&str> = std::str::from_utf8(&output.stdout)
			.map_err(|e| PwError::Context(format!("Invalid lsof output: {}", e)))?
			.trim()
			.lines()
			.collect();

		if pids.is_empty() {
			return Err(PwError::Context(format!("No process found on port {}", port)));
		}

		let mut killed = Vec::new();
		for pid in &pids {
			debug!("Killing PID {} on port {}", pid, port);
			let kill_result = Command::new("kill").args(["-TERM", pid]).status();

			match kill_result {
				Ok(status) if status.success() => killed.push(*pid),
				Ok(_) => debug!("kill -TERM {} returned non-zero", pid),
				Err(e) => debug!("Failed to kill {}: {}", pid, e),
			}
		}

		if killed.is_empty() {
			return Err(PwError::Context(format!("Failed to kill process on port {}", port)));
		}

		Ok(Some(killed.join(", ")))
	}

	#[cfg(windows)]
	{
		let output = Command::new("netstat")
			.args(["-ano"])
			.output()
			.map_err(|e| PwError::Context(format!("Failed to run netstat: {}", e)))?;

		let output_str = String::from_utf8_lossy(&output.stdout);
		let port_str = format!(":{}", port);

		for line in output_str.lines() {
			if line.contains(&port_str) && line.contains("LISTENING") {
				let parts: Vec<&str> = line.split_whitespace().collect();
				if let Some(pid) = parts.last() {
					let kill_result = Command::new("taskkill").args(["/PID", pid, "/F"]).status();

					if kill_result.map(|s| s.success()).unwrap_or(false) {
						return Ok(Some(pid.to_string()));
					}
				}
			}
		}

		Err(PwError::Context(format!("Could not find or kill process on port {}", port)))
	}
}

#[cfg(test)]
mod tests {
	use std::fs;

	use tempfile::TempDir;

	use super::*;

	#[test]
	fn resolve_user_data_dir_defaults_to_namespace_scoped_path() {
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

		let dir = resolve_user_data_dir(&ctx_state, None).unwrap();
		if wsl::is_wsl() {
			assert_eq!(
				dir,
				std::path::PathBuf::from(wsl::WSL_MANAGED_USER_DATA_ROOT).join("workspace-id").join("agent-a"),
				"resolved path was {}",
				dir.display()
			);
		} else {
			assert!(
				dir.ends_with("playwright/.pw-cli-v4/profiles/agent-a/connect-user-data"),
				"resolved path was {}",
				dir.display()
			);
			assert!(temp.path().join("playwright").join(".pw-cli-v4").join(".gitignore").exists());
		}
	}

	#[test]
	fn resolve_user_data_dir_makes_relative_paths_workspace_relative() {
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

		let dir = resolve_user_data_dir(&ctx_state, Some(std::path::Path::new("profiles/debug")));
		let expected = temp.path().join("profiles/debug");
		assert_eq!(dir.unwrap(), expected);
	}

	#[test]
	fn load_auth_state_errors_for_missing_file() {
		let err = load_auth_state(Path::new("/definitely/missing/auth.json")).unwrap_err();
		assert!(err.to_string().contains("Failed to load auth file"));
	}

	#[test]
	fn load_auth_state_accepts_storage_state_file() {
		let temp = TempDir::new().unwrap();
		let auth = temp.path().join("auth.json");
		fs::write(
			&auth,
			r#"{
  "cookies": [
    {
      "name": "session",
      "value": "token",
      "domain": ".example.com",
      "path": "/",
      "expires": -1.0,
      "httpOnly": true,
      "secure": true,
      "sameSite": "Lax"
    }
  ],
  "origins": []
}"#,
		)
		.unwrap();

		let state = load_auth_state(&auth).unwrap();
		assert_eq!(state.cookies.len(), 1);
		assert_eq!(state.origins.len(), 0);
	}

	#[test]
	fn resolve_connect_port_uses_namespace_identity_when_unspecified() {
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

		let expected = compute_cdp_port(&ctx_state.namespace_id());
		assert_eq!(resolve_connect_port(&ctx_state, None), expected);
	}

	#[test]
	fn resolve_connect_port_prefers_explicit_port() {
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

		assert_eq!(resolve_connect_port(&ctx_state, Some(9555)), 9555);
	}

	#[test]
	fn windows_browser_candidates_include_common_commands() {
		let candidates = windows_browser_candidates();
		assert!(candidates.contains(&"chrome.exe".to_string()));
		assert!(candidates.contains(&"msedge.exe".to_string()));
		assert!(candidates.contains(&"brave.exe".to_string()));
	}
}
