//! Integration tests for helpful error messages.
//!
//! These tests verify that error messages provide helpful hints when
//! common mistakes are made, such as using a selector where a URL is expected.

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

/// Test: Error message suggests `-s` when selector-like URL is used
///
/// When a user provides a selector-like string as a URL (e.g., via --url flag),
/// the error should hint about using `-s` for CSS selectors.
#[test]
fn error_suggests_selector_flag_for_selector_like_url() {
    let _lock = lock_context();
    clear_context_store();

    // Using --url with a selector-like value should fail and suggest -s
    // Note: The smart detection in args.rs catches positional selector args,
    // but explicit --url bypasses that
    let (success, stdout, _stderr) = run_pw(&["-f", "json", "text", "--url", ".class-name"]);

    // Command should fail because ".class-name" is not a valid URL
    assert!(!success, "Expected failure with selector-like URL");

    // Check that the error output contains the helpful hint
    // The navigation error should mention the -s flag
    let output = stdout.to_lowercase();
    assert!(
        output.contains("-s") || output.contains("selector"),
        "Expected error to mention -s or selector, got: {}",
        stdout
    );
}

/// Test: Error message mentions context when no URL provided
///
/// When running a command without a URL and without prior context setup,
/// the error should explain how to set up context.
#[test]
fn error_mentions_context_when_no_url() {
    let _lock = lock_context();
    clear_context_store();

    // Run text command without any URL or prior context
    let (success, stdout, _stderr) = run_pw(&["-f", "json", "text", "-s", "body"]);

    // Should fail because no URL available
    assert!(!success, "Expected failure without URL");

    // Check that the error mentions context
    let output = stdout.to_lowercase();
    assert!(
        output.contains("navigate") || output.contains("context") || output.contains("url"),
        "Expected error to mention navigate/context/URL, got: {}",
        stdout
    );
}

/// Test: Error message is helpful when --no-context requires URL
///
/// When using --no-context without providing a URL, the error should
/// explain what's needed.
#[test]
fn error_helpful_for_no_context_without_url() {
    let _lock = lock_context();
    clear_context_store();

    // First set up context
    let url = "data:text/html,<h1>Test</h1>";
    let (success, _stdout, stderr) = run_pw(&["-f", "json", "navigate", url]);
    assert!(success, "Navigate failed: {}", stderr);

    // Now try --no-context without URL
    let (success, stdout, _stderr) = run_pw(&["-f", "json", "--no-context", "text", "-s", "body"]);

    // Should fail
    assert!(!success, "Expected failure with --no-context and no URL");

    // Check that the error is helpful
    let output = stdout.to_lowercase();
    assert!(
        output.contains("url") && output.contains("required"),
        "Expected error about URL required, got: {}",
        stdout
    );
}
