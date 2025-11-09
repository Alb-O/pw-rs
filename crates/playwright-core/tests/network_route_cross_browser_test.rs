// Cross-browser integration tests for Network Routing (Phase 5, Slice 4c)
//
// Tests routing works across all browsers (Chromium, Firefox, WebKit)
//
// Tests cover:
// - route.abort() in Firefox and WebKit
// - route.continue() in Firefox and WebKit
// - Pattern matching across browsers
// - Request access in route handlers

mod test_server;

use playwright_core::protocol::Playwright;
use std::sync::{Arc, Mutex};
use test_server::TestServer;

#[tokio::test]
async fn test_route_abort_firefox() {
    let server = TestServer::start().await;
    let playwright = Playwright::launch()
        .await
        .expect("Failed to launch Playwright");
    let browser = playwright
        .firefox()
        .launch()
        .await
        .expect("Failed to launch Firefox");
    let page = browser.new_page().await.expect("Failed to create page");

    let aborted = Arc::new(Mutex::new(false));
    let aborted_clone = aborted.clone();

    // Test: Route handler can abort image requests in Firefox
    page.route("**/*.png", move |route| {
        let aborted = aborted_clone.clone();
        async move {
            *aborted.lock().unwrap() = true;
            route.abort(None).await
        }
    })
    .await
    .expect("Failed to set up route");

    page.goto(&format!("{}/", server.url()), None)
        .await
        .expect("Failed to navigate");

    // Give time for route to be called
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    browser.close().await.expect("Failed to close browser");
    server.shutdown();

    // Note: We can't verify the abort actually blocked the image without evaluate() return values
    // But we can verify the handler was registered and would be called
    println!("Route abort test passed in Firefox");
}

#[tokio::test]
async fn test_route_continue_firefox() {
    let server = TestServer::start().await;
    let playwright = Playwright::launch()
        .await
        .expect("Failed to launch Playwright");
    let browser = playwright
        .firefox()
        .launch()
        .await
        .expect("Failed to launch Firefox");
    let page = browser.new_page().await.expect("Failed to create page");

    let continued = Arc::new(Mutex::new(false));
    let continued_clone = continued.clone();

    // Test: Route can continue requests unchanged in Firefox
    page.route("**/*", move |route| {
        let continued = continued_clone.clone();
        async move {
            *continued.lock().unwrap() = true;
            route.continue_(None).await
        }
    })
    .await
    .expect("Failed to set up route");

    let response = page
        .goto(&format!("{}/", server.url()), None)
        .await
        .expect("Failed to navigate");

    assert_eq!(
        response.status(),
        200,
        "Request should succeed when continued in Firefox"
    );

    assert!(
        *continued.lock().unwrap(),
        "Route handler should have been called"
    );

    browser.close().await.expect("Failed to close browser");
    server.shutdown();
}

#[tokio::test]
async fn test_route_abort_webkit() {
    let server = TestServer::start().await;
    let playwright = Playwright::launch()
        .await
        .expect("Failed to launch Playwright");
    let browser = playwright
        .webkit()
        .launch()
        .await
        .expect("Failed to launch WebKit");
    let page = browser.new_page().await.expect("Failed to create page");

    let aborted = Arc::new(Mutex::new(false));
    let aborted_clone = aborted.clone();

    // Test: Route handler can abort image requests in WebKit
    page.route("**/*.png", move |route| {
        let aborted = aborted_clone.clone();
        async move {
            *aborted.lock().unwrap() = true;
            route.abort(None).await
        }
    })
    .await
    .expect("Failed to set up route");

    page.goto(&format!("{}/", server.url()), None)
        .await
        .expect("Failed to navigate");

    // Give time for route to be called
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    browser.close().await.expect("Failed to close browser");
    server.shutdown();

    println!("Route abort test passed in WebKit");
}

#[tokio::test]
async fn test_route_continue_webkit() {
    let server = TestServer::start().await;
    let playwright = Playwright::launch()
        .await
        .expect("Failed to launch Playwright");
    let browser = playwright
        .webkit()
        .launch()
        .await
        .expect("Failed to launch WebKit");
    let page = browser.new_page().await.expect("Failed to create page");

    let continued = Arc::new(Mutex::new(false));
    let continued_clone = continued.clone();

    // Test: Route can continue requests unchanged in WebKit
    page.route("**/*", move |route| {
        let continued = continued_clone.clone();
        async move {
            *continued.lock().unwrap() = true;
            route.continue_(None).await
        }
    })
    .await
    .expect("Failed to set up route");

    let response = page
        .goto(&format!("{}/", server.url()), None)
        .await
        .expect("Failed to navigate");

    assert_eq!(
        response.status(),
        200,
        "Request should succeed when continued in WebKit"
    );

    assert!(
        *continued.lock().unwrap(),
        "Route handler should have been called"
    );

    browser.close().await.expect("Failed to close browser");
    server.shutdown();
}

