//! Integration tests for helpful error messages in protocol v2.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use serde_json::json;

static CONTEXT_LOCK: Mutex<()> = Mutex::new(());

fn lock_context() -> std::sync::MutexGuard<'static, ()> {
	CONTEXT_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn pw_binary() -> PathBuf {
	let mut path = std::env::current_exe().unwrap();
	path.pop();
	path.pop();
	path.push("pw");
	path
}

fn workspace_root() -> PathBuf {
	std::env::temp_dir().join("pw-cli-error-messages")
}

fn clear_context_store() {
	let _ = std::fs::remove_dir_all(workspace_root());
}

fn run_exec(op: &str, input: serde_json::Value) -> (serde_json::Value, String) {
	let workspace = workspace_root();
	let _ = std::fs::create_dir_all(&workspace);
	let output = Command::new(pw_binary())
		.current_dir(&workspace)
		.args(["-f", "json", "exec", op, "--input"])
		.arg(input.to_string())
		.output()
		.expect("failed to execute pw");

	let stdout = String::from_utf8_lossy(&output.stdout).to_string();
	let stderr = String::from_utf8_lossy(&output.stderr).to_string();
	let parsed = serde_json::from_str::<serde_json::Value>(&stdout).unwrap_or_else(|_| json!({ "raw": stdout }));
	(parsed, stderr)
}

#[test]
fn error_for_selector_like_url_mentions_url_problem() {
	let _lock = lock_context();
	clear_context_store();

	let (json, _stderr) = run_exec("page.text", json!({ "urlFlag": ".class-name" }));
	assert_eq!(json["ok"], false, "expected command-level failure with selector-like URL");
	let msg = json["error"]["message"].as_str().unwrap_or_default().to_lowercase();
	assert!(
		msg.contains("url") || msg.contains("base") || msg.contains("invalid"),
		"expected URL-related error message, got: {msg}"
	);
}

#[test]
fn error_mentions_context_when_no_url_available() {
	let _lock = lock_context();
	clear_context_store();

	let (json, _stderr) = run_exec("page.text", json!({ "selector": "body" }));
	assert_eq!(json["ok"], false, "expected command-level failure without URL/context");
	let msg = json["error"]["message"].as_str().unwrap_or_default().to_lowercase();
	assert!(
		msg.contains("navigate") || msg.contains("context") || msg.contains("url"),
		"expected context-related message, got: {msg}"
	);
}

#[test]
fn error_for_unknown_operation_is_clear() {
	let _lock = lock_context();
	clear_context_store();

	let workspace = workspace_root();
	let _ = std::fs::create_dir_all(&workspace);
	let output = Command::new(pw_binary())
		.current_dir(&workspace)
		.args(["-f", "json", "exec", "nav", "--input", "{}"])
		.output()
		.expect("failed to execute pw");

	let stdout = String::from_utf8_lossy(&output.stdout);
	let json: serde_json::Value = serde_json::from_str(&stdout).expect("expected JSON output");
	assert_eq!(json["ok"], false, "expected command-level failure for alias operation");
	let msg = json["error"]["message"].as_str().unwrap_or_default().to_lowercase();
	assert!(msg.contains("unknown operation"), "expected unknown operation message, got: {msg}");
}
