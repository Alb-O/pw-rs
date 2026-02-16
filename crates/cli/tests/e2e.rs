//! Integration tests for `pw` protocol v2.
//!
//! These tests launch real browser instances and use `data:` URLs to avoid
//! network dependencies.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;

fn pw_binary() -> PathBuf {
	let mut path = std::env::current_exe().unwrap();
	path.pop();
	path.pop();
	path.push("pw");
	path
}

static WORKSPACE_COUNTER: AtomicU64 = AtomicU64::new(1);

fn unique_workspace() -> PathBuf {
	let run_id = WORKSPACE_COUNTER.fetch_add(1, Ordering::Relaxed);
	std::env::temp_dir().join("pw-cli-e2e").join(format!("run-{run_id}"))
}

fn run_pw(args: &[&str]) -> (bool, String, String) {
	let workspace = unique_workspace();
	let _ = std::fs::create_dir_all(&workspace);
	let output = Command::new(pw_binary())
		.current_dir(&workspace)
		.args(args)
		.output()
		.expect("failed to execute pw");
	let stdout = String::from_utf8_lossy(&output.stdout).to_string();
	let stderr = String::from_utf8_lossy(&output.stderr).to_string();
	(output.status.success(), stdout, stderr)
}

fn run_exec(op: &str, input: serde_json::Value) -> (bool, serde_json::Value, String) {
	let (success, stdout, stderr) = run_pw(&["-f", "json", "exec", op, "--input", &input.to_string()]);
	let parsed = serde_json::from_str::<serde_json::Value>(&stdout).unwrap_or_else(|_| json!({ "raw": stdout }));
	(success, parsed, stderr)
}

#[test]
fn screenshot_creates_file() {
	let temp_dir = std::env::temp_dir();
	let output_path = temp_dir.join("pw-test-screenshot.png");
	let _ = std::fs::remove_file(&output_path);

	let (success, _json, stderr) = run_exec(
		"screenshot",
		json!({
			"url": "data:text/html,<h1>Test Screenshot</h1>",
			"output": output_path.to_string_lossy().to_string()
		}),
	);
	assert!(success, "command failed: {stderr}");
	assert!(output_path.exists(), "screenshot file was not created");
	assert!(std::fs::metadata(&output_path).unwrap().len() > 0, "screenshot file is empty");
	let _ = std::fs::remove_file(&output_path);
}

#[test]
fn html_with_selector() {
	let (success, json, stderr) = run_exec(
		"page.html",
		json!({
			"url": "data:text/html,<div><span id='target'>Found me</span><span>Other</span></div>",
			"selector": "#target"
		}),
	);
	assert!(success, "command failed: {stderr}");
	assert_eq!(json["ok"], true);
	assert!(json["data"]["html"].as_str().unwrap_or_default().contains("Found me"));
}

#[test]
fn text_simple() {
	let (success, json, stderr) = run_exec(
		"page.text",
		json!({
			"url": "data:text/html,<p id='msg'>Hello World</p>",
			"selector": "#msg"
		}),
	);
	assert!(success, "command failed: {stderr}");
	assert_eq!(json["ok"], true);
	assert_eq!(json["schemaVersion"], 5);
	assert!(json["durationMs"].is_null() || json["durationMs"].is_number());
	assert_eq!(json["data"]["text"], "Hello World");
	assert_eq!(json["data"]["matchCount"], 1);
}

#[test]
fn eval_simple_expression() {
	let (success, json, stderr) = run_exec("page.eval", json!({ "expression": "1 + 1", "url": "data:text/html,<h1>Test</h1>" }));
	assert!(success, "command failed: {stderr}");
	assert_eq!(json["ok"], true);
	assert_eq!(json["data"]["result"], 2);
}

#[test]
fn eval_document_title() {
	let (success, json, stderr) = run_exec(
		"page.eval",
		json!({
			"expression": "document.title",
			"url": "data:text/html,<html><head><title>My Title</title></head></html>"
		}),
	);
	assert!(success, "command failed: {stderr}");
	assert_eq!(json["data"]["result"], "My Title");
}

#[test]
fn eval_query_selector() {
	let (success, json, stderr) = run_exec(
		"page.eval",
		json!({
			"expression": "document.querySelector('#test').textContent",
			"url": "data:text/html,<div id='test'>Content</div>"
		}),
	);
	assert!(success, "command failed: {stderr}");
	assert_eq!(json["data"]["result"], "Content");
}

#[test]
fn coords_finds_element() {
	let (success, json, stderr) = run_exec(
		"page.coords",
		json!({
			"url": "data:text/html,<button id='btn' style='width:100px;height:50px'>Click</button>",
			"selector": "#btn"
		}),
	);
	assert!(success, "command failed: {stderr}");
	assert!(json["data"]["coords"]["x"].is_number());
	assert!(json["data"]["coords"]["y"].is_number());
	assert!(json["data"]["coords"]["width"].is_number());
	assert!(json["data"]["coords"]["height"].is_number());
}

