//! Playwright driver management
//!
//! Handles locating and managing the Playwright Node.js driver.
//! Follows the same architecture as playwright-python, playwright-java, and playwright-dotnet.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tracing::warn;

use crate::error::{Error, Result};

/// Get the path to the Playwright driver executable
///
/// This function attempts to locate the Playwright driver in the following order:
/// 1. PLAYWRIGHT_NODE_EXE and PLAYWRIGHT_CLI_JS environment variables (runtime override)
/// 2. PLAYWRIGHT_DRIVER_PATH environment variable (runtime override)
/// 3. Bundled driver downloaded by build.rs (matches official bindings)
/// 4. Global npm installation (`npm root -g`) (development fallback)
/// 5. Local npm installation (`npm root`) (development fallback)
///
/// Runtime environment variables take precedence over the bundled driver to support
/// environments like NixOS where the bundled driver's dynamically-linked node binary
/// won't work.
///
/// Returns a tuple of (node_executable_path, cli_js_path).
///
/// # Errors
///
/// Returns `Error::ServerNotFound` if the driver cannot be located in any of the search paths.
pub fn get_driver_executable() -> Result<(PathBuf, PathBuf)> {
	// 1. Try PLAYWRIGHT_NODE_EXE and PLAYWRIGHT_CLI_JS environment variables (runtime override)
	if let Some((node, cli)) = try_node_cli_env()? {
		if let Some(paths) = resolve_candidate_with_fallback(
			"PLAYWRIGHT_NODE_EXE/PLAYWRIGHT_CLI_JS",
			node,
			cli,
			find_node_executable,
		) {
			return Ok(paths);
		}
	}

	// 2. Try PLAYWRIGHT_DRIVER_PATH environment variable (runtime override)
	if let Some((node, cli)) = try_driver_path_env()? {
		if let Some(paths) = resolve_candidate_with_fallback(
			"PLAYWRIGHT_DRIVER_PATH",
			node,
			cli,
			find_node_executable,
		) {
			return Ok(paths);
		}
	}

	// 3. Try bundled driver from build.rs (matches official bindings)
	if let Some((node, cli)) = try_bundled_driver()? {
		if let Some(paths) =
			resolve_candidate_with_fallback("bundled driver", node, cli, find_node_executable)
		{
			return Ok(paths);
		}
	}

	// 4. Try npm global installation (development fallback)
	if let Some((node, cli)) = try_npm_global()? {
		if let Some(paths) =
			resolve_candidate_with_fallback("npm global", node, cli, find_node_executable)
		{
			return Ok(paths);
		}
	}

	// 5. Try npm local installation (development fallback)
	if let Some((node, cli)) = try_npm_local()? {
		if let Some(paths) =
			resolve_candidate_with_fallback("npm local", node, cli, find_node_executable)
		{
			return Ok(paths);
		}
	}

	Err(Error::ServerNotFound)
}

fn resolve_candidate_with_fallback<F>(
	label: &str,
	node: PathBuf,
	cli: PathBuf,
	find_node: F,
) -> Option<(PathBuf, PathBuf)>
where
	F: Fn() -> Result<PathBuf>,
{
	let usable = node_is_usable(&node);
	debug_candidate(label, &node, &cli, usable);
	if usable {
		return Some((node, cli));
	}

	warn!(
		target = "pw",
		source = label,
		node = %node.display(),
		cli = %cli.display(),
		"Playwright driver candidate node is not runnable; trying fallback node"
	);

	let fallback_node = find_node().ok()?;
	if fallback_node == node {
		return None;
	}

	let fallback_usable = node_is_usable(&fallback_node);
	let fallback_label = format!("{label} (fallback node)");
	debug_candidate(&fallback_label, &fallback_node, &cli, fallback_usable);
	if fallback_usable {
		warn!(
			target = "pw",
			source = label,
			node = %fallback_node.display(),
			cli = %cli.display(),
			"Using fallback node executable for Playwright CLI"
		);
		return Some((fallback_node, cli));
	}

	None
}

