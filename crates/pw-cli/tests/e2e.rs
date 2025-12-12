//! Integration tests for pw-tool
//!
//! These tests launch actual browser instances and verify the CLI works correctly.
//! They use data: URLs to avoid network dependencies.

use std::path::PathBuf;
use std::process::Command;

/// Helper to get the pw binary path
fn pw_binary() -> PathBuf {
    // In cargo test, the binary is in target/debug
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test binary name
    path.pop(); // Remove deps
    path.push("pw");
    path
}

/// Helper to run pw command and capture output
fn run_pw(args: &[&str]) -> (bool, String, String) {
    let output = Command::new(pw_binary())
        .args(args)
        .output()
        .expect("Failed to execute pw");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (output.status.success(), stdout, stderr)
}

// =============================================================================
// Screenshot Command Tests
// =============================================================================

#[test]
fn screenshot_creates_file() {
    let temp_dir = std::env::temp_dir();
    let output_path = temp_dir.join("pw-test-screenshot.png");

    // Clean up any existing file
    let _ = std::fs::remove_file(&output_path);

    let (success, _stdout, stderr) = run_pw(&[
        "screenshot",
        "data:text/html,<h1>Test Screenshot</h1>",
        "-o",
        output_path.to_str().unwrap(),
    ]);

    assert!(success, "Command failed: {}", stderr);
    assert!(output_path.exists(), "Screenshot file was not created");

    // Verify it's a valid PNG
    let metadata = std::fs::metadata(&output_path).unwrap();
    assert!(metadata.len() > 0, "Screenshot file is empty");

    // Clean up
    let _ = std::fs::remove_file(&output_path);
}

#[test]
fn screenshot_with_complex_html() {
    let temp_dir = std::env::temp_dir();
    let output_path = temp_dir.join("pw-test-screenshot-complex.png");

    let _ = std::fs::remove_file(&output_path);

    let html = "data:text/html,<html><body style='background:blue'><h1 style='color:white'>Complex Test</h1><p>Paragraph</p></body></html>";

    let (success, _stdout, stderr) = run_pw(&["screenshot", html, "-o", output_path.to_str().unwrap()]);

    assert!(success, "Command failed: {}", stderr);
    assert!(output_path.exists(), "Screenshot file was not created");

    let _ = std::fs::remove_file(&output_path);
}

// =============================================================================
// HTML Command Tests
// =============================================================================

#[test]
fn html_full_page() {
    let (success, stdout, stderr) = run_pw(&[
        "html",
        "data:text/html,<html><body><h1>Title</h1><p>Content</p></body></html>",
    ]);

    assert!(success, "Command failed: {}", stderr);
    assert!(stdout.contains("<h1>Title</h1>"), "Expected h1 in output");
    assert!(
        stdout.contains("<p>Content</p>"),
        "Expected paragraph in output"
    );
}

#[test]
fn html_with_selector() {
    let (success, stdout, stderr) = run_pw(&[
        "html",
        "data:text/html,<div><span id='target'>Found me</span><span>Other</span></div>",
        "#target",
    ]);

    assert!(success, "Command failed: {}", stderr);
    assert_eq!(stdout.trim(), "Found me");
}

#[test]
fn html_nested_elements() {
    let (success, stdout, stderr) = run_pw(&[
        "html",
        "data:text/html,<div class='wrapper'><ul><li>One</li><li>Two</li></ul></div>",
        ".wrapper",
    ]);

    assert!(success, "Command failed: {}", stderr);
    assert!(stdout.contains("<ul>"), "Expected ul in output");
    assert!(stdout.contains("<li>One</li>"), "Expected first li");
    assert!(stdout.contains("<li>Two</li>"), "Expected second li");
}

// =============================================================================
// Text Command Tests
// =============================================================================

#[test]
fn text_simple() {
    let (success, stdout, stderr) = run_pw(&[
        "text",
        "data:text/html,<p id='msg'>Hello World</p>",
        "#msg",
    ]);

    assert!(success, "Command failed: {}", stderr);
    assert_eq!(stdout.trim(), "Hello World");
}

#[test]
fn text_nested_content() {
    let (success, stdout, stderr) = run_pw(&[
        "text",
        "data:text/html,<div id='container'><span>First</span> <span>Second</span></div>",
        "#container",
    ]);

    assert!(success, "Command failed: {}", stderr);
    assert!(stdout.contains("First"), "Expected 'First' in output");
    assert!(stdout.contains("Second"), "Expected 'Second' in output");
}

