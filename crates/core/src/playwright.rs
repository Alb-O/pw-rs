// Copyright 2024 Paul Adamson
// Licensed under the Apache License, Version 2.0
//
// Playwright - Root protocol object
//
// Reference:
// - Python: playwright-python/playwright/_impl/_playwright.py
// - Protocol: protocol.yml (Playwright interface)

use crate::BrowserType;
use parking_lot::Mutex;
use pw_runtime::PlaywrightServer;
use pw_runtime::Result;
use pw_runtime::channel::Channel;
use pw_runtime::channel_owner::{ChannelOwner, ChannelOwnerImpl, ParentOrConnection};
use pw_runtime::connection::ConnectionLike;
use serde_json::Value;
use std::sync::Arc;

/// Playwright is the root object that provides access to browser types.
///
/// This is the main entry point for the Playwright API. It provides access to
/// the three browser types (Chromium, Firefox, WebKit) and other top-level services.
///
/// # Example
///
/// ```ignore
/// use pw::protocol::Playwright;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Launch Playwright server and initialize
///     let playwright = Playwright::launch().await?;
///
///     // Verify all three browser types are available
///     let chromium = playwright.chromium();
///     let firefox = playwright.firefox();
///     let webkit = playwright.webkit();
///
///     assert_eq!(chromium.name(), "chromium");
///     assert_eq!(firefox.name(), "firefox");
///     assert_eq!(webkit.name(), "webkit");
///
///     // Verify we can launch a browser
///     let browser = chromium.launch().await?;
///     assert!(!browser.version().is_empty());
///     browser.close().await?;
///
///     // Shutdown when done
///     playwright.shutdown().await?;
///
///     Ok(())
/// }
/// ```
///
/// See: <https://playwright.dev/docs/api/class-playwright>
pub struct Playwright {
    /// Base ChannelOwner implementation
    base: ChannelOwnerImpl,
    /// Chromium browser type (stored as `Arc<dyn ChannelOwner>`, downcast on access)
    chromium: Arc<dyn ChannelOwner>,
    /// Firefox browser type (stored as `Arc<dyn ChannelOwner>`, downcast on access)
    firefox: Arc<dyn ChannelOwner>,
    /// WebKit browser type (stored as `Arc<dyn ChannelOwner>`, downcast on access)
    webkit: Arc<dyn ChannelOwner>,
    /// Playwright server process (for clean shutdown)
    ///
    /// Stored as `Option<PlaywrightServer>` wrapped in Arc<Mutex<>> to allow:
    /// - Sharing across clones (Arc)
    /// - Taking ownership during shutdown (Option::take)
    /// - Interior mutability (Mutex)
    server: Arc<Mutex<Option<PlaywrightServer>>>,
    /// Whether to keep the launched server running when Playwright is dropped
    keep_server_running: bool,
    /// Whether this Playwright instance owns the server process
    owns_server: bool,
}

impl Playwright {
    /// Launches Playwright and returns a handle to interact with browser types.
    ///
    /// This is the main entry point for the Playwright API. It will:
    /// 1. Launch the Playwright server process
    /// 2. Establish a connection via stdio
    /// 3. Initialize the protocol
    /// 4. Return a Playwright instance with access to browser types
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Playwright server is not found or fails to launch
    /// - Connection to server fails
    /// - Protocol initialization fails
    /// - Server doesn't respond within timeout (30s)
    pub async fn launch() -> Result<Self> {
        use pw_runtime::PipeTransport;
        use pw_runtime::PlaywrightServer;
        use pw_runtime::connection::Connection;

        // 1. Launch Playwright server
        tracing::debug!("Launching Playwright server");
        let mut server = PlaywrightServer::launch().await?;

        // 2. Take stdio streams from server process
        let stdin = server.process.stdin.take().ok_or_else(|| {
            pw_runtime::Error::ServerError("Failed to get server stdin".to_string())
        })?;

        let stdout = server.process.stdout.take().ok_or_else(|| {
            pw_runtime::Error::ServerError("Failed to get server stdout".to_string())
        })?;

        // 3. Create transport and connection
        tracing::debug!("Creating transport and connection");
        let (transport, message_rx) = PipeTransport::new(stdin, stdout);
        let parts = transport.into_transport_parts(message_rx);
        let connection: Arc<Connection> = Arc::new(Connection::new(parts));

        // 4. Spawn connection message loop in background
        let conn_for_loop: Arc<Connection> = Arc::clone(&connection);
        tokio::spawn(async move {
            conn_for_loop.run().await;
        });

        // 5. Initialize Playwright (sends initialize message, waits for Playwright object)
        tracing::debug!("Initializing Playwright protocol");
        let playwright_obj = crate::initialize_playwright(&connection).await?;

        // 6. Downcast to Playwright type
        let playwright = playwright_obj.downcast_ref::<Playwright>().ok_or_else(|| {
            pw_runtime::Error::ProtocolError(
                "Initialized object is not Playwright type".to_string(),
            )
        })?;

        // Clone the Playwright object to return it
        // Note: We need to own the Playwright, not just borrow it
        // Since we only have &Playwright from downcast_ref, we need to extract the data
        Ok(Self {
            base: playwright.base.clone(),
            chromium: Arc::clone(&playwright.chromium),
            firefox: Arc::clone(&playwright.firefox),
            webkit: Arc::clone(&playwright.webkit),
            server: Arc::new(Mutex::new(Some(server))),
            keep_server_running: false,
            owns_server: true,
        })
    }

