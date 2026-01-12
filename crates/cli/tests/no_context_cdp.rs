//! Integration tests for --no-context mode with CDP connections.
//!
//! These tests verify that when connected via CDP endpoint, the --no-context
//! flag allows commands to operate on the current browser page without
//! requiring an explicit URL.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

/// Mutex to serialize tests that use the global context store.
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

/// Helper to run pw command with --no-project
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

/// Test: --no-context mode requires URL without CDP endpoint
///
/// When --no-context is used without a CDP endpoint, commands should error
/// if no URL is provided.
#[test]
fn no_context_without_cdp_requires_url() {
    let _lock = lock_context();
    clear_context_store();

    // Run text command with --no-context but without URL or CDP
    let (success, stdout, _stderr) = run_pw(&["-f", "json", "--no-context", "text", "-s", "body"]);

    // Should fail because no URL provided and no CDP endpoint
    assert!(
        !success,
        "Expected failure when --no-context without URL or CDP"
    );
    assert!(
        stdout.contains("URL is required") || stdout.contains("error"),
        "Expected error about missing URL: {}",
        stdout
    );
}

/// Test: Normal mode with context works without URL
///
/// When context is enabled (default), subsequent commands can use the cached URL.
#[test]
fn normal_mode_uses_cached_url() {
    let _lock = lock_context();
    clear_context_store();

    // First, navigate to set up context
    let url = "data:text/html,<h1>Cached Test</h1>";
    let (success, _stdout, stderr) = run_pw(&["-f", "json", "navigate", url]);
    assert!(success, "Navigate failed: {}", stderr);

    // Now run text without URL - should use context
    let (success, stdout, stderr) = run_pw(&["-f", "json", "text", "-s", "h1"]);
    assert!(success, "Text command failed: {}", stderr);
    assert!(
        stdout.contains("Cached Test"),
        "Expected cached content: {}",
        stdout
    );
}

/// Test: --no-context mode ignores cached URL
///
/// When --no-context is used, cached URLs should be ignored.
#[test]
fn no_context_ignores_cached_url() {
    let _lock = lock_context();
    clear_context_store();

    // First, navigate to set up context
    let url = "data:text/html,<h1>Should Be Ignored</h1>";
    let (success, _stdout, stderr) = run_pw(&["-f", "json", "navigate", url]);
    assert!(success, "Navigate failed: {}", stderr);

    // Now run text with --no-context but without URL
    // Should fail even though context has a URL
    let (success, stdout, _stderr) = run_pw(&["-f", "json", "--no-context", "text", "-s", "h1"]);

    assert!(!success, "Expected failure with --no-context and no URL");
    assert!(
        stdout.contains("URL is required") || stdout.contains("error"),
        "Expected error about missing URL: {}",
        stdout
    );
}

/// Test: --no-context with explicit URL still works
///
/// When --no-context is used with an explicit URL, the command should work.
#[test]
fn no_context_with_explicit_url_works() {
    let _lock = lock_context();
    clear_context_store();

    let url = "data:text/html,<p>Explicit URL Test</p>";
    let (success, stdout, stderr) = run_pw(&["-f", "json", "--no-context", "text", url, "-s", "p"]);

    assert!(success, "Text command failed: {}", stderr);
    assert!(
        stdout.contains("Explicit URL Test"),
        "Expected content: {}",
        stdout
    );
}

// Note: Testing --no-context with an actual CDP connection requires
// launching a browser with remote debugging enabled, which is complex
// for automated tests. The unit tests for the sentinel logic in
// context_store.rs and session_broker.rs cover the core functionality.
//
// Manual verification:
// 1. Launch Chrome with: google-chrome --remote-debugging-port=9222
// 2. Get the WS URL: curl -s http://127.0.0.1:9222/json/version | jq -r .webSocketDebuggerUrl
// 3. Connect: pw connect "ws://..."
// 4. Navigate: pw navigate "https://example.com"
// 5. Test: pw --no-context text -s "h1"  # Should work on current page