/// Try to find bundled driver from build.rs
fn try_bundled_driver() -> Result<Option<(PathBuf, PathBuf)>> {
	// Check if build.rs set the environment variables (compile-time)
	if let (Some(node_exe), Some(cli_js)) = (
		option_env!("PLAYWRIGHT_BUNDLED_NODE_EXE"),
		option_env!("PLAYWRIGHT_BUNDLED_CLI_JS"),
	) {
		let node_path = PathBuf::from(node_exe);
		let cli_path = PathBuf::from(cli_js);

		if node_path.exists() && cli_path.exists() {
			return Ok(Some((node_path, cli_path)));
		}
	}

	// Fallback: Check PLAYWRIGHT_DRIVER_DIR and construct paths (compile-time)
	if let Some(driver_dir) = option_env!("PLAYWRIGHT_DRIVER_DIR") {
		let driver_path = PathBuf::from(driver_dir);
		let node_exe = if cfg!(windows) {
			driver_path.join("node.exe")
		} else {
			driver_path.join("node")
		};
		let cli_js = driver_path.join("package").join("cli.js");

		if node_exe.exists() && cli_js.exists() {
			return Ok(Some((node_exe, cli_js)));
		}
	}

	Ok(None)
}

/// Try to find driver from PLAYWRIGHT_DRIVER_PATH environment variable
fn try_driver_path_env() -> Result<Option<(PathBuf, PathBuf)>> {
	if let Ok(driver_path) = std::env::var("PLAYWRIGHT_DRIVER_PATH") {
		let driver_dir = PathBuf::from(driver_path);
		let node_exe = if cfg!(windows) {
			driver_dir.join("node.exe")
		} else {
			driver_dir.join("node")
		};
		let cli_js = driver_dir.join("package").join("cli.js");

		if node_exe.exists() && cli_js.exists() {
			return Ok(Some((node_exe, cli_js)));
		}
	}

	Ok(None)
}

/// Try to find driver from PLAYWRIGHT_NODE_EXE and PLAYWRIGHT_CLI_JS environment variables
fn try_node_cli_env() -> Result<Option<(PathBuf, PathBuf)>> {
	if let (Ok(node_exe), Ok(cli_js)) = (
		std::env::var("PLAYWRIGHT_NODE_EXE"),
		std::env::var("PLAYWRIGHT_CLI_JS"),
	) {
		let node_path = PathBuf::from(node_exe);
		let cli_path = PathBuf::from(cli_js);

		if node_path.exists() && cli_path.exists() {
			return Ok(Some((node_path, cli_path)));
		}
	}

	Ok(None)
}

/// Try to find driver in npm global installation (development fallback)
fn try_npm_global() -> Result<Option<(PathBuf, PathBuf)>> {
	let output = Command::new("npm").args(["root", "-g"]).output();

	if let Ok(output) = output {
		if output.status.success() {
			let npm_root = String::from_utf8_lossy(&output.stdout).trim().to_string();
			let node_modules = PathBuf::from(npm_root);
			if node_modules.exists() {
				if let Ok(paths) = find_playwright_in_node_modules(&node_modules) {
					return Ok(Some(paths));
				}
			}
		}
	}

	Ok(None)
}

/// Try to find driver in npm local installation (development fallback)
fn try_npm_local() -> Result<Option<(PathBuf, PathBuf)>> {
	let output = Command::new("npm").args(["root"]).output();

	if let Ok(output) = output {
		if output.status.success() {
			let npm_root = String::from_utf8_lossy(&output.stdout).trim().to_string();
			let node_modules = PathBuf::from(npm_root);
			if node_modules.exists() {
				if let Ok(paths) = find_playwright_in_node_modules(&node_modules) {
					return Ok(Some(paths));
				}
			}
		}
	}

	Ok(None)
}

fn node_is_usable(node: &Path) -> bool {
	Command::new(node)
		.arg("--version")
		.stdout(Stdio::null())
		.stderr(Stdio::null())
		.status()
		.map(|status| status.success())
		.unwrap_or(false)
}

fn debug_candidate(label: &str, node: &Path, cli: &Path, usable: bool) {
	if std::env::var("PW_DEBUG_DRIVER").is_ok() {
		eprintln!(
			"[driver-check] {label}: node={} cli={} usable={}",
			node.display(),
			cli.display(),
			usable
		);
	}
}

