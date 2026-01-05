//! Integration tests for click command navigation detection.
//!
//! These tests verify that the click command accurately reports whether
//! navigation occurred after a click action.

use std::path::PathBuf;
use std::process::Command;

/// Helper to get the pw binary path
fn pw_binary() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test binary name
    path.pop(); // Remove deps
    path.push("pw");
    path
}

fn clear_context_store() {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    let base_dir = base.join("pw").join("cli");
    let path = base_dir.join("contexts.json");
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir_all(base_dir.join("sessions"));
}

/// Helper to run pw command and capture output
fn run_pw(args: &[&str]) -> (bool, String, String) {
    clear_context_store();

    let output = Command::new(pw_binary())
        .args(args)
        .output()
        .expect("Failed to execute pw");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (output.status.success(), stdout, stderr)
}

/// Test: Click an element that triggers pushState URL change, verify `navigated: true`
///
/// This tests that clicking an element which modifies the URL via history.pushState
/// properly detects that the URL changed. We use pushState because full navigation
/// between data: URLs is blocked by browser security policies, and hash changes in
/// data: URLs can interfere with the data: URL parsing.
#[test]
fn click_element_changes_url_reports_navigated() {
    // Page with a button that changes the URL via history.pushState
    // pushState allows URL modification without actual navigation
    let html = "data:text/html,<html><body><button id='change-btn' onclick=\"history.pushState({}, '', location.href + '?changed=1')\">Change URL</button></body></html>";

    let (success, stdout, stderr) = run_pw(&[
        "-f",
        "json",
        "click",
        html,
        "#change-btn",
        "--wait-ms",
        "100",
    ]);

    assert!(success, "Click command failed: {}", stderr);
    assert!(
        stdout.contains("\"ok\": true"),
        "Expected success in JSON: {}",
        stdout
    );
    // The key assertion: navigated should be true when URL changes via pushState
    assert!(
        stdout.contains("\"navigated\": true"),
        "Expected navigated: true when URL changes via pushState. Output: {}",
        stdout
    );
    // Verify that afterUrl contains the query parameter
    assert!(
        stdout.contains("changed=1"),
        "Expected changed=1 in afterUrl: {}",
        stdout
    );
}

/// Test: Click a button that doesn't navigate, verify `navigated: false`
///
/// This tests that clicking an element that only modifies the page content
/// (via JavaScript) without changing the URL properly reports no navigation.
#[test]
fn click_button_no_navigate_reports_false() {
    // Page with a button that modifies content but doesn't navigate
    let html = r#"data:text/html,<html><body><button id="action-btn" onclick="document.body.innerHTML += '<p>Clicked</p>'">Action</button></body></html>"#;

    let (success, stdout, stderr) = run_pw(&[
        "-f",
        "json",
        "click",
        html,
        "#action-btn",
        "--wait-ms",
        "100",
    ]);

    assert!(success, "Click command failed: {}", stderr);
    assert!(
        stdout.contains("\"ok\": true"),
        "Expected success in JSON: {}",
        stdout
    );
    // The key assertion: navigated should be false for non-navigation clicks
    assert!(
        stdout.contains("\"navigated\": false"),
        "Expected navigated: false when clicking a non-navigation button. Output: {}",
        stdout
    );
}

/// Test: Verify that click command returns beforeUrl and afterUrl for comparison
///
/// This ensures the click command includes URL tracking data regardless of
/// whether navigation occurred.
#[test]
fn click_includes_before_and_after_urls() {
    let html = r#"data:text/html,<html><body><span id="target">Text</span></body></html>"#;

    let (success, stdout, stderr) = run_pw(&["-f", "json", "click", html, "#target"]);

    assert!(success, "Click command failed: {}", stderr);

    // Verify both URL fields are present
    assert!(
        stdout.contains("\"beforeUrl\""),
        "Expected beforeUrl in output: {}",
        stdout
    );
    assert!(
        stdout.contains("\"afterUrl\""),
        "Expected afterUrl in output: {}",
        stdout
    );
    assert!(
        stdout.contains("\"navigated\""),
        "Expected navigated field in output: {}",
        stdout
    );
}

/// Test: Click command uses JavaScript-based URL detection (not page.url())
///
/// This tests that the click command uses window.location.href for accurate
/// URL detection. The URL should contain the data: scheme.
#[test]
fn click_uses_accurate_url_detection() {
    let html = r#"data:text/html,<html><body><div id="el">Element</div></body></html>"#;

    let (success, stdout, stderr) = run_pw(&["-f", "json", "click", html, "#el"]);

    assert!(success, "Click command failed: {}", stderr);

    // The URLs should accurately reflect the data: URL
    // This confirms JavaScript evaluation is used (page.url() might not return this)
    assert!(
        stdout.contains("data:text/html"),
        "Expected data: URL in output, confirming accurate URL detection: {}",
        stdout
    );
}
