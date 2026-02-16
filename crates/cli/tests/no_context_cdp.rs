//! Integration tests for profile-scoped context behavior in protocol v2.

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
	std::env::temp_dir().join("pw-cli-no-context-cdp")
}

fn clear_context_store() {
	let _ = std::fs::remove_dir_all(workspace_root());
}

fn run_exec_with_profile(op: &str, input: serde_json::Value, profile: &str) -> (bool, serde_json::Value, String) {
	let workspace = workspace_root();
	let _ = std::fs::create_dir_all(&workspace);

	let request_path = workspace.join("request.json");
	let request = json!({
		"schemaVersion": 5,
		"op": op,
		"input": input,
		"runtime": {
			"profile": profile,
			"overrides": {
				"useDaemon": false
			}
		}
	});
	std::fs::write(&request_path, serde_json::to_string(&request).unwrap()).expect("failed to write request file");

	let output = Command::new(pw_binary())
		.current_dir(&workspace)
		.args(["-f", "json", "exec", "--file"])
		.arg(request_path)
		.output()
		.expect("failed to execute pw");

	let stdout = String::from_utf8_lossy(&output.stdout).to_string();
	let stderr = String::from_utf8_lossy(&output.stderr).to_string();
	let parsed = serde_json::from_str::<serde_json::Value>(&stdout).unwrap_or_else(|_| json!({ "raw": stdout }));
	(output.status.success(), parsed, stderr)
}

fn run_exec(op: &str, input: serde_json::Value) -> (bool, serde_json::Value, String) {
	run_exec_with_profile(op, input, "default")
}

#[test]
fn no_cached_url_requires_explicit_url() {
	let _lock = lock_context();
	clear_context_store();

	let (success, json, _stderr) = run_exec("page.text", json!({ "selector": "body" }));
	assert!(success, "exec transport should succeed");
	assert_eq!(json["ok"], false, "expected protocol error without context URL");
	let msg = json["error"]["message"].as_str().unwrap_or_default().to_lowercase();
	assert!(msg.contains("url"), "expected URL-related message, got: {msg}");
}

#[test]
fn cached_url_allows_omitted_url() {
	let _lock = lock_context();
	clear_context_store();

	let url = "data:text/html,<h1>Cached Test</h1>";
	let (success, _json, stderr) = run_exec("navigate", json!({ "url": url }));
	assert!(success, "navigate failed: {stderr}");

	let (success, json, stderr) = run_exec("page.text", json!({ "selector": "h1" }));
	assert!(success, "page.text failed: {stderr}");
	assert_eq!(json["data"]["text"], "Cached Test");
}

#[test]
fn profile_isolation_prevents_cross_profile_context_reuse() {
	let _lock = lock_context();
	clear_context_store();

	let url = "data:text/html,<h1>Default Profile</h1>";
	let (success, _json, stderr) = run_exec_with_profile("navigate", json!({ "url": url }), "default");
	assert!(success, "default profile navigate failed: {stderr}");

	let (success, json, _stderr) = run_exec_with_profile("page.text", json!({ "selector": "h1" }), "other");
	assert!(success, "exec transport should succeed");
	assert_eq!(json["ok"], false, "expected profile-isolated URL resolution failure");
	let msg = json["error"]["message"].as_str().unwrap_or_default().to_lowercase();
	assert!(msg.contains("url"), "expected URL-related error in isolated profile, got: {msg}");
}

#[test]
fn explicit_url_works_without_cached_context() {
	let _lock = lock_context();
	clear_context_store();

	let url = "data:text/html,<p>Explicit URL Test</p>";
	let (success, json, stderr) = run_exec("page.text", json!({ "url": url, "selector": "p" }));
	assert!(success, "page.text failed: {stderr}");
	assert_eq!(json["data"]["text"], "Explicit URL Test");
}
