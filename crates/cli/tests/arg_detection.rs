//! Integration tests for URL/selector resolution in protocol v2.

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
	std::env::temp_dir().join("pw-cli-arg-detection")
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
fn selector_with_context_url() {
	let _lock = lock_context();
	clear_context_store();

	let url = "data:text/html,<div class=\"content\">Hello World</div>";
	let (success, _json, stderr) = run_exec("navigate", json!({ "url": url }));
	assert!(success, "navigate failed: {stderr}");

	let (success, json, stderr) = run_exec("page.text", json!({ "selector": ".content" }));
	assert!(success, "page.text failed: {stderr}");
	assert_eq!(json["ok"], true);
	assert_eq!(json["data"]["text"], "Hello World");
}

#[test]
fn url_input_is_used_when_provided() {
	let _lock = lock_context();
	clear_context_store();

	let url = "data:text/html,<body>Page Content</body>";
	let (success, json, stderr) = run_exec("page.text", json!({ "url": url, "selector": "body" }));
	assert!(success, "page.text failed: {stderr}");
	assert_eq!(json["data"]["text"], "Page Content");
}

#[test]
fn url_and_selector_both_resolve() {
	let _lock = lock_context();
	clear_context_store();

	let url = "data:text/html,<h1>Title</h1><p class=\"para\">Paragraph</p>";
	let (success, json, stderr) = run_exec("page.text", json!({ "url": url, "selector": ".para" }));
	assert!(success, "page.text failed: {stderr}");
	assert_eq!(json["data"]["text"], "Paragraph");
}

#[test]
fn selector_flag_alias_is_supported() {
	let _lock = lock_context();
	clear_context_store();

	let url = "data:text/html,<span id=\"test\">Test Text</span>";
	let (success, _json, stderr) = run_exec("navigate", json!({ "url": url }));
	assert!(success, "navigate failed: {stderr}");

	let (success, json, stderr) = run_exec("page.text", json!({ "selectorFlag": "#test" }));
	assert!(success, "page.text failed: {stderr}");
	assert_eq!(json["data"]["text"], "Test Text");
}

#[test]
fn id_selector_resolution() {
	let _lock = lock_context();
	clear_context_store();

	let url = "data:text/html,<div id=\"main\">Main Content</div>";
	let (success, _json, stderr) = run_exec("navigate", json!({ "url": url }));
	assert!(success, "navigate failed: {stderr}");

	let (success, json, stderr) = run_exec("page.text", json!({ "selector": "#main" }));
	assert!(success, "page.text failed: {stderr}");
	assert_eq!(json["data"]["text"], "Main Content");
}

#[test]
fn complex_selector_resolution() {
	let _lock = lock_context();
	clear_context_store();

	let url = "data:text/html,<ul><li>First</li><li>Second</li></ul>";
	let (success, _json, stderr) = run_exec("navigate", json!({ "url": url }));
	assert!(success, "navigate failed: {stderr}");

	let (success, json, stderr) = run_exec("page.text", json!({ "selector": "li:first-child" }));
	assert!(success, "page.text failed: {stderr}");
	assert_eq!(json["data"]["text"], "First");
}

#[test]
fn click_with_context_selector() {
	let _lock = lock_context();
	clear_context_store();

	let url = "data:text/html,<button class=\"btn\">Click Me</button>";
	let (success, _json, stderr) = run_exec("navigate", json!({ "url": url }));
	assert!(success, "navigate failed: {stderr}");

	let (success, json, stderr) = run_exec("click", json!({ "selector": ".btn" }));
	assert!(success, "click failed: {stderr}");
	assert_eq!(json["ok"], true);
}

#[test]
fn html_with_context_selector() {
	let _lock = lock_context();
	clear_context_store();

	let url = "data:text/html,<article><p>Article content</p></article>";
	let (success, _json, stderr) = run_exec("navigate", json!({ "url": url }));
	assert!(success, "navigate failed: {stderr}");

	let (success, json, stderr) = run_exec("page.html", json!({ "selector": "article" }));
	assert!(success, "page.html failed: {stderr}");
	let html = json["data"]["html"].as_str().unwrap_or_default();
	assert!(html.contains("<p>Article content</p>"), "expected article HTML in: {html}");
}