#[test]
fn text_with_whitespace() {
    let (success, stdout, stderr) = run_pw(&[
        "text",
        "data:text/html,<pre id='code'>  indented  </pre>",
        "#code",
    ]);

    assert!(success, "Command failed: {}", stderr);
    // Text should be trimmed
    assert!(stdout.contains("indented"), "Expected 'indented' in output");
}

// =============================================================================
// Eval Command Tests
// =============================================================================

#[test]
fn eval_simple_expression() {
    let (success, stdout, stderr) = run_pw(&["eval", "data:text/html,<h1>Test</h1>", "1 + 1"]);

    assert!(success, "Command failed: {}", stderr);
    assert_eq!(stdout.trim(), "2");
}

#[test]
fn eval_document_title() {
    let (success, stdout, stderr) = run_pw(&[
        "eval",
        "data:text/html,<html><head><title>My Title</title></head></html>",
        "document.title",
    ]);

    assert!(success, "Command failed: {}", stderr);
    assert!(
        stdout.contains("My Title"),
        "Expected title in output: {}",
        stdout
    );
}

#[test]
fn eval_query_selector() {
    let (success, stdout, stderr) = run_pw(&[
        "eval",
        "data:text/html,<div id='test'>Content</div>",
        "document.querySelector('#test').textContent",
    ]);

    assert!(success, "Command failed: {}", stderr);
    assert!(
        stdout.contains("Content"),
        "Expected 'Content' in output: {}",
        stdout
    );
}

#[test]
fn eval_returns_object() {
    let (success, stdout, stderr) = run_pw(&[
        "eval",
        "data:text/html,<html></html>",
        "({a: 1, b: 'test'})",
    ]);

    assert!(success, "Command failed: {}", stderr);
    assert!(stdout.contains("\"a\""), "Expected 'a' key in output");
    assert!(stdout.contains("\"b\""), "Expected 'b' key in output");
}

#[test]
fn eval_returns_array() {
    let (success, stdout, stderr) =
        run_pw(&["eval", "data:text/html,<html></html>", "[1, 2, 3]"]);

    assert!(success, "Command failed: {}", stderr);
    assert!(stdout.contains("1"), "Expected 1 in output");
    assert!(stdout.contains("2"), "Expected 2 in output");
    assert!(stdout.contains("3"), "Expected 3 in output");
}

// =============================================================================
// Coords Command Tests
// =============================================================================

#[test]
fn coords_finds_element() {
    let (success, stdout, stderr) = run_pw(&[
        "coords",
        "data:text/html,<button id='btn' style='width:100px;height:50px'>Click</button>",
        "#btn",
    ]);

    assert!(success, "Command failed: {}", stderr);
    assert!(stdout.contains("\"x\""), "Expected x coordinate");
    assert!(stdout.contains("\"y\""), "Expected y coordinate");
    assert!(stdout.contains("\"width\""), "Expected width");
    assert!(stdout.contains("\"height\""), "Expected height");
}

#[test]
fn coords_includes_text() {
    let (success, stdout, stderr) = run_pw(&[
        "coords",
        "data:text/html,<span id='target'>Sample Text</span>",
        "#target",
    ]);

    assert!(success, "Command failed: {}", stderr);
    assert!(
        stdout.contains("Sample Text"),
        "Expected text content in output"
    );
}

#[test]
fn coords_includes_href() {
    let (success, stdout, stderr) = run_pw(&[
        "coords",
        "data:text/html,<a id='link' href='/page'>Link</a>",
        "#link",
    ]);

    assert!(success, "Command failed: {}", stderr);
    assert!(stdout.contains("/page"), "Expected href in output");
}

#[test]
fn coords_element_not_found() {
    let (success, stdout, _stderr) = run_pw(&[
        "coords",
        "data:text/html,<div>No button here</div>",
        "#nonexistent",
    ]);

    assert!(success, "Command should succeed even if element not found");
    assert!(
        stdout.contains("not found") || stdout.contains("null"),
        "Expected 'not found' message"
    );
}

// =============================================================================
// Coords-All Command Tests
// =============================================================================

#[test]
fn coords_all_multiple_elements() {
    let (success, stdout, stderr) = run_pw(&[
        "coords-all",
        "data:text/html,<ul><li class='item'>One</li><li class='item'>Two</li><li class='item'>Three</li></ul>",
        ".item",
    ]);

    assert!(success, "Command failed: {}", stderr);
    assert!(stdout.contains("\"index\": 0"), "Expected first element");
    assert!(stdout.contains("\"index\": 1"), "Expected second element");
    assert!(stdout.contains("\"index\": 2"), "Expected third element");
}

