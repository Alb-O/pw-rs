//! Integration tests for `page.read` in protocol v2.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use serde_json::{Value, json};

static CONTEXT_LOCK: Mutex<()> = Mutex::new(());

fn pw_binary() -> PathBuf {
	let mut path = std::env::current_exe().expect("current_exe should resolve");
	path.pop();
	path.pop();
	path.push("pw");
	path
}

fn workspace_root() -> PathBuf {
	std::env::temp_dir().join("pw-cli-page-read")
}

fn clear_context_store() {
	let _ = std::fs::remove_dir_all(workspace_root());
}

fn run_exec(op: &str, input: serde_json::Value) -> (bool, Value, String) {
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
	let parsed = serde_json::from_str(&stdout).unwrap_or_else(|err| panic!("expected JSON stdout: {err}\nstdout:\n{stdout}\nstderr:\n{stderr}"));
	(output.status.success(), parsed, stderr)
}

#[test]
fn read_defaults_to_markdown_format() {
	let _lock = CONTEXT_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
	clear_context_store();

	let (success, json, stderr) = run_exec(
		"page.read",
		json!({
			"url": "data:text/html,<article><h1>Guide</h1><p>This article is long enough to remain after extraction and should render markdown output.</p></article>"
		}),
	);

	assert!(success, "command failed: {stderr}");
	assert_eq!(json["ok"], Value::Bool(true));
	assert_eq!(json["data"]["format"], Value::String("markdown".to_string()));
	assert!(json["data"]["content"].as_str().expect("content should be string").contains("# Guide"));
}

#[test]
fn read_supports_text_output() {
	let _lock = CONTEXT_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
	clear_context_store();

	let (success, json, stderr) = run_exec(
		"page.read",
		json!({
			"url": "data:text/html,<article><h1>Title</h1><p>Hello world from text output.</p></article>",
			"outputFormat": "text"
		}),
	);

	assert!(success, "command failed: {stderr}");
	assert_eq!(json["ok"], Value::Bool(true));
	assert_eq!(json["data"]["format"], Value::String("text".to_string()));
	let content = json["data"]["content"].as_str().expect("content should be string");
	assert!(content.contains("Hello world from text output."));
	assert!(json["data"]["wordCount"].as_u64().expect("wordCount should be a number") > 0);
}

#[test]
fn read_supports_html_output() {
	let _lock = CONTEXT_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
	clear_context_store();

	let (success, json, stderr) = run_exec(
		"page.read",
		json!({
			"url": "data:text/html,<article><h1>Markup</h1><p>Retain html payload.</p></article>",
			"outputFormat": "html"
		}),
	);

	assert!(success, "command failed: {stderr}");
	assert_eq!(json["ok"], Value::Bool(true));
	assert_eq!(json["data"]["format"], Value::String("html".to_string()));
	let content = json["data"]["content"].as_str().expect("content should be string");
	assert!(content.contains("<h1>Markup</h1>"));
}

#[test]
fn read_includes_metadata_when_requested() {
	let _lock = CONTEXT_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
	clear_context_store();

	let (success, json, stderr) = run_exec(
		"page.read",
		json!({
			"url": "data:text/html,<html><head><meta property='og:title' content='Meta Story'><meta name='author' content='Ada Lovelace'><meta property='article:published_time' content='2025-01-02'></head><body><article><p>Body</p></article></body></html>",
			"metadata": true
		}),
	);

	assert!(success, "command failed: {stderr}");
	assert_eq!(json["ok"], Value::Bool(true));
	assert_eq!(json["data"]["title"], Value::String("Meta Story".to_string()));
	assert_eq!(json["data"]["author"], Value::String("Ada Lovelace".to_string()));
	assert_eq!(json["data"]["published"], Value::String("2025-01-02".to_string()));
}

#[test]
fn read_omits_metadata_fields_without_flag() {
	let _lock = CONTEXT_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
	clear_context_store();

	let (success, json, stderr) = run_exec(
		"page.read",
		json!({
			"url": "data:text/html,<html><head><meta property='og:title' content='Hidden Meta'></head><body><article><p>Body</p></article></body></html>"
		}),
	);

	assert!(success, "command failed: {stderr}");
	assert_eq!(json["ok"], Value::Bool(true));
	let data = json["data"].as_object().expect("data should be object");
	assert!(!data.contains_key("title"));
	assert!(!data.contains_key("author"));
	assert!(!data.contains_key("published"));
}
