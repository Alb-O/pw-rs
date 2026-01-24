//! Playwright test runner command.
//!
//! Runs Playwright tests using the bundled test runner package,
//! without requiring npm or Node.js to be installed.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use pw::pw_runtime::{self, TestRunnerPaths};

use crate::error::{PwError, Result};

/// Spawns the Playwright test runner with the given arguments.
pub fn execute(args: Vec<String>) -> Result<()> {
	let paths = pw_runtime::get_test_runner_paths().map_err(|_| {
        PwError::Init(
            "Playwright test runner not found. The test package may not have been downloaded during build.".to_string(),
        )
    })?;

	let node_modules = ensure_node_modules(&paths)?;
	let cli_js = if node_modules.join("playwright/cli.js").exists() {
		node_modules.join("playwright/cli.js")
	} else {
		paths.test_cli_js.clone()
	};

	let mut cmd = Command::new(&paths.node_exe);
	cmd.arg(&cli_js)
		.arg("test")
		.args(&args)
		.env("NODE_PATH", &node_modules);

	#[cfg(unix)]
	{
		// Replace the current process to avoid extra parent/child layers
		// that can interfere with Playwright's webServer spawning logic.
		use std::os::unix::process::CommandExt;
		let err = cmd.exec();
		Err(err.into())
	}

	#[cfg(not(unix))]
	{
		let status = cmd.status()?;
		if !status.success() {
			std::process::exit(status.code().unwrap_or(1));
		}
		Ok(())
	}
}

/// Sets up node_modules with required structure for module resolution.
///
/// Returns the path to use for `NODE_PATH`. Falls back to a cache directory
/// when the package's node_modules is read-only (e.g., Nix store).
fn ensure_node_modules(paths: &TestRunnerPaths) -> Result<PathBuf> {
	let playwright_dir = paths.test_cli_js.parent().unwrap();

	let node_modules = if is_writable(&paths.node_modules_dir) {
		fs::create_dir_all(&paths.node_modules_dir)?;
		paths.node_modules_dir.clone()
	} else {
		cache_node_modules()?
	};

	setup_node_modules(&node_modules, &paths.driver_package_dir, playwright_dir)?;
	Ok(node_modules)
}

/// Tests if a path is writable by attempting to create a temp file.
fn is_writable(path: &Path) -> bool {
	if path.exists() {
		let test_file = path.join(".pw-write-test");
		if fs::write(&test_file, b"").is_ok() {
			let _ = fs::remove_file(&test_file);
			return true;
		}
		false
	} else {
		path.parent().is_some_and(is_writable)
	}
}

/// Returns a writable cache directory for node_modules.
fn cache_node_modules() -> Result<PathBuf> {
	let cache_dir = dirs::cache_dir()
		.or_else(|| std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from))
		.unwrap_or_else(|| PathBuf::from("/tmp"))
		.join("pw/test-runner/node_modules");
	fs::create_dir_all(&cache_dir)?;
	Ok(cache_dir)
}

/// Creates the node_modules structure for Playwright module resolution.
///
/// For Nix builds (read-only source), copies packages to avoid circular dependency
/// errors caused by Node.js resolving realpaths when caching modules.
///
/// For local cargo builds where node_modules is nested inside playwright_dir,
/// uses symlinks since the playwright package is already accessible as the parent.
fn setup_node_modules(
	node_modules: &Path,
	driver_package_dir: &Path,
	playwright_dir: &Path,
) -> Result<()> {
	let playwright_dst = node_modules.join("playwright");
	let core_dst = node_modules.join("playwright-core");

	let is_nested = node_modules
		.canonicalize()
		.ok()
		.zip(playwright_dir.canonicalize().ok())
		.is_some_and(|(nm, pd)| nm.starts_with(&pd));

	if is_nested {
		link_or_copy(driver_package_dir, &core_dst)?;
		symlink_to_parent(&playwright_dst)?;
	} else {
		copy_dir_if_needed(driver_package_dir, &core_dst)?;
		copy_dir_if_needed(playwright_dir, &playwright_dst)?;
	}

	let version = read_package_version(if is_nested {
		playwright_dir
	} else {
		&playwright_dst
	})
	.unwrap_or_else(|| "0.0.0".into());
	setup_scoped_wrapper(node_modules, &version)
}