#[test]
fn coords_element_not_found() {
	let (_success, json, _stderr) = run_exec(
		"page.coords",
		json!({
			"url": "data:text/html,<div>No button here</div>",
			"selector": "#nonexistent"
		}),
	);
	assert_eq!(json["ok"], false, "command response should report selector failure");
	let msg = json["error"]["message"].as_str().unwrap_or_default();
	assert!(msg.contains("selector") || msg.contains("No elements"), "expected selector error: {msg}");
}

#[test]
fn coords_all_multiple_elements() {
	let (success, json, stderr) = run_exec(
		"page.coords-all",
		json!({
			"url": "data:text/html,<ul><li class='item'>One</li><li class='item'>Two</li><li class='item'>Three</li></ul>",
			"selector": ".item"
		}),
	);
	assert!(success, "command failed: {stderr}");
	assert_eq!(json["data"]["count"], 3);
}

#[test]
fn coords_all_empty_result() {
	let (success, json, stderr) = run_exec(
		"page.coords-all",
		json!({
			"url": "data:text/html,<div>Nothing here</div>",
			"selector": ".nonexistent"
		}),
	);
	assert!(success, "command failed: {stderr}");
	assert_eq!(json["data"]["coords"], json!([]));
}

#[test]
fn navigate_returns_json() {
	let (success, json, stderr) = run_exec("navigate", json!({ "url": "data:text/html,<html><head><title>Nav Test</title></head></html>" }));
	assert!(success, "command failed: {stderr}");
	assert_eq!(json["ok"], true);
	assert_eq!(json["schemaVersion"], 5);
	assert!(json["data"]["url"].is_string());
	assert_eq!(json["data"]["title"], "Nav Test");
}

#[test]
fn wait_timeout() {
	let (success, json, stderr) = run_exec("wait", json!({ "url": "data:text/html,<div>Test</div>", "condition": "100" }));
	assert!(success, "command failed: {stderr}");
	assert_eq!(json["ok"], true);
	assert_eq!(json["data"]["waitedMs"], 100);
}

#[test]
fn wait_load_state() {
	let (success, json, stderr) = run_exec("wait", json!({ "url": "data:text/html,<div>Test</div>", "condition": "networkidle" }));
	assert!(success, "command failed: {stderr}");
	assert_eq!(json["ok"], true);
	assert_eq!(json["data"]["condition"], "loadstate:networkidle");
}

#[test]
fn wait_selector_found() {
	let (success, json, stderr) = run_exec("wait", json!({ "url": "data:text/html,<div id='target'>Exists</div>", "condition": "#target" }));
	assert!(success, "command failed: {stderr}");
	assert_eq!(json["ok"], true);
	assert_eq!(json["data"]["selectorFound"], true);
}

#[test]
fn missing_required_exec_args() {
	let (success, _stdout, _stderr) = run_pw(&["exec"]);
	assert!(!success, "exec should fail without operation or file");
}

#[test]
fn unknown_command() {
	let (_success, stdout, _stderr) = run_pw(&["-f", "json", "exec", "unknown-command", "--input", "{}"]);
	let json: serde_json::Value = serde_json::from_str(&stdout).expect("expected JSON output");
	assert_eq!(json["ok"], false);
	let msg = json["error"]["message"].as_str().unwrap_or_default().to_lowercase();
	assert!(msg.contains("unknown operation"), "expected unknown operation message, got: {msg}");
}

#[test]
fn verbose_output() {
	let temp_dir = std::env::temp_dir();
	let output_path = temp_dir.join("pw-test-verbose.png");
	let _ = std::fs::remove_file(&output_path);

	let input = json!({
		"url": "data:text/html,<h1>Verbose Test</h1>",
		"output": output_path.to_string_lossy().to_string()
	});
	let (success, _stdout, stderr) = run_pw(&["-v", "-f", "json", "exec", "screenshot", "--input", &input.to_string()]);
	assert!(success, "command failed: {stderr}");
	assert!(stderr.contains("INFO"), "expected INFO output in stderr");
	let _ = std::fs::remove_file(&output_path);
}

#[test]
fn help_flag() {
	let (success, stdout, _stderr) = run_pw(&["--help"]);
	assert!(success, "help should succeed");
	assert!(stdout.contains("Usage"), "expected usage in help");
	assert!(stdout.contains("exec"), "expected exec command");
	assert!(stdout.contains("batch"), "expected batch command");
}

#[test]
fn version_flag() {
	let (success, stdout, _stderr) = run_pw(&["--version"]);
	assert!(success, "version should succeed");
	assert!(stdout.contains("pw"), "expected version output");
}

#[test]
fn subcommand_help() {
	let (success, stdout, _stderr) = run_pw(&["exec", "--help"]);
	assert!(success, "subcommand help should succeed");
	assert!(stdout.contains("OP"), "expected operation argument in help");
	assert!(stdout.contains("--input"), "expected input option in help");
}