    /// Connect to a running Playwright driver over WebSocket.
    pub async fn connect_ws(ws_url: &str) -> Result<Self> {
        use pw_runtime::WebSocketTransport;
        use pw_runtime::connection::Connection;

        tracing::debug!(%ws_url, "Connecting to Playwright driver via websocket");
        let (transport, message_rx) = WebSocketTransport::connect(ws_url).await?;
        let parts = transport.into_transport_parts(message_rx);
        let connection: Arc<Connection> = Arc::new(Connection::new(parts));

        let conn_for_loop: Arc<Connection> = Arc::clone(&connection);
        tokio::spawn(async move {
            conn_for_loop.run().await;
        });

        let playwright_obj = crate::initialize_playwright(&connection).await?;

        let playwright = playwright_obj.downcast_ref::<Playwright>().ok_or_else(|| {
            pw_runtime::Error::ProtocolError(
                "Initialized object is not Playwright type".to_string(),
            )
        })?;

        Ok(Self {
            base: playwright.base.clone(),
            chromium: Arc::clone(&playwright.chromium),
            firefox: Arc::clone(&playwright.firefox),
            webkit: Arc::clone(&playwright.webkit),
            server: Arc::new(Mutex::new(None)),
            keep_server_running: false,
            owns_server: false,
        })
    }

    /// Connect to a running Playwright driver on localhost.
    pub async fn connect_daemon(port: u16) -> Result<Self> {
        Self::connect_ws(&format!("ws://127.0.0.1:{}", port)).await
    }

    /// Creates a new Playwright object from protocol initialization.
    ///
    /// Called by the object factory when server sends __create__ message for root object.
    ///
    /// # Arguments
    /// * `connection` - The connection (Playwright is root, so no parent)
    /// * `type_name` - Protocol type name ("Playwright")
    /// * `guid` - Unique GUID from server (typically "playwright@1")
    /// * `initializer` - Initial state with references to browser types
    ///
    /// # Initializer Format
    ///
    /// The initializer contains GUID references to BrowserType objects:
    /// ```json
    /// {
    ///   "chromium": { "guid": "browserType@chromium" },
    ///   "firefox": { "guid": "browserType@firefox" },
    ///   "webkit": { "guid": "browserType@webkit" }
    /// }
    /// ```
    pub async fn new(
        connection: Arc<dyn ConnectionLike>,
        type_name: String,
        guid: Arc<str>,
        initializer: Value,
    ) -> Result<Self> {
        let base = ChannelOwnerImpl::new(
            ParentOrConnection::Connection(connection.clone()),
            type_name,
            guid,
            initializer.clone(),
        );

        // Extract BrowserType GUIDs from initializer
        let chromium_guid = initializer["chromium"]["guid"].as_str().ok_or_else(|| {
            pw_runtime::Error::ProtocolError(
                "Playwright initializer missing 'chromium.guid'".to_string(),
            )
        })?;

        let firefox_guid = initializer["firefox"]["guid"].as_str().ok_or_else(|| {
            pw_runtime::Error::ProtocolError(
                "Playwright initializer missing 'firefox.guid'".to_string(),
            )
        })?;

        let webkit_guid = initializer["webkit"]["guid"].as_str().ok_or_else(|| {
            pw_runtime::Error::ProtocolError(
                "Playwright initializer missing 'webkit.guid'".to_string(),
            )
        })?;

        // Get BrowserType objects from connection registry
        // Note: These objects should already exist (created by earlier __create__ messages)
        // We store them as Arc<dyn ChannelOwner> and downcast when accessed
        let chromium = connection.get_object(chromium_guid).await?;
        let firefox = connection.get_object(firefox_guid).await?;
        let webkit = connection.get_object(webkit_guid).await?;

        Ok(Self {
            base,
            chromium,
            firefox,
            webkit,
            server: Arc::new(Mutex::new(None)), // No server for protocol-created objects
            keep_server_running: false,
            owns_server: false,
        })
    }

