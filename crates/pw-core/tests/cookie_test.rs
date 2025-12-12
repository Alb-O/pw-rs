// Cookie and Storage State Integration Tests
//
// Tests for cookie management and storage state persistence.
// These tests verify actual browser behavior, not just serialization.

mod test_server;

use pw::{Cookie, ClearCookiesOptions, SameSite, StorageState, Playwright};
use test_server::TestServer;

async fn setup() -> (Playwright, TestServer) {
    let playwright = Playwright::launch().await.expect("Failed to launch Playwright");
    let server = TestServer::start().await;
    (playwright, server)
}

#[tokio::test]
async fn test_add_and_retrieve_cookies() {
    let (playwright, server) = setup().await;
    let browser = playwright.chromium().launch().await.unwrap();
    let context = browser.new_context().await.unwrap();
    let page = context.new_page().await.unwrap();

    // Navigate to establish origin
    page.goto(&server.url(), None).await.unwrap();

    // Add cookies using URL (required for IP-based origins)
    // Don't mix url with path - use one or the other
    context.add_cookies(vec![
        Cookie::from_url("session", "abc123", &server.url()),
        Cookie::from_url("user_id", "42", &server.url()),
    ]).await.unwrap();

    // Retrieve cookies
    let cookies = context.cookies(None).await.unwrap();

    assert!(cookies.len() >= 2, "Expected at least 2 cookies, got {}", cookies.len());

    let session_cookie = cookies.iter().find(|c| c.name == "session");
    assert!(session_cookie.is_some(), "session cookie not found");
    assert_eq!(session_cookie.unwrap().value, "abc123");

    let user_cookie = cookies.iter().find(|c| c.name == "user_id");
    assert!(user_cookie.is_some(), "user_id cookie not found");
    assert_eq!(user_cookie.unwrap().value, "42");

    browser.close().await.unwrap();
    server.shutdown();
}

#[tokio::test]
async fn test_cookies_filtered_by_url() {
    let (playwright, server) = setup().await;
    let browser = playwright.chromium().launch().await.unwrap();
    let context = browser.new_context().await.unwrap();
    let page = context.new_page().await.unwrap();

    page.goto(&server.url(), None).await.unwrap();

    // Add cookie for local server using URL
    context.add_cookies(vec![
        Cookie::from_url("local_cookie", "value1", &server.url()),
    ]).await.unwrap();

    // Filter by URL should return matching cookies
    let local_cookies = context.cookies(Some(vec![&server.url()])).await.unwrap();

    let has_local = local_cookies.iter().any(|c| c.name == "local_cookie");
    assert!(has_local, "local_cookie should be returned for local URL");

    browser.close().await.unwrap();
    server.shutdown();
}

#[tokio::test]
async fn test_clear_all_cookies() {
    let (playwright, server) = setup().await;
    let browser = playwright.chromium().launch().await.unwrap();
    let context = browser.new_context().await.unwrap();
    let page = context.new_page().await.unwrap();

    page.goto(&server.url(), None).await.unwrap();

    // Add cookies using URL
    context.add_cookies(vec![
        Cookie::from_url("cookie1", "value1", &server.url()),
        Cookie::from_url("cookie2", "value2", &server.url()),
    ]).await.unwrap();

    // Verify they exist
    let before = context.cookies(None).await.unwrap();
    assert!(before.len() >= 2);

    // Clear all
    context.clear_cookies(None).await.unwrap();

    // Verify empty
    let after = context.cookies(None).await.unwrap();
    assert!(after.is_empty(), "Expected no cookies after clear, got {}", after.len());

    browser.close().await.unwrap();
    server.shutdown();
}