/// Find Playwright CLI in node_modules directory
fn find_playwright_in_node_modules(node_modules: &Path) -> Result<(PathBuf, PathBuf)> {
	let playwright_dirs = [
		node_modules.join("playwright"),
		node_modules.join("@playwright").join("test"),
	];

	for playwright_dir in &playwright_dirs {
		if !playwright_dir.exists() {
			continue;
		}

		let cli_js = playwright_dir.join("cli.js");
		if !cli_js.exists() {
			continue;
		}

		if let Ok(node_exe) = find_node_executable() {
			return Ok((node_exe, cli_js));
		}
	}

	Err(Error::ServerNotFound)
}

/// Paths needed for the Playwright test runner.
pub struct TestRunnerPaths {
	/// Node.js executable.
	pub node_exe: PathBuf,
	/// Test runner CLI entry point (`cli.js`).
	pub test_cli_js: PathBuf,
	/// Directory for module symlinks (`node_modules/`).
	pub node_modules_dir: PathBuf,
	/// Driver package directory (symlink target for `playwright-core`).
	pub driver_package_dir: PathBuf,
}

/// Returns paths needed for the Playwright test runner.
///
/// Checks in order:
/// 1. `PLAYWRIGHT_TEST_CLI_JS` env var (Nix wrapper sets this explicitly)
/// 2. `PLAYWRIGHT_CLI_JS` env var if it has test.js (full playwright package)
/// 3. Build-time `PLAYWRIGHT_TEST_DIR` (cargo build downloads playwright npm package)
///
/// # Errors
///
/// Returns [`Error::ServerNotFound`] if no test runner is available.
pub fn get_test_runner_paths() -> Result<TestRunnerPaths> {
	let (node_exe, driver_cli_js) = get_driver_executable()?;

	let test_dir = if let Ok(test_cli) = std::env::var("PLAYWRIGHT_TEST_CLI_JS") {
		PathBuf::from(test_cli)
			.parent()
			.ok_or(Error::ServerNotFound)?
			.to_path_buf()
	} else if let Some(dir) = env_cli_js_if_has_test() {
		dir
	} else if let Some(test_dir) = option_env!("PLAYWRIGHT_TEST_DIR") {
		PathBuf::from(test_dir)
	} else {
		return Err(Error::ServerNotFound);
	};

	let test_cli_js = test_dir.join("cli.js");
	if !test_cli_js.exists() {
		return Err(Error::ServerNotFound);
	}

	let driver_package_dir = driver_cli_js
		.parent()
		.map(|p| p.to_path_buf())
		.unwrap_or_else(|| test_dir.clone());

	Ok(TestRunnerPaths {
		node_exe,
		test_cli_js,
		node_modules_dir: test_dir.join("node_modules"),
		driver_package_dir,
	})
}

/// Returns the directory from PLAYWRIGHT_CLI_JS if it contains test.js.
fn env_cli_js_if_has_test() -> Option<PathBuf> {
	let cli_js = std::env::var("PLAYWRIGHT_CLI_JS").ok()?;
	let dir = PathBuf::from(cli_js).parent()?.to_path_buf();
	dir.join("test.js").exists().then_some(dir)
}

/// Find the node executable in PATH or common locations
fn find_node_executable() -> Result<PathBuf> {
	#[cfg(not(windows))]
	let which_cmd = "which";
	#[cfg(windows)]
	let which_cmd = "where";

	if let Ok(output) = Command::new(which_cmd).arg("node").output() {
		if output.status.success() {
			let node_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
			if !node_path.is_empty() {
				let path = PathBuf::from(node_path.lines().next().unwrap_or(&node_path));
				if path.exists() {
					return Ok(path);
				}
			}
		}
	}

	#[cfg(not(windows))]
	let common_locations = [
		"/usr/local/bin/node",
		"/usr/bin/node",
		"/opt/homebrew/bin/node",
		"/opt/local/bin/node",
	];

	#[cfg(windows)]
	let common_locations = [
		"C:\\Program Files\\nodejs\\node.exe",
		"C:\\Program Files (x86)\\nodejs\\node.exe",
	];

	for location in &common_locations {
		let path = PathBuf::from(location);
		if path.exists() {
			return Ok(path);
		}
	}

	Err(Error::LaunchFailed(
		"Node.js executable not found. Please install Node.js or set PLAYWRIGHT_NODE_EXE."
			.to_string(),
	))
}

