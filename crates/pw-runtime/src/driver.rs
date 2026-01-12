//! Playwright driver management
//!
//! Handles locating and managing the Playwright Node.js driver.
//! Follows the same architecture as playwright-python, playwright-java, and playwright-dotnet.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tracing::warn;

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
        let usable = node_is_usable(&node);
        debug_candidate("env node/cli", &node, &cli, usable);
        if usable {
            return Ok((node, cli));
        }
        warn!(
            target = "pw",
            node = %node.display(),
            cli = %cli.display(),
            "PLAYWRIGHT_NODE_EXE is set but node is not runnable; falling back"
        );
    }

    // 2. Try PLAYWRIGHT_DRIVER_PATH environment variable (runtime override)
    if let Some((node, cli)) = try_driver_path_env()? {
        let usable = node_is_usable(&node);
        debug_candidate("PLAYWRIGHT_DRIVER_PATH", &node, &cli, usable);
        if usable {
            return Ok((node, cli));
        }
        warn!(
            target = "pw",
            node = %node.display(),
            cli = %cli.display(),
            "PLAYWRIGHT_DRIVER_PATH is set but node is not runnable; falling back"
        );
    }

    // 3. Try bundled driver from build.rs (matches official bindings)
    if let Some((node, cli)) = try_bundled_driver()? {
        let usable = node_is_usable(&node);
        debug_candidate("bundled driver", &node, &cli, usable);
        if usable {
            return Ok((node, cli));
        }
        warn!(
            target = "pw",
            node = %node.display(),
            cli = %cli.display(),
            "Bundled Playwright driver not runnable; falling back"
        );
    }

    // 4. Try npm global installation (development fallback)
    if let Some((node, cli)) = try_npm_global()? {
        let usable = node_is_usable(&node);
        debug_candidate("npm global", &node, &cli, usable);
        if usable {
            return Ok((node, cli));
        }
        warn!(
            target = "pw",
            node = %node.display(),
            cli = %cli.display(),
            "Global npm Playwright driver not runnable; falling back"
        );
    }

    // 5. Try npm local installation (development fallback)
    if let Some((node, cli)) = try_npm_local()? {
        let usable = node_is_usable(&node);
        debug_candidate("npm local", &node, &cli, usable);
        if usable {
            return Ok((node, cli));
        }
        warn!(
            target = "pw",
            node = %node.display(),
            cli = %cli.display(),
            "Local npm Playwright driver not runnable; falling back"
        );
    }

    Err(Error::ServerNotFound)
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
    use super::*;

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
}
