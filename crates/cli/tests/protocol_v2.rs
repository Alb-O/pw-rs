use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use tempfile::TempDir;

fn pw_binary() -> PathBuf {
	let mut path = std::env::current_exe().unwrap();
	path.pop();
	path.pop();
	path.push("pw");
	path
}

fn run_pw(workdir: &std::path::Path, args: &[&str]) -> (bool, String, String) {
	let output = Command::new(pw_binary())
		.current_dir(workdir)
		.args(args)
		.output()
		.expect("failed to execute pw");

	let stdout = String::from_utf8_lossy(&output.stdout).to_string();
	let stderr = String::from_utf8_lossy(&output.stderr).to_string();
	(output.status.success(), stdout, stderr)
}

#[test]
fn exec_page_text_with_data_url() {
	let tmp = TempDir::new().unwrap();
	let input = r#"{"url":"data:text/html,<h1>Hello V2</h1>","selector":"h1"}"#;
	let (success, stdout, stderr) = run_pw(tmp.path(), &["-f", "json", "exec", "page.text", "--input", input]);

	assert!(success, "exec failed: {stderr}");
	let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
	assert_eq!(json["schemaVersion"], 5);
	assert_eq!(json["ok"], true);
	assert_eq!(json["op"], "page.text");
	assert_eq!(json["data"]["text"], "Hello V2");
}

#[test]
fn batch_rejects_alias_operation_name() {
	let tmp = TempDir::new().unwrap();
	let mut child = Command::new(pw_binary())
		.current_dir(tmp.path())
		.args(["-f", "ndjson", "batch", "--profile", "default"])
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.expect("failed to start pw batch");

	{
		let stdin = child.stdin.as_mut().unwrap();
		writeln!(stdin, r#"{{"schemaVersion":5,"requestId":"1","op":"har-show","input":{{}}}}"#).unwrap();
		writeln!(stdin, r#"{{"schemaVersion":5,"requestId":"2","op":"quit","input":{{}}}}"#).unwrap();
	}

	let output = child.wait_with_output().unwrap();
	assert!(output.status.success(), "batch process failed: {}", String::from_utf8_lossy(&output.stderr));

	let stdout = String::from_utf8_lossy(&output.stdout);
	let first = stdout.lines().find(|line| !line.trim().is_empty()).expect("missing batch response");
	let json: serde_json::Value = serde_json::from_str(first).unwrap();
	assert_eq!(json["schemaVersion"], 5);
	assert_eq!(json["requestId"], "1");
	assert_eq!(json["ok"], false);
	assert_eq!(json["op"], "har-show");
	assert_eq!(json["error"]["code"], "INVALID_INPUT");
}
