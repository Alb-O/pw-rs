use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use serde_json::json;

static HAR_LOCK: Mutex<()> = Mutex::new(());

fn lock_har() -> std::sync::MutexGuard<'static, ()> {
	HAR_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn pw_binary() -> PathBuf {
	let mut path = std::env::current_exe().unwrap();
	path.pop();
	path.pop();
	path.push("pw");
	path
}

fn workspace_root() -> PathBuf {
	std::env::temp_dir().join("pw-cli-har-config")
}

fn clear_context_store() {
	let _ = std::fs::remove_dir_all(workspace_root());
}

fn run_exec_json(op: &str, input: serde_json::Value) -> (bool, serde_json::Value, String) {
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
	let parsed = serde_json::from_str::<serde_json::Value>(&stdout).expect("expected valid JSON output");
	(output.status.success(), parsed, stderr)
}

fn run_pw_raw(args: &[&str]) -> (bool, String, String) {
	let output = Command::new(pw_binary()).args(args).output().expect("failed to execute pw");
	let stdout = String::from_utf8_lossy(&output.stdout).to_string();
	let stderr = String::from_utf8_lossy(&output.stderr).to_string();
	(output.status.success(), stdout, stderr)
}

#[test]
fn har_show_is_disabled_by_default() {
	let _lock = lock_har();
	clear_context_store();

	let (success, json, stderr) = run_exec_json("har.show", json!({}));
	assert!(success, "command failed: {stderr}");
	assert_eq!(json["ok"], true);
	assert_eq!(json["data"]["enabled"], false);
	assert!(json["data"]["har"].is_null());
}

#[test]
fn har_set_persists_and_show_reflects_config() {
	let _lock = lock_har();
	clear_context_store();

	let (success, _json, stderr) = run_exec_json(
		"har.set",
		json!({
			"file": "network.har",
			"content": "embed",
			"mode": "minimal",
			"omitContent": true,
			"urlFilter": "*.api.example.com"
		}),
	);
	assert!(success, "har.set failed: {stderr}");

	let (success, json, stderr) = run_exec_json("har.show", json!({}));
	assert!(success, "har.show failed: {stderr}");
	assert_eq!(json["ok"], true);
	assert_eq!(json["data"]["enabled"], true);
	assert_eq!(json["data"]["har"]["path"], "network.har");
	assert_eq!(json["data"]["har"]["contentPolicy"], "embed");
	assert_eq!(json["data"]["har"]["mode"], "minimal");
	assert_eq!(json["data"]["har"]["omitContent"], true);
	assert_eq!(json["data"]["har"]["urlFilter"], "*.api.example.com");
}

#[test]
fn har_set_persists_profile_config_file() {
	let _lock = lock_har();
	clear_context_store();

	let (success, _json, stderr) = run_exec_json(
		"har.set",
		json!({
			"file": "captures/network.har",
			"content": "embed",
			"mode": "minimal",
			"omitContent": true,
			"urlFilter": "*.api.example.com"
		}),
	);
	assert!(success, "har.set failed: {stderr}");

	let config_path = workspace_root()
		.join("playwright")
		.join(".pw-cli-v4")
		.join("profiles")
		.join("default")
		.join("config.json");

	let config = std::fs::read_to_string(&config_path).unwrap_or_else(|e| panic!("failed to read {}: {}", config_path.display(), e));
	let value: serde_json::Value = serde_json::from_str(&config).expect("expected valid config JSON");
	assert_eq!(value["har"]["path"], "captures/network.har");
	assert_eq!(value["har"]["contentPolicy"], "embed");
	assert_eq!(value["har"]["mode"], "minimal");
	assert_eq!(value["har"]["omitContent"], true);
	assert_eq!(value["har"]["urlFilter"], "*.api.example.com");
}

#[test]
fn har_clear_disables_subsequent_commands() {
	let _lock = lock_har();
	clear_context_store();

	let (success, json, stderr) = run_exec_json(
		"har.set",
		json!({
			"file": "network.har",
			"content": "attach",
			"mode": "full",
			"omitContent": false
		}),
	);
	assert!(success, "har.set failed: {stderr}");
	assert_eq!(json["ok"], true, "har.set should succeed");

	let (success, json, stderr) = run_exec_json("har.clear", json!({}));
	assert!(success, "har.clear failed: {stderr}");
	assert_eq!(json["data"]["cleared"], true);

	let (success, json, stderr) = run_exec_json("har.show", json!({}));
	assert!(success, "har.show failed: {stderr}");
	assert_eq!(json["data"]["enabled"], false);
}

#[test]
fn root_help_lists_v2_commands() {
	let _lock = lock_har();
	let (success, stdout, _stderr) = run_pw_raw(&["--help"]);
	assert!(success, "help should succeed");
	assert!(stdout.contains("exec"), "expected exec command in help output");
	assert!(stdout.contains("batch"), "expected batch command in help output");
	assert!(!stdout.contains("--har"), "legacy --har flag should not appear");
	assert!(!stdout.contains("--har-content"), "legacy --har-content flag should not appear");
}
