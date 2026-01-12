//! Integration tests for smart URL/selector argument detection.
//!
//! These tests verify that the CLI correctly distinguishes between URLs and
//! CSS selectors when provided as positional arguments.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

/// Mutex to serialize tests that use the global context store.
/// Use `lock_context()` to acquire, which handles poisoned locks.
static CONTEXT_LOCK: Mutex<()> = Mutex::new(());

fn lock_context() -> std::sync::MutexGuard<'static, ()> {
    CONTEXT_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Helper to get the pw binary path
fn pw_binary() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test binary name
    path.pop(); // Remove deps
    path.push("pw");
    path
}

fn context_store_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("pw").join("cli").join("contexts.json")
}

fn clear_context_store() {
    let path = context_store_path();
    let _ = std::fs::remove_file(&path);
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir_all(parent.join("sessions"));
    }
}

/// Helper to run pw command with --no-project to use global context only
fn run_pw(args: &[&str]) -> (bool, String, String) {
    let mut full_args = vec!["--no-project"];
    full_args.extend_from_slice(args);

    let output = Command::new(pw_binary())
        .args(&full_args)
        .output()
        .expect("Failed to execute pw");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (output.status.success(), stdout, stderr)
}

/// Test: Selector-like positional argument with context URL
///
/// When context has a URL (from prior navigate), a selector-like positional
/// argument should be treated as a selector.
#[test]
fn selector_positional_with_context() {
    let _lock = lock_context();
    clear_context_store();

    // First, set up context with a URL
    let url = "data:text/html,<div class=\"content\">Hello World</div>";
    let (success, _stdout, stderr) = run_pw(&["-f", "json", "navigate", url]);
    assert!(success, "Navigate failed: {}", stderr);

    // Now run text with just a selector - should use context URL
    let (success, stdout, stderr) = run_pw(&["-f", "json", "text", ".content"]);
    assert!(success, "Text command failed: {}", stderr);
    assert!(
        stdout.contains("Hello World"),
        "Expected content in output: {}",
        stdout
    );
}

/// Test: URL-like positional argument
///
/// A string that looks like a URL should be treated as a URL, not a selector.
#[test]
fn url_positional_treated_as_url() {
    let _lock = lock_context();
    clear_context_store();

    // Run text with a data: URL - should navigate and get body text
    let url = "data:text/html,<body>Page Content</body>";
    let (success, stdout, stderr) = run_pw(&["-f", "json", "text", url, "-s", "body"]);
    assert!(success, "Text command failed: {}", stderr);
    assert!(
        stdout.contains("Page Content"),
        "Expected page content: {}",
        stdout
    );
}

/// Test: Both URL and selector as positional arguments
///
/// When two positional arguments are provided, the first should be URL and
/// second should be selector.
#[test]
fn both_url_and_selector_positional() {
    let _lock = lock_context();
    clear_context_store();

    let url = "data:text/html,<h1>Title</h1><p class=\"para\">Paragraph</p>";
    let (success, stdout, stderr) = run_pw(&["-f", "json", "text", url, ".para"]);
    assert!(success, "Text command failed: {}", stderr);
    assert!(
        stdout.contains("Paragraph"),
        "Expected paragraph content: {}",
        stdout
    );
}

/// Test: Explicit -s flag for selector (backward compatibility)
///
/// The explicit -s flag should always work regardless of what the argument
/// looks like.
#[test]
fn explicit_selector_flag() {
    let _lock = lock_context();
    clear_context_store();

    // Set up context
    let url = "data:text/html,<span id=\"test\">Test Text</span>";
    let (success, _stdout, stderr) = run_pw(&["-f", "json", "navigate", url]);
    assert!(success, "Navigate failed: {}", stderr);

    // Use explicit -s flag
    let (success, stdout, stderr) = run_pw(&["-f", "json", "text", "-s", "#test"]);
    assert!(success, "Text command failed: {}", stderr);
    assert!(
        stdout.contains("Test Text"),
        "Expected test text: {}",
        stdout
    );
}

/// Test: ID selector detection
///
/// Selectors starting with # should be recognized as selectors.
#[test]
fn id_selector_detection() {
    let _lock = lock_context();
    clear_context_store();

    let url = "data:text/html,<div id=\"main\">Main Content</div>";
    let (success, _stdout, stderr) = run_pw(&["-f", "json", "navigate", url]);
    assert!(success, "Navigate failed: {}", stderr);

    // #main should be detected as selector
    let (success, stdout, stderr) = run_pw(&["-f", "json", "text", "#main"]);
    assert!(success, "Text command failed: {}", stderr);
    assert!(
        stdout.contains("Main Content"),
        "Expected main content: {}",
        stdout
    );
}

/// Test: Complex selector detection
///
/// Complex CSS selectors should be properly detected.
#[test]
fn complex_selector_detection() {
    let _lock = lock_context();
    clear_context_store();

    let url = "data:text/html,<ul><li>First</li><li>Second</li></ul>";
    let (success, _stdout, stderr) = run_pw(&["-f", "json", "navigate", url]);
    assert!(success, "Navigate failed: {}", stderr);

    // Complex selector with combinator - use :first-child to match exactly one element
    let (success, stdout, stderr) = run_pw(&["-f", "json", "text", "li:first-child"]);
    assert!(success, "Text command failed: {}", stderr);
    assert!(
        stdout.contains("First"),
        "Expected first item content: {}",
        stdout
    );
}

/// Test: Click command with selector detection
///
/// The click command should also support smart selector detection.
#[test]
fn click_with_selector_detection() {
    let _lock = lock_context();
    clear_context_store();

    let url = "data:text/html,<button class=\"btn\">Click Me</button>";
    let (success, _stdout, stderr) = run_pw(&["-f", "json", "navigate", url]);
    assert!(success, "Navigate failed: {}", stderr);

    // .btn should be detected as selector
    let (success, stdout, stderr) = run_pw(&["-f", "json", "click", ".btn"]);
    assert!(success, "Click command failed: {}", stderr);
    assert!(
        stdout.contains("\"ok\": true"),
        "Expected success: {}",
        stdout
    );
}

/// Test: HTML command with selector detection
///
/// The html command should also support smart selector detection.
#[test]
fn html_with_selector_detection() {
    let _lock = lock_context();
    clear_context_store();

    let url = "data:text/html,<article><p>Article content</p></article>";
    let (success, _stdout, stderr) = run_pw(&["-f", "json", "navigate", url]);
    assert!(success, "Navigate failed: {}", stderr);

    // article should work as tag selector (contains no special chars)
    // Let's use a more explicit selector
    let (success, stdout, stderr) = run_pw(&["-f", "json", "html", "-s", "article"]);
    assert!(success, "HTML command failed: {}", stderr);
    assert!(
        stdout.contains("<p>Article content</p>"),
        "Expected article HTML: {}",
        stdout
    );
}