#[test]
fn coords_all_empty_result() {
    let (success, stdout, stderr) = run_pw(&[
        "coords-all",
        "data:text/html,<div>Nothing here</div>",
        ".nonexistent",
    ]);

    assert!(success, "Command failed: {}", stderr);
    assert!(stdout.contains("[]"), "Expected empty array");
}

// =============================================================================
// Navigate Command Tests
// =============================================================================

#[test]
fn navigate_returns_json() {
    let (success, stdout, stderr) = run_pw(&[
        "navigate",
        "data:text/html,<html><head><title>Nav Test</title></head></html>",
    ]);

    assert!(success, "Command failed: {}", stderr);
    assert!(stdout.contains("\"url\""), "Expected url in JSON");
    assert!(stdout.contains("\"title\""), "Expected title in JSON");
    assert!(stdout.contains("\"hasErrors\""), "Expected hasErrors in JSON");
}

// =============================================================================
// Wait Command Tests
// =============================================================================

#[test]
fn wait_timeout() {
    let (success, stdout, stderr) = run_pw(&["wait", "data:text/html,<div>Test</div>", "100"]);

    assert!(success, "Command failed: {}", stderr);
    assert!(stdout.contains("100ms"), "Expected wait confirmation");
}

#[test]
fn wait_load_state() {
    let (success, stdout, stderr) =
        run_pw(&["wait", "data:text/html,<div>Test</div>", "networkidle"]);

    assert!(success, "Command failed: {}", stderr);
    assert!(
        stdout.contains("networkidle"),
        "Expected load state confirmation"
    );
}

#[test]
fn wait_selector_found() {
    let (success, stdout, stderr) = run_pw(&[
        "wait",
        "data:text/html,<div id='target'>Exists</div>",
        "#target",
    ]);

    assert!(success, "Command failed: {}", stderr);
    assert!(
        stdout.contains("visible") || stdout.contains("#target"),
        "Expected selector confirmation"
    );
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[test]
fn missing_required_args() {
    let (success, _stdout, stderr) = run_pw(&["screenshot"]);

    assert!(!success, "Command should fail without URL");
    assert!(
        stderr.contains("error") || stderr.contains("required"),
        "Expected error message"
    );
}

#[test]
fn unknown_command() {
    let (success, _stdout, stderr) = run_pw(&["unknown-command"]);

    assert!(!success, "Command should fail for unknown command");
    assert!(
        stderr.contains("error") || stderr.contains("invalid"),
        "Expected error message"
    );
}

// =============================================================================
// Verbose Flag Tests
// =============================================================================

#[test]
fn verbose_output() {
    let temp_dir = std::env::temp_dir();
    let output_path = temp_dir.join("pw-test-verbose.png");

    let _ = std::fs::remove_file(&output_path);

    let (success, _stdout, stderr) = run_pw(&[
        "-v",
        "screenshot",
        "data:text/html,<h1>Verbose Test</h1>",
        "-o",
        output_path.to_str().unwrap(),
    ]);

    assert!(success, "Command failed: {}", stderr);
    // Verbose mode should produce more output, but we just verify it doesn't break
    assert!(stderr.contains("INFO"), "Expected INFO message in stderr");

    let _ = std::fs::remove_file(&output_path);
}

// =============================================================================
// Help and Version Tests
// =============================================================================

#[test]
fn help_flag() {
    let (success, stdout, _stderr) = run_pw(&["--help"]);

    assert!(success, "Help should succeed");
    assert!(stdout.contains("Usage"), "Expected usage in help");
    assert!(stdout.contains("screenshot"), "Expected screenshot command");
    assert!(stdout.contains("html"), "Expected html command");
}

#[test]
fn version_flag() {
    let (success, stdout, _stderr) = run_pw(&["--version"]);

    assert!(success, "Version should succeed");
    assert!(
        stdout.contains("pw") || stdout.contains("0.1"),
        "Expected version info"
    );
}

#[test]
fn subcommand_help() {
    let (success, stdout, _stderr) = run_pw(&["screenshot", "--help"]);

    assert!(success, "Subcommand help should succeed");
    assert!(
        stdout.contains("screenshot"),
        "Expected screenshot description"
    );
    assert!(stdout.contains("URL"), "Expected URL argument");
}
