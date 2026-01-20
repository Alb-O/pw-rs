//! Playwright test runner command.
//!
//! Runs Playwright tests using the bundled test runner package,
//! without requiring npm or Node.js to be installed.

use crate::error::{PwError, Result};
use pw::pw_runtime::{self, TestRunnerPaths};
use std::fs;
use std::process::{Command, Stdio};

/// Spawns the Playwright test runner with the given arguments.
pub fn execute(args: Vec<String>) -> Result<()> {
    let paths = pw_runtime::get_test_runner_paths().map_err(|_| {
        PwError::Init(
            "Playwright test runner not found. The test package may not have been downloaded during build.".to_string(),
        )
    })?;

    ensure_node_modules(&paths)?;

    let status = Command::new(&paths.node_exe)
        .arg(&paths.test_cli_js)
        .arg("test")
        .args(&args)
        .env("NODE_PATH", &paths.node_modules_dir)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

/// Sets up node_modules with required symlinks for module resolution.
///
/// Creates:
/// - `playwright-core` symlink to the bundled driver package
/// - `@playwright/test` wrapper package pointing to `playwright/test.js`
fn ensure_node_modules(paths: &TestRunnerPaths) -> Result<()> {
    let node_modules = &paths.node_modules_dir;
    let playwright_dir = paths.test_cli_js.parent().unwrap();

    fs::create_dir_all(node_modules)?;

    create_symlink_if_needed(
        &paths.driver_package_dir,
        &node_modules.join("playwright-core"),
    )?;

    // @playwright/test needs a wrapper package.json because Node resolves
    // scoped packages by their main field, not the parent package's exports
    let scoped_dir = node_modules.join("@playwright").join("test");
    fs::create_dir_all(&scoped_dir)?;

    let wrapper_package_json = scoped_dir.join("package.json");
    if !wrapper_package_json.exists() {
        let content = format!(
            r#"{{"name":"@playwright/test","version":"1.56.1","main":"{}"}}"#,
            playwright_dir.join("test.js").display()
        );
        fs::write(&wrapper_package_json, content)?;
    }

    Ok(())
}

/// Creates a symlink, replacing any existing incorrect link.
fn create_symlink_if_needed(target: &std::path::Path, link: &std::path::Path) -> Result<()> {
    if link.exists() || link.is_symlink() {
        if fs::read_link(link).ok().as_deref() == Some(target) {
            return Ok(());
        }
        if link.is_dir() && !link.is_symlink() {
            fs::remove_dir_all(link)?;
        } else {
            fs::remove_file(link)?;
        }
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link)?;

    #[cfg(windows)]
    if std::os::windows::fs::symlink_dir(target, link).is_err() {
        copy_dir_recursive(target, link)?;
    }

    Ok(())
}

#[cfg(windows)]
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path)?;
        } else {
            fs::copy(entry.path(), &dst_path)?;
        }
    }
    Ok(())
}