/// Creates the @playwright/test wrapper package.
fn setup_scoped_wrapper(node_modules: &Path, version: &str) -> Result<()> {
	let scoped_dir = node_modules.join("@playwright").join("test");

	if scoped_dir.is_symlink() {
		fs::remove_file(&scoped_dir)?;
	} else if scoped_dir.exists() && !scoped_dir.join("package.json").exists() {
		fs::remove_dir_all(&scoped_dir)?;
	}

	fs::create_dir_all(&scoped_dir)?;
	fs::write(
		scoped_dir.join("package.json"),
		format!(r#"{{"name":"@playwright/test","version":"{version}","main":"index.js"}}"#),
	)?;
	fs::write(
		scoped_dir.join("index.js"),
		"module.exports = require('playwright/test');",
	)?;
	Ok(())
}

/// Creates a symlink pointing to the parent directory.
fn symlink_to_parent(link: &Path) -> Result<()> {
	remove_existing(link)?;

	#[cfg(unix)]
	std::os::unix::fs::symlink("..", link)?;

	#[cfg(windows)]
	std::os::windows::fs::symlink_dir("..", link)?;

	Ok(())
}

/// Removes an existing file, symlink, or directory at the given path.
fn remove_existing(path: &Path) -> Result<()> {
	if path.is_symlink() {
		fs::remove_file(path)?;
	} else if path.exists() {
		fs::remove_dir_all(path)?;
	}
	Ok(())
}

/// Links or copies a directory, preferring symlinks with copy fallback.
fn link_or_copy(src: &Path, dst: &Path) -> Result<()> {
	remove_existing(dst)?;

	#[cfg(unix)]
	if std::os::unix::fs::symlink(src, dst).is_ok() {
		return Ok(());
	}

	#[cfg(windows)]
	if std::os::windows::fs::symlink_dir(src, dst).is_ok() {
		return Ok(());
	}

	copy_dir_recursive(src, dst)
}

/// Extracts the version field from a package.json file.
fn read_package_version(package_dir: &Path) -> Option<String> {
	let content = fs::read_to_string(package_dir.join("package.json")).ok()?;
	let version_key = content.find(r#""version""#)?;
	let after_key = &content[version_key + 9..];
	let quote_start = after_key.find('"')? + 1;
	let version_str = &after_key[quote_start..];
	let quote_end = version_str.find('"')?;
	Some(version_str[..quote_end].to_string())
}

/// Copies a directory if it doesn't exist or source has changed.
///
/// Uses a marker file to track the source path and skip redundant copies.
fn copy_dir_if_needed(src: &Path, dst: &Path) -> Result<()> {
	let marker = dst.join(".pw-source");
	let src_str = src.to_string_lossy();

	if dst.exists() {
		if fs::read_to_string(&marker).is_ok_and(|s| s == src_str) {
			return Ok(());
		}
		fs::remove_dir_all(dst)?;
	}

	copy_dir_recursive(src, dst)?;
	fs::write(&marker, src_str.as_ref())?;
	Ok(())
}

/// Recursively copies a directory, making files writable.
///
/// Nix store files are read-only; copies must be writable for later updates.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
	fs::create_dir_all(dst)?;
	for entry in fs::read_dir(src)? {
		let entry = entry?;
		let dst_path = dst.join(entry.file_name());
		if entry.file_type()?.is_dir() {
			copy_dir_recursive(&entry.path(), &dst_path)?;
		} else {
			fs::copy(entry.path(), &dst_path)?;
			#[cfg(unix)]
			{
				use std::os::unix::fs::PermissionsExt;
				if let Ok(meta) = fs::metadata(&dst_path) {
					let mut perms = meta.permissions();
					perms.set_mode(perms.mode() | 0o200);
					let _ = fs::set_permissions(&dst_path, perms);
				}
			}
		}
	}
	Ok(())
}