    /// Returns the Chromium browser type.
    pub fn chromium(&self) -> &BrowserType {
        // Downcast from Arc<dyn ChannelOwner> to &BrowserType
        self.chromium
            .downcast_ref::<BrowserType>()
            .expect("chromium should be BrowserType")
    }

    /// Returns the Firefox browser type.
    pub fn firefox(&self) -> &BrowserType {
        self.firefox
            .downcast_ref::<BrowserType>()
            .expect("firefox should be BrowserType")
    }

    /// Returns the WebKit browser type.
    pub fn webkit(&self) -> &BrowserType {
        self.webkit
            .downcast_ref::<BrowserType>()
            .expect("webkit should be BrowserType")
    }

    /// Allow the launched Playwright server to keep running after this handle is dropped.
    pub fn keep_server_running(&mut self) {
        self.keep_server_running = true;
    }

    /// Re-enable automatic server shutdown on drop (default behavior).
    pub fn enable_server_shutdown(&mut self) {
        self.keep_server_running = false;
    }

    /// Shuts down the Playwright server gracefully.
    ///
    /// This method should be called when you're done using Playwright to ensure
    /// the server process is terminated cleanly, especially on Windows.
    ///
    /// # Platform-Specific Behavior
    ///
    /// **Windows**: Closes stdio pipes before shutting down to prevent hangs.
    ///
    /// **Unix**: Standard graceful shutdown.
    ///
    /// # Errors
    ///
    /// Returns an error if the server shutdown fails.
    pub async fn shutdown(&self) -> Result<()> {
        // Take server from mutex without holding the lock across await
        let server = self.server.lock().take();
        if let Some(server) = server {
            tracing::debug!("Shutting down Playwright server");
            server.shutdown().await?;
        }
        Ok(())
    }
}

impl pw_runtime::channel_owner::private::Sealed for Playwright {}

impl ChannelOwner for Playwright {
    fn guid(&self) -> &str {
        self.base.guid()
    }

    fn type_name(&self) -> &str {
        self.base.type_name()
    }

    fn parent(&self) -> Option<Arc<dyn ChannelOwner>> {
        self.base.parent()
    }

    fn connection(&self) -> Arc<dyn ConnectionLike> {
        self.base.connection()
    }

    fn initializer(&self) -> &Value {
        self.base.initializer()
    }

    fn channel(&self) -> &Channel {
        self.base.channel()
    }

    fn dispose(&self, reason: pw_runtime::channel_owner::DisposeReason) {
        self.base.dispose(reason)
    }

    fn adopt(&self, child: Arc<dyn ChannelOwner>) {
        self.base.adopt(child)
    }

    fn add_child(&self, guid: Arc<str>, child: Arc<dyn ChannelOwner>) {
        self.base.add_child(guid, child)
    }

    fn remove_child(&self, guid: &str) {
        self.base.remove_child(guid)
    }

    fn on_event(&self, method: &str, params: Value) {
        self.base.on_event(method, params)
    }

    fn was_collected(&self) -> bool {
        self.base.was_collected()
    }
}

impl Drop for Playwright {
    /// Ensures Playwright server is shut down when Playwright is dropped.
    ///
    /// This is critical on Windows to prevent process hangs when tests complete.
    /// The Drop implementation will attempt to kill the server process synchronously.
    ///
    /// Note: For graceful shutdown, prefer calling `playwright.shutdown().await`
    /// explicitly before dropping.
    fn drop(&mut self) {
        if self.keep_server_running || !self.owns_server {
            return;
        }

        if let Some(mut server) = self.server.lock().take() {
            tracing::debug!("Drop: Force-killing Playwright server");

            // We can't call async shutdown in Drop, so use blocking kill
            // This is less graceful but ensures the process terminates
            #[cfg(windows)]
            {
                // On Windows: Close stdio pipes before killing
                drop(server.process.stdin.take());
                drop(server.process.stdout.take());
                drop(server.process.stderr.take());
            }

            // Force kill the process
            if let Err(e) = server.process.start_kill() {
                tracing::warn!("Failed to kill Playwright server in Drop: {}", e);
            }
        }
    }
}

impl std::fmt::Debug for Playwright {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Playwright")
            .field("guid", &self.guid())
            .field("chromium", &self.chromium().name())
            .field("firefox", &self.firefox().name())
            .field("webkit", &self.webkit().name())
            .finish()
    }
}

// Note: Playwright testing is done via integration tests since it requires:
// - A real Connection with object registry
// - BrowserType objects already created and registered
// - Protocol messages from the server
// See: crates/playwright-core/tests/connection_integration.rs
