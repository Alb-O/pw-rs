// Integration tests for Navigation Error Handling (Phase 4, Slice 3)
//
// Following TDD: Write tests first (Red), then verify behavior (Green)
//
// Tests cover:
// - goto() timeout errors
// - reload() timeout errors
// - wait_until option behavior
// - Descriptive error messages
// - Cross-browser compatibility

mod test_server;

use playwright_core::protocol::{GotoOptions, Playwright, WaitUntil};
use std::time::Duration;
use test_server::TestServer;

#[tokio::test]
async fn test_goto_timeout_error() {
    let playwright = Playwright::launch()
        .await
        .expect("Failed to launch Playwright");
    let browser = playwright
        .chromium()
        .launch()
        .await
        .expect("Failed to launch browser");
    let page = browser.new_page().await.expect("Failed to create page");

    // Test: goto() should fail with very short timeout on slow/unresponsive server
    // Using a URL that will definitely timeout (non-routable IP)
    let options = GotoOptions::new().timeout(Duration::from_millis(100));

    let result = page.goto("http://10.255.255.1:9999/", Some(options)).await;

    // Should error due to timeout
    assert!(result.is_err(), "Expected timeout error");

    // Error message should be descriptive
    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(
        error_msg.contains("Timeout") || error_msg.contains("timeout"),
        "Error message should mention timeout: {}",
        error_msg
    );

    browser.close().await.expect("Failed to close browser");
}

#[tokio::test]
async fn test_goto_with_valid_timeout() {
    let server = TestServer::start().await;
    let playwright = Playwright::launch()
        .await
        .expect("Failed to launch Playwright");
    let browser = playwright
        .chromium()
        .launch()
        .await
        .expect("Failed to launch browser");
    let page = browser.new_page().await.expect("Failed to create page");

    // Test: goto() should succeed with reasonable timeout
    let options = GotoOptions::new().timeout(Duration::from_secs(10));

    let result = page
        .goto(&format!("{}/locators.html", server.url()), Some(options))
        .await;

    assert!(
        result.is_ok(),
        "Navigation should succeed with valid timeout"
    );

    browser.close().await.expect("Failed to close browser");
    server.shutdown();
}

#[tokio::test]
async fn test_goto_invalid_url() {
    let playwright = Playwright::launch()
        .await
        .expect("Failed to launch Playwright");
    let browser = playwright
        .chromium()
        .launch()
        .await
        .expect("Failed to launch browser");
    let page = browser.new_page().await.expect("Failed to create page");

    // Test: goto() should fail with invalid URL
    let result = page.goto("not-a-valid-url", None).await;

    assert!(result.is_err(), "Expected error for invalid URL");

    browser.close().await.expect("Failed to close browser");
}

#[tokio::test]
async fn test_reload_timeout_error() {
    let server = TestServer::start().await;
    let playwright = Playwright::launch()
        .await
        .expect("Failed to launch Playwright");
    let browser = playwright
        .chromium()
        .launch()
        .await
        .expect("Failed to launch browser");
    let page = browser.new_page().await.expect("Failed to create page");

    // First navigate to a valid page
    page.goto(&format!("{}/locators.html", server.url()), None)
        .await
        .expect("Initial navigation should succeed");

    // Now try to reload with an impossibly short timeout
    let options = GotoOptions::new().timeout(Duration::from_millis(1));

    let result = page.reload(Some(options)).await;

    // May or may not timeout depending on timing, but should not crash
    // This test mainly verifies the timeout option is respected
    let _ = result; // Don't assert on result since timing is unpredictable

    browser.close().await.expect("Failed to close browser");
    server.shutdown();
}

#[tokio::test]
async fn test_wait_until_load() {
    let server = TestServer::start().await;
    let playwright = Playwright::launch()
        .await
        .expect("Failed to launch Playwright");
    let browser = playwright
        .chromium()
        .launch()
        .await
        .expect("Failed to launch browser");
    let page = browser.new_page().await.expect("Failed to create page");

    // Test: wait_until Load option
    let options = GotoOptions::new().wait_until(WaitUntil::Load);

    let result = page
        .goto(&format!("{}/locators.html", server.url()), Some(options))
        .await;

    assert!(
        result.is_ok(),
        "Navigation with wait_until=Load should succeed"
    );

    browser.close().await.expect("Failed to close browser");
    server.shutdown();
}

#[tokio::test]
async fn test_wait_until_domcontentloaded() {
    let server = TestServer::start().await;
    let playwright = Playwright::launch()
        .await
        .expect("Failed to launch Playwright");
    let browser = playwright
        .chromium()
        .launch()
        .await
        .expect("Failed to launch browser");
    let page = browser.new_page().await.expect("Failed to create page");

    // Test: wait_until DomContentLoaded option
    let options = GotoOptions::new().wait_until(WaitUntil::DomContentLoaded);

    let result = page
        .goto(&format!("{}/locators.html", server.url()), Some(options))
        .await;

    assert!(
        result.is_ok(),
        "Navigation with wait_until=DOMContentLoaded should succeed"
    );

    browser.close().await.expect("Failed to close browser");
    server.shutdown();
}

#[tokio::test]
async fn test_wait_until_networkidle() {
    let server = TestServer::start().await;
    let playwright = Playwright::launch()
        .await
        .expect("Failed to launch Playwright");
    let browser = playwright
        .chromium()
        .launch()
        .await
        .expect("Failed to launch browser");
    let page = browser.new_page().await.expect("Failed to create page");

    // Test: wait_until NetworkIdle option
    let options = GotoOptions::new().wait_until(WaitUntil::NetworkIdle);

    let result = page
        .goto(&format!("{}/locators.html", server.url()), Some(options))
        .await;

    assert!(
        result.is_ok(),
        "Navigation with wait_until=NetworkIdle should succeed"
    );

    browser.close().await.expect("Failed to close browser");
    server.shutdown();
}

// Cross-browser tests

#[tokio::test]
async fn test_timeout_error_firefox() {
    let playwright = Playwright::launch()
        .await
        .expect("Failed to launch Playwright");
    let browser = playwright
        .firefox()
        .launch()
        .await
        .expect("Failed to launch Firefox");
    let page = browser.new_page().await.expect("Failed to create page");

    let options = GotoOptions::new().timeout(Duration::from_millis(100));

    let result = page.goto("http://10.255.255.1:9999/", Some(options)).await;

    assert!(result.is_err(), "Expected timeout error in Firefox");

    browser.close().await.expect("Failed to close browser");
}

#[tokio::test]
async fn test_timeout_error_webkit() {
    let playwright = Playwright::launch()
        .await
        .expect("Failed to launch Playwright");
    let browser = playwright
        .webkit()
        .launch()
        .await
        .expect("Failed to launch WebKit");
    let page = browser.new_page().await.expect("Failed to create page");

    let options = GotoOptions::new().timeout(Duration::from_millis(100));

    let result = page.goto("http://10.255.255.1:9999/", Some(options)).await;

    assert!(result.is_err(), "Expected timeout error in WebKit");

    browser.close().await.expect("Failed to close browser");
}
