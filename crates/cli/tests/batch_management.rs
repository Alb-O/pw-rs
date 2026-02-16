use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

fn pw_binary() -> PathBuf {
	let mut path = std::env::current_exe().unwrap();
	path.pop();
	path.pop();
	path.push("pw");
	path
}

fn workspace_base() -> PathBuf {
	std::env::temp_dir().join("pw-cli-batch-management")
}

fn workspace_root() -> PathBuf {
	static WORKSPACE_COUNTER: AtomicU64 = AtomicU64::new(1);
	let run_id = WORKSPACE_COUNTER.fetch_add(1, Ordering::Relaxed);
	workspace_base().join(format!("run-{run_id}"))
}

fn clear_context_store() {
	let _ = std::fs::remove_dir_all(workspace_base());
}

fn run_pw_batch(lines: &[&str]) -> (bool, String, String) {
	let workspace = workspace_root();
	let _ = std::fs::create_dir_all(&workspace);

	let mut child = Command::new(pw_binary())
		.current_dir(&workspace)
		.args(["-f", "ndjson", "batch", "--profile", "default"])
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.expect("failed to start pw batch");

	{
		let stdin = child.stdin.as_mut().expect("stdin unavailable");
		for line in lines {
			writeln!(stdin, "{line}").expect("failed to write batch request");
		}
	}

	let output = child.wait_with_output().expect("failed waiting for pw batch");
	let stdout = String::from_utf8_lossy(&output.stdout).to_string();
	let stderr = String::from_utf8_lossy(&output.stderr).to_string();
	(output.status.success(), stdout, stderr)
}

fn parse_ndjson(stdout: &str) -> Vec<serde_json::Value> {
	stdout
		.lines()
		.filter(|line| !line.trim().is_empty())
		.map(|line| serde_json::from_str::<serde_json::Value>(line).expect("line should be valid JSON"))
		.collect()
}

#[test]
fn batch_supports_har_show() {
	clear_context_store();

	let (success, stdout, stderr) = run_pw_batch(&[
		r#"{"schemaVersion":5,"requestId":"1","op":"har.show","input":{}}"#,
		r#"{"schemaVersion":5,"requestId":"2","op":"quit","input":{}}"#,
	]);

	assert!(success, "batch run failed: {stderr}");
	let lines = parse_ndjson(&stdout);
	assert!(lines.len() >= 2, "expected at least two response lines, got: {stdout}");

	let first = &lines[0];
	assert_eq!(first["requestId"], "1");
	assert_eq!(first["ok"], true);
	assert_eq!(first["schemaVersion"], 5);
	assert_eq!(first["op"], "har.show");
	assert_eq!(first["data"]["enabled"], false);
}

#[test]
fn batch_rejects_auth_login_as_interactive() {
	clear_context_store();

	let (success, stdout, stderr) = run_pw_batch(&[
		r#"{"schemaVersion":5,"requestId":"1","op":"auth.login","input":{"url":"https://example.com"}}"#,
		r#"{"schemaVersion":5,"requestId":"2","op":"quit","input":{}}"#,
	]);

	assert!(success, "batch run should stay healthy: {stderr}");
	let lines = parse_ndjson(&stdout);
	assert!(lines.len() >= 2, "expected at least two response lines, got: {stdout}");

	let first = &lines[0];
	assert_eq!(first["requestId"], "1");
	assert_eq!(first["ok"], false);
	assert_eq!(first["schemaVersion"], 5);
	assert_eq!(first["op"], "auth.login");
	assert_eq!(first["error"]["code"], "UNSUPPORTED_MODE");
}

#[test]
fn batch_alias_is_rejected() {
	clear_context_store();

	let (success, stdout, stderr) = run_pw_batch(&[
		r#"{"schemaVersion":5,"requestId":"1","op":"har-show","input":{}}"#,
		r#"{"schemaVersion":5,"requestId":"2","op":"quit","input":{}}"#,
	]);

	assert!(success, "batch run failed: {stderr}");
	let lines = parse_ndjson(&stdout);
	assert!(lines.len() >= 2, "expected at least two response lines, got: {stdout}");

	let first = &lines[0];
	assert_eq!(first["requestId"], "1");
	assert_eq!(first["ok"], false);
	assert_eq!(first["schemaVersion"], 5);
	assert_eq!(first["op"], "har-show");
	assert_eq!(first["error"]["code"], "INVALID_INPUT");
}