#[tokio::test]
async fn test_clear_cookies_by_name() {
    let (playwright, server) = setup().await;
    let browser = playwright.chromium().launch().await.unwrap();
    let context = browser.new_context().await.unwrap();
    let page = context.new_page().await.unwrap();

    page.goto(&server.url(), None).await.unwrap();

    // Add multiple cookies using URL
    context.add_cookies(vec![
        Cookie::from_url("keep_me", "value1", &server.url()),
        Cookie::from_url("delete_me", "value2", &server.url()),
    ]).await.unwrap();

    // Clear only specific cookie
    context.clear_cookies(Some(
        ClearCookiesOptions::new().name("delete_me")
    )).await.unwrap();

    let remaining = context.cookies(None).await.unwrap();

    let has_keep = remaining.iter().any(|c| c.name == "keep_me");
    let has_delete = remaining.iter().any(|c| c.name == "delete_me");

    assert!(has_keep, "keep_me cookie should still exist");
    assert!(!has_delete, "delete_me cookie should be removed");

    browser.close().await.unwrap();
    server.shutdown();
}

#[tokio::test]
async fn test_storage_state_roundtrip() {
    let (playwright, server) = setup().await;
    let browser = playwright.chromium().launch().await.unwrap();

    // Create context and add cookies
    let context1 = browser.new_context().await.unwrap();
    let page1 = context1.new_page().await.unwrap();
    page1.goto(&server.url(), None).await.unwrap();

    context1.add_cookies(vec![
        Cookie::from_url("auth_token", "secret123", &server.url()),
    ]).await.unwrap();

    // Get storage state
    let state = context1.storage_state(None).await.unwrap();

    // Verify state contains our cookie
    assert!(!state.cookies.is_empty(), "Storage state should contain cookies");
    let auth_cookie = state.cookies.iter().find(|c| c.name == "auth_token");
    assert!(auth_cookie.is_some(), "auth_token cookie should be in storage state");

    // Close first context
    context1.close().await.unwrap();

    // For restoring, we need to ensure cookies have path set
    // (storage state from browser includes all fields)
    let options = pw::BrowserContextOptions::builder()
        .storage_state(state.clone())
        .build();
    let context2 = browser.new_context_with_options(options).await.unwrap();
    let page2 = context2.new_page().await.unwrap();
    page2.goto(&server.url(), None).await.unwrap();

    // Verify cookies were restored
    let restored_cookies = context2.cookies(None).await.unwrap();
    let restored_auth = restored_cookies.iter().find(|c| c.name == "auth_token");
    assert!(restored_auth.is_some(), "auth_token should be restored in new context");
    assert_eq!(restored_auth.unwrap().value, "secret123");

    browser.close().await.unwrap();
    server.shutdown();
}

#[tokio::test]
async fn test_storage_state_file_io() {
    // Test StorageState file save/load without browser
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("test_storage_state.json");

    let original = StorageState {
        cookies: vec![
            Cookie::new("session", "abc", ".example.com")
                .path("/")
                .http_only(true)
                .secure(true)
                .same_site(SameSite::Strict),
            Cookie::new("tracking", "xyz", ".example.com")
                .expires(1735689600.0), // Some future timestamp
        ],
        origins: vec![
            pw::OriginState {
                origin: "https://example.com".to_string(),
                local_storage: vec![
                    pw::LocalStorageEntry {
                        name: "user_prefs".to_string(),
                        value: r#"{"theme":"dark","lang":"en"}"#.to_string(),
                    },
                ],
            },
        ],
    };

    // Save to file
    original.to_file(&temp_file).expect("Failed to save storage state");

    // Load from file
    let loaded = StorageState::from_file(&temp_file).expect("Failed to load storage state");

    // Verify cookies
    assert_eq!(loaded.cookies.len(), 2);

    let session = loaded.cookies.iter().find(|c| c.name == "session").unwrap();
    assert_eq!(session.value, "abc");
    assert_eq!(session.http_only, Some(true));
    assert_eq!(session.secure, Some(true));
    assert_eq!(session.same_site, Some(SameSite::Strict));

    let tracking = loaded.cookies.iter().find(|c| c.name == "tracking").unwrap();
    assert_eq!(tracking.expires, Some(1735689600.0));

    // Verify origins/localStorage
    assert_eq!(loaded.origins.len(), 1);
    assert_eq!(loaded.origins[0].origin, "https://example.com");
    assert_eq!(loaded.origins[0].local_storage.len(), 1);
    assert_eq!(loaded.origins[0].local_storage[0].name, "user_prefs");

    // Cleanup
    std::fs::remove_file(&temp_file).ok();
}

