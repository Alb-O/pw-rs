//! Integration tests for click navigation detection in protocol v2.

use std::path::PathBuf;
use std::process::Command;

use serde_json::json;

fn pw_binary() -> PathBuf {
	let mut path = std::env::current_exe().unwrap();
	path.pop();
	path.pop();
	path.push("pw");
	path
}

fn workspace_root() -> PathBuf {
	std::env::temp_dir().join("pw-cli-click-navigation")
}

fn clear_context_store() {
	let _ = std::fs::remove_dir_all(workspace_root());
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
fn click_element_changes_url_reports_navigated() {
	clear_context_store();
	let html = "data:text/html,<html><body><button id='change-btn' onclick=\"history.pushState({}, '', location.href + '?changed=1')\">Change URL</button></body></html>";

	let (success, json, stderr) = run_exec("click", json!({ "url": html, "selector": "#change-btn", "waitMs": 100 }));
	assert!(success, "click failed: {stderr}");
	assert_eq!(json["ok"], true);
	assert_eq!(json["data"]["navigated"], true);
	let after = json["data"]["afterUrl"].as_str().unwrap_or_default();
	assert!(after.contains("changed=1"), "expected changed=1 in afterUrl: {after}");
}

#[test]
fn click_button_no_navigate_reports_false() {
	clear_context_store();
	let html = r#"data:text/html,<html><body><button id="action-btn" onclick="document.body.innerHTML += '<p>Clicked</p>'">Action</button></body></html>"#;

	let (success, json, stderr) = run_exec("click", json!({ "url": html, "selector": "#action-btn", "waitMs": 100 }));
	assert!(success, "click failed: {stderr}");
	assert_eq!(json["ok"], true);
	assert_eq!(json["data"]["navigated"], false);
}

#[test]
fn click_includes_before_and_after_urls() {
	clear_context_store();
	let html = r#"data:text/html,<html><body><span id="target">Text</span></body></html>"#;

	let (success, json, stderr) = run_exec("click", json!({ "url": html, "selector": "#target" }));
	assert!(success, "click failed: {stderr}");

	assert!(json["data"]["beforeUrl"].is_string());
	assert!(json["data"]["afterUrl"].is_string());
	assert!(json["data"]["navigated"].is_boolean());
}

#[test]
fn click_uses_accurate_url_detection() {
	clear_context_store();
	let html = r#"data:text/html,<html><body><div id="el">Element</div></body></html>"#;

	let (success, json, stderr) = run_exec("click", json!({ "url": html, "selector": "#el" }));
	assert!(success, "click failed: {stderr}");

	let before = json["data"]["beforeUrl"].as_str().unwrap_or_default();
	assert!(before.contains("data:text/html"), "expected data URL in beforeUrl: {before}");
}
