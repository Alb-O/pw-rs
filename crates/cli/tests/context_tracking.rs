//! Integration tests for context URL tracking.
//!
//! These tests verify that the context store records the *actual* browser URL
//! after command execution, not just the input URL. This is critical for
//! proper context caching when clicks cause navigation or redirects occur.
//!
//! Note: Tests use --no-project with explicit --workspace to isolate state.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

/// Mutex to serialize tests that use the global context store
static CONTEXT_LOCK: Mutex<()> = Mutex::new(());

/// Helper to get the pw binary path
fn pw_binary() -> PathBuf {
	let mut path = std::env::current_exe().unwrap();
	path.pop(); // Remove test binary name
	path.pop(); // Remove deps
	path.push("pw");
	path
}

fn workspace_root() -> PathBuf {
	std::env::temp_dir().join("pw-cli-context-tracking")
}

fn context_store_path() -> PathBuf {
	workspace_root()
		.join("playwright")
		.join(".pw-cli-v3")
		.join("namespaces")
		.join("default")
		.join("cache.json")
}

fn clear_context_store() {
	let _ = std::fs::remove_dir_all(workspace_root());
}

fn read_context_store() -> Option<serde_json::Value> {
	let path = context_store_path();
	std::fs::read_to_string(&path)
		.ok()
		.and_then(|content| serde_json::from_str(&content).ok())
}

fn get_last_url_from_context() -> Option<String> {
	let store = read_context_store()?;
	store.get("lastUrl")?.as_str().map(String::from)
}

/// Helper to run pw command with --no-project and explicit workspace isolation.
fn run_pw(args: &[&str]) -> (bool, String, String) {
	let workspace = workspace_root();
	let workspace_str = workspace.to_string_lossy().to_string();
	let mut full_args = vec![
		"--no-project",
		"--workspace",
		&workspace_str,
		"--namespace",
		"default",
	];
	full_args.extend_from_slice(args);

	let output = Command::new(pw_binary())
		.args(&full_args)
		.output()
		.expect("Failed to execute pw");

	let stdout = String::from_utf8_lossy(&output.stdout).to_string();
	let stderr = String::from_utf8_lossy(&output.stderr).to_string();

	(output.status.success(), stdout, stderr)
}

/// Test: Click that changes URL via pushState should update context with new URL
///
/// This verifies that after a click triggers a URL change, the context store
/// records the *actual* post-click URL, not the original input URL.
#[test]
fn click_updates_context_with_actual_url() {
	let _lock = CONTEXT_LOCK.lock().unwrap();
	clear_context_store();

	// Page with a button that changes URL via history.pushState
	// Use simpler HTML that's URL-safe
	let html = "data:text/html,<button id=btn onclick=\"history.pushState(null,null,location.href+'?changed=1')\">Go</button>";

	// Run click command
	let (success, stdout, stderr) =
		run_pw(&["-f", "json", "click", html, "#btn", "--wait-ms", "100"]);

	assert!(success, "Click command failed: {}", stderr);
	assert!(
		stdout.contains("\"ok\": true"),
		"Expected success: {}",
		stdout
	);

	// Verify context was updated with the new URL (containing changed=1)
	let last_url = get_last_url_from_context();
	assert!(
		last_url.is_some(),
		"Expected lastUrl to be set in context store"
	);

	let last_url = last_url.unwrap();
	assert!(
		last_url.contains("changed=1"),
		"Context should store actual URL with query param, got: {}",
		last_url
	);
}

/// Test: Navigate command should store the actual browser URL
///
/// While we can't easily test HTTP redirects without a real server,
/// we can verify that the URL is stored after navigation.
#[test]
fn navigate_stores_url_in_context() {
	let _lock = CONTEXT_LOCK.lock().unwrap();
	clear_context_store();

	let url = "data:text/html,<title>Test</title><body>Hello</body>";

	let (success, _stdout, stderr) = run_pw(&["-f", "json", "navigate", url]);

	assert!(success, "Navigate command failed: {}", stderr);

	// Verify context has the URL
	let last_url = get_last_url_from_context();
	assert!(
		last_url.is_some(),
		"Expected lastUrl to be set after navigate"
	);

	let last_url = last_url.unwrap();
	assert!(
		last_url.contains("data:text/html"),
		"Context should store the navigated URL: {}",
		last_url
	);
}

/// Test: Subsequent command can use URL from context
///
/// Verifies that after a command updates context, the next command
/// can use that URL without specifying it again.
#[test]
fn subsequent_command_uses_context_url() {
	let _lock = CONTEXT_LOCK.lock().unwrap();
	clear_context_store();

	// First, navigate to set up context
	let url = "data:text/html,<h1>Title</h1>";
	let (success, _stdout, stderr) = run_pw(&["-f", "json", "navigate", url]);
	assert!(success, "First navigate failed: {}", stderr);

	// Now run text command without URL - should use context
	let (success, stdout, stderr) = run_pw(&["-f", "json", "page", "text", "-s", "h1"]);
	assert!(success, "Text command failed: {}", stderr);
	assert!(
		stdout.contains("Title"),
		"Expected to find Title in output: {}",
		stdout
	);
}