#[cfg(test)]
mod tests {
	use std::fs;
	#[cfg(unix)]
	use std::os::unix::fs::PermissionsExt;
	use std::path::Path;

	use tempfile::TempDir;

	use super::*;

	#[cfg(unix)]
	fn write_mock_node(path: &Path, exit_code: i32) {
		let script = format!(
			"#!/bin/sh\n[ \"$1\" = \"--version\" ]\nexit {}\n",
			exit_code
		);
		fs::write(path, script).unwrap();
		let mut perms = fs::metadata(path).unwrap().permissions();
		perms.set_mode(0o755);
		fs::set_permissions(path, perms).unwrap();
	}

	#[test]
	fn test_find_node_executable() {
		let result = find_node_executable();
		match result {
			Ok(node_path) => {
				println!("Found node at: {:?}", node_path);
				assert!(node_path.exists());
			}
			Err(e) => {
				println!(
					"Node.js not found (expected if Node.js not installed): {:?}",
					e
				);
			}
		}
	}

	#[test]
	fn test_get_driver_executable() {
		let result = get_driver_executable();
		match result {
			Ok((node, cli)) => {
				println!("Found Playwright driver:");
				println!("  Node: {:?}", node);
				println!("  CLI:  {:?}", cli);
				assert!(node.exists());
				assert!(cli.exists());
			}
			Err(Error::ServerNotFound) => {
				println!("Playwright driver not found (expected in some environments)");
			}
			Err(e) => panic!("Unexpected error: {:?}", e),
		}
	}

	#[test]
	fn test_bundled_driver_detection() {
		let result = try_bundled_driver();
		match result {
			Ok(Some((node, cli))) => {
				println!("Found bundled driver:");
				println!("  Node: {:?}", node);
				println!("  CLI:  {:?}", cli);
				assert!(node.exists());
				assert!(cli.exists());
			}
			Ok(None) => {
				println!("No bundled driver (expected during development)");
			}
			Err(e) => panic!("Unexpected error: {:?}", e),
		}
	}

	#[cfg(unix)]
	#[test]
	fn test_resolve_candidate_falls_back_to_second_node() {
		let temp = TempDir::new().unwrap();
		let candidate_node = temp.path().join("candidate-node");
		let fallback_node = temp.path().join("fallback-node");
		let cli_js = temp.path().join("cli.js");

		write_mock_node(&candidate_node, 1);
		write_mock_node(&fallback_node, 0);
		fs::write(&cli_js, "// test cli").unwrap();

		let resolved =
			resolve_candidate_with_fallback("test", candidate_node.clone(), cli_js.clone(), || {
				Ok(fallback_node.clone())
			});

		assert_eq!(resolved, Some((fallback_node, cli_js)));
	}

	#[cfg(unix)]
	#[test]
	fn test_resolve_candidate_keeps_first_node_when_usable() {
		let temp = TempDir::new().unwrap();
		let candidate_node = temp.path().join("candidate-node");
		let cli_js = temp.path().join("cli.js");

		write_mock_node(&candidate_node, 0);
		fs::write(&cli_js, "// test cli").unwrap();

		let resolved =
			resolve_candidate_with_fallback("test", candidate_node.clone(), cli_js.clone(), || {
				panic!("fallback should not be consulted when candidate node is usable");
			});

		assert_eq!(resolved, Some((candidate_node, cli_js)));
	}

	#[cfg(unix)]
	#[test]
	fn test_resolve_candidate_returns_none_when_fallback_unavailable() {
		let temp = TempDir::new().unwrap();
		let candidate_node = temp.path().join("candidate-node");
		let cli_js = temp.path().join("cli.js");

		write_mock_node(&candidate_node, 1);
		fs::write(&cli_js, "// test cli").unwrap();

		let resolved = resolve_candidate_with_fallback("test", candidate_node, cli_js, || {
			Err(Error::LaunchFailed("missing node".to_string()))
		});

		assert!(resolved.is_none());
	}
}
