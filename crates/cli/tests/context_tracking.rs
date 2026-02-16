//! Integration tests for context URL tracking in protocol v2.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use serde_json::json;

static CONTEXT_LOCK: Mutex<()> = Mutex::new(());

fn pw_binary() -> PathBuf {
	let mut path = std::env::current_exe().unwrap();
	path.pop();
	path.pop();
	path.push("pw");
	path
}

fn workspace_root() -> PathBuf {
	std::env::temp_dir().join("pw-cli-context-tracking")
}

fn context_store_path() -> PathBuf {
	workspace_root()
		.join("playwright")
		.join(".pw-cli-v4")
		.join("profiles")
		.join("default")
		.join("cache.json")
}

fn clear_context_store() {
	let _ = std::fs::remove_dir_all(workspace_root());
}

fn read_context_store() -> Option<serde_json::Value> {
	let path = context_store_path();
	std::fs::read_to_string(&path).ok().and_then(|content| serde_json::from_str(&content).ok())
}

fn get_last_url_from_context() -> Option<String> {
	let store = read_context_store()?;
	store.get("lastUrl")?.as_str().map(String::from)
}

fn run_exec(op: &str, input: serde_json::Value) -> (bool, serde_json::Value, String) {
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
	(output.status.success(), parsed, stderr)
}

#[test]
fn click_updates_context_with_actual_url() {
	let _lock = CONTEXT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
	clear_context_store();

	let html = "data:text/html,<button id=btn onclick=\"history.pushState(null,null,location.href+'?changed=1')\">Go</button>";
	let (success, json, stderr) = run_exec("click", json!({ "url": html, "selector": "#btn", "waitMs": 100 }));
	assert!(success, "click failed: {stderr}");
	assert_eq!(json["ok"], true);

	let last_url = get_last_url_from_context().expect("lastUrl should be set");
	assert!(last_url.contains("changed=1"), "context should store changed URL, got: {last_url}");
}

#[test]
fn navigate_stores_url_in_context() {
	let _lock = CONTEXT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
	clear_context_store();

	let url = "data:text/html,<title>Test</title><body>Hello</body>";
	let (success, _json, stderr) = run_exec("navigate", json!({ "url": url }));
	assert!(success, "navigate failed: {stderr}");

	let last_url = get_last_url_from_context().expect("lastUrl should be set");
	assert!(last_url.contains("data:text/html"), "context should store navigated URL: {last_url}");
}

#[test]
fn subsequent_command_uses_context_url() {
	let _lock = CONTEXT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
	clear_context_store();

	let url = "data:text/html,<h1>Title</h1>";
	let (success, _json, stderr) = run_exec("navigate", json!({ "url": url }));
	assert!(success, "navigate failed: {stderr}");

	let (success, json, stderr) = run_exec("page.text", json!({ "selector": "h1" }));
	assert!(success, "page.text failed: {stderr}");
	assert_eq!(json["data"]["text"], "Title");
}