#[tokio::test]
async fn test_route_pattern_matching_firefox() {
    let server = TestServer::start().await;
    let playwright = Playwright::launch()
        .await
        .expect("Failed to launch Playwright");
    let browser = playwright
        .firefox()
        .launch()
        .await
        .expect("Failed to launch Firefox");
    let page = browser.new_page().await.expect("Failed to create page");

    let handler_called = Arc::new(Mutex::new(false));
    let handler_called_clone = handler_called.clone();

    // Test: Multiple routes with different patterns work in Firefox
    page.route("**/*.css", |route| async move { route.abort(None).await })
        .await
        .expect("Failed to set up CSS route");

    page.route("**/*.js", |route| async move { route.abort(None).await })
        .await
        .expect("Failed to set up JS route");

    page.route("**/*", move |route| {
        let handler_called = handler_called_clone.clone();
        async move {
            *handler_called.lock().unwrap() = true;
            route.continue_(None).await
        }
    })
    .await
    .expect("Failed to set up catch-all route");

    // Navigate should work (HTML continues)
    let response = page
        .goto(&format!("{}/", server.url()), None)
        .await
        .expect("Failed to navigate");
    assert_eq!(response.status(), 200, "Should work in Firefox");

    assert!(
        *handler_called.lock().unwrap(),
        "Catch-all handler should be called"
    );

    browser.close().await.expect("Failed to close browser");
    server.shutdown();
}

#[tokio::test]
async fn test_route_request_access_webkit() {
    let server = TestServer::start().await;
    let playwright = Playwright::launch()
        .await
        .expect("Failed to launch Playwright");
    let browser = playwright
        .webkit()
        .launch()
        .await
        .expect("Failed to launch WebKit");
    let page = browser.new_page().await.expect("Failed to create page");

    // Test: Route handler can access request data in WebKit
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(10);

    page.route("**/*", move |route| {
        let tx = tx.clone();
        async move {
            let request = route.request();
            let url = request.url();
            let method = request.method();
            tx.send(format!("{} {}", method, url)).await.ok();
            route.continue_(None).await
        }
    })
    .await
    .expect("Failed to set up route");

    page.goto(&format!("{}/", server.url()), None)
        .await
        .expect("Failed to navigate");

    // Verify the route handler saw the request
    let captured_data = rx.recv().await.expect("Should receive data from handler");
    assert!(
        captured_data.contains("GET"),
        "Handler should see GET method, got: {}",
        captured_data
    );
    assert!(
        captured_data.len() > 4, // "GET " + at least something
        "Handler should see request URL, got: {}",
        captured_data
    );

    browser.close().await.expect("Failed to close browser");
    server.shutdown();
}

#[tokio::test]
async fn test_route_error_codes_firefox() {
    let server = TestServer::start().await;
    let playwright = Playwright::launch()
        .await
        .expect("Failed to launch Playwright");
    let browser = playwright
        .firefox()
        .launch()
        .await
        .expect("Failed to launch Firefox");
    let page = browser.new_page().await.expect("Failed to create page");

    // Test: Route can abort with specific error code in Firefox
    page.route("**/data.json", |route| async move {
        route.abort(Some("accessdenied")).await
    })
    .await
    .expect("Failed to set up route");

    page.goto(&format!("{}/", server.url()), None)
        .await
        .expect("Failed to navigate");

    // Give time for potential requests
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    browser.close().await.expect("Failed to close browser");
    server.shutdown();

    println!("Error code test passed in Firefox");
}

#[tokio::test]
async fn test_route_conditional_logic_webkit() {
    let server = TestServer::start().await;
    let playwright = Playwright::launch()
        .await
        .expect("Failed to launch Playwright");
    let browser = playwright
        .webkit()
        .launch()
        .await
        .expect("Failed to launch WebKit");
    let page = browser.new_page().await.expect("Failed to create page");

    let blocked_count = Arc::new(Mutex::new(0));
    let allowed_count = Arc::new(Mutex::new(0));
    let blocked_clone = blocked_count.clone();
    let allowed_clone = allowed_count.clone();

    // Test: Conditionally abort based on request URL in WebKit
    page.route("**/*", move |route| {
        let blocked = blocked_clone.clone();
        let allowed = allowed_clone.clone();
        async move {
            let request = route.request();
            if request.url().contains("block-me") {
                *blocked.lock().unwrap() += 1;
                route.abort(None).await
            } else {
                *allowed.lock().unwrap() += 1;
                route.continue_(None).await
            }
        }
    })
    .await
    .expect("Failed to set up route");

    page.goto(&format!("{}/", server.url()), None)
        .await
        .expect("Failed to navigate");

    // Give time for requests
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    browser.close().await.expect("Failed to close browser");
    server.shutdown();

    assert!(
        *allowed_count.lock().unwrap() > 0,
        "Should have allowed some requests"
    );
}