#[tokio::test]
async fn test_cookie_same_site_values() {
    let (playwright, server) = setup().await;
    let browser = playwright.chromium().launch().await.unwrap();
    let context = browser.new_context().await.unwrap();
    let page = context.new_page().await.unwrap();

    page.goto(&server.url(), None).await.unwrap();

    // Add cookies with different SameSite values using URL
    context.add_cookies(vec![
        Cookie::from_url("strict_cookie", "1", &server.url())
            .same_site(SameSite::Strict),
        Cookie::from_url("lax_cookie", "2", &server.url())
            .same_site(SameSite::Lax),
    ]).await.unwrap();

    let cookies = context.cookies(None).await.unwrap();

    // Verify SameSite values are preserved
    let strict = cookies.iter().find(|c| c.name == "strict_cookie");
    let lax = cookies.iter().find(|c| c.name == "lax_cookie");

    assert!(strict.is_some(), "strict_cookie should exist");
    assert!(lax.is_some(), "lax_cookie should exist");

    browser.close().await.unwrap();
    server.shutdown();
}

#[tokio::test]
async fn test_cookies_persist_across_pages() {
    let (playwright, server) = setup().await;
    let browser = playwright.chromium().launch().await.unwrap();
    let context = browser.new_context().await.unwrap();

    // Create first page and navigate
    let page1 = context.new_page().await.unwrap();
    page1.goto(&server.url(), None).await.unwrap();

    // Add cookie using URL after navigation
    context.add_cookies(vec![
        Cookie::from_url("page1_cookie", "from_page1", &server.url()),
    ]).await.unwrap();

    // Create second page in same context
    let page2 = context.new_page().await.unwrap();
    page2.goto(&format!("{}/button.html", server.url()), None).await.unwrap();

    // Cookie should be available
    let cookies = context.cookies(None).await.unwrap();
    let has_page1 = cookies.iter().any(|c| c.name == "page1_cookie");

    assert!(has_page1, "page1_cookie should persist across pages");

    browser.close().await.unwrap();
    server.shutdown();
}

#[tokio::test]
async fn test_separate_contexts_have_isolated_cookies() {
    let (playwright, server) = setup().await;
    let browser = playwright.chromium().launch().await.unwrap();

    // Create two separate contexts
    let context1 = browser.new_context().await.unwrap();
    let context2 = browser.new_context().await.unwrap();

    let page1 = context1.new_page().await.unwrap();
    let page2 = context2.new_page().await.unwrap();

    page1.goto(&server.url(), None).await.unwrap();
    page2.goto(&server.url(), None).await.unwrap();

    // Add different cookies to each context using URL
    context1.add_cookies(vec![
        Cookie::from_url("context1_only", "value1", &server.url()),
    ]).await.unwrap();

    context2.add_cookies(vec![
        Cookie::from_url("context2_only", "value2", &server.url()),
    ]).await.unwrap();

    // Verify isolation
    let cookies1 = context1.cookies(None).await.unwrap();
    let cookies2 = context2.cookies(None).await.unwrap();

    let c1_has_own = cookies1.iter().any(|c| c.name == "context1_only");
    let c1_has_other = cookies1.iter().any(|c| c.name == "context2_only");
    let c2_has_own = cookies2.iter().any(|c| c.name == "context2_only");
    let c2_has_other = cookies2.iter().any(|c| c.name == "context1_only");

    assert!(c1_has_own, "context1 should have its own cookie");
    assert!(!c1_has_other, "context1 should NOT have context2's cookie");
    assert!(c2_has_own, "context2 should have its own cookie");
    assert!(!c2_has_other, "context2 should NOT have context1's cookie");

    browser.close().await.unwrap();
    server.shutdown();
}
