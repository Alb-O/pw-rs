// Copyright 2024 Paul Adamson
// Licensed under the Apache License, Version 2.0
//
// BrowserType - Represents a browser type (Chromium, Firefox, WebKit)
//
// Reference:
// - Python: playwright-python/playwright/_impl/_browser_type.py
// - Protocol: protocol.yml (BrowserType interface)

use crate::LaunchOptions;
use crate::{Browser, BrowserContext};
use pw_runtime::Result;
use pw_runtime::channel::Channel;
use pw_runtime::channel_owner::{ChannelOwner, ChannelOwnerImpl, ParentOrConnection};
use pw_runtime::connection::ConnectionLike;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// BrowserType represents a browser engine (Chromium, Firefox, or WebKit).
///
/// Each Playwright instance provides three BrowserType objects accessible via:
/// - `playwright.chromium()`
/// - `playwright.firefox()`
/// - `playwright.webkit()`
///
/// # Example
///
/// ```ignore
/// # use pw::protocol::Playwright;
/// # use pw::api::LaunchOptions;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let playwright = Playwright::launch().await?;
/// let chromium = playwright.chromium();
///
/// // Verify browser type info
/// assert_eq!(chromium.name(), "chromium");
/// assert!(!chromium.executable_path().is_empty());
///
/// // Launch with default options
/// let browser1 = chromium.launch().await?;
/// assert_eq!(browser1.name(), "chromium");
/// assert!(!browser1.version().is_empty());
/// browser1.close().await?;
///
/// // Launch with custom options
/// let options = LaunchOptions::default()
///     .headless(true)
///     .slow_mo(100.0)
///     .args(vec!["--no-sandbox".to_string()]);
///
/// let browser2 = chromium.launch_with_options(options).await?;
/// assert_eq!(browser2.name(), "chromium");
/// assert!(!browser2.version().is_empty());
/// browser2.close().await?;
/// # Ok(())
/// # }
/// ```
///
/// See: <https://playwright.dev/docs/api/class-browsertype>
pub struct BrowserType {
    /// Base ChannelOwner implementation
    base: ChannelOwnerImpl,
    /// Browser name ("chromium", "firefox", or "webkit")
    name: String,
    /// Path to browser executable
    executable_path: String,
}

impl BrowserType {
    /// Creates a new BrowserType object from protocol initialization.
    ///
    /// Called by the object factory when server sends __create__ message.
    ///
    /// # Arguments
    /// * `parent` - Parent Playwright object
    /// * `type_name` - Protocol type name ("BrowserType")
    /// * `guid` - Unique GUID from server (e.g., "browserType@chromium")
    /// * `initializer` - Initial state with name and executablePath
    pub fn new(
        parent: Arc<dyn ChannelOwner>,
        type_name: String,
        guid: Arc<str>,
        initializer: Value,
    ) -> Result<Self> {
        let base = ChannelOwnerImpl::new(
            ParentOrConnection::Parent(parent),
            type_name,
            guid,
            initializer.clone(),
        );

        // Extract fields from initializer
        let name = initializer["name"]
            .as_str()
            .ok_or_else(|| {
                pw_runtime::Error::ProtocolError(
                    "BrowserType initializer missing 'name'".to_string(),
                )
            })?
            .to_string();

        let executable_path = initializer["executablePath"]
            .as_str()
            .ok_or_else(|| {
                pw_runtime::Error::ProtocolError(
                    "BrowserType initializer missing 'executablePath'".to_string(),
                )
            })?
            .to_string();

        Ok(Self {
            base,
            name,
            executable_path,
        })
    }

    /// Returns the browser name ("chromium", "firefox", or "webkit").
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the path to the browser executable.
    pub fn executable_path(&self) -> &str {
        &self.executable_path
    }

    /// Launches a browser instance with default options.
    ///
    /// This is equivalent to calling `launch_with_options(LaunchOptions::default())`.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Browser executable not found
    /// - Launch timeout (default 30s)
    /// - Browser process fails to start
    ///
    /// See: <https://playwright.dev/docs/api/class-browsertype#browser-type-launch>
    pub async fn launch(&self) -> Result<Browser> {
        self.launch_with_options(LaunchOptions::default()).await
    }

    /// Launches a browser instance with custom options.
    ///
    /// # Arguments
    ///
    /// * `options` - Launch options (headless, args, etc.)
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Browser executable not found
    /// - Launch timeout
    /// - Invalid options
    /// - Browser process fails to start
    ///
    /// See: <https://playwright.dev/docs/api/class-browsertype#browser-type-launch>
    pub async fn launch_with_options(&self, options: LaunchOptions) -> Result<Browser> {
        // Add Windows CI-specific browser args to prevent hanging
        let options = {
            #[cfg(windows)]
            {
                let mut options = options;
                // Check if we're in a CI environment (GitHub Actions, Jenkins, etc.)
                let is_ci = std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok();

                if is_ci {
                    tracing::info!("Detected Windows CI environment, adding stability flags");

                    // Get existing args or create empty vec
                    let mut args = options.args.unwrap_or_default();

                    // Add Windows CI stability flags if not already present
                    let ci_flags = vec![
                        "--no-sandbox",            // Disable sandboxing (often problematic in CI)
                        "--disable-dev-shm-usage", // Overcome limited /dev/shm resources
                        "--disable-gpu",           // Disable GPU hardware acceleration
                        "--disable-web-security",  // Avoid CORS issues in CI
                        "--disable-features=IsolateOrigins,site-per-process", // Reduce process overhead
                    ];

                    for flag in ci_flags {
                        if !args.iter().any(|a| a == flag) {
                            args.push(flag.to_string());
                        }
                    }

                    // Update options with enhanced args
                    options.args = Some(args);

                    // Increase timeout for Windows CI (slower startup)
                    if options.timeout.is_none() {
                        options.timeout = Some(60000.0); // 60 seconds for Windows CI
                    }
                }
                options
            }

            #[cfg(not(windows))]
            {
                options
            }
        };

        // Normalize options for protocol transmission
        let params = options.normalize();

        // Send launch RPC to server
        let response: LaunchResponse = self.base.channel().send("launch", params).await?;

        // Get browser object from registry
        let browser_arc = self.connection().get_object(&response.browser.guid).await?;

        // Downcast to Browser
        let browser = browser_arc.downcast_ref::<Browser>().ok_or_else(|| {
            pw_runtime::Error::ProtocolError(format!(
                "Expected Browser object, got {}",
                browser_arc.type_name()
            ))
        })?;

        Ok(browser.clone())
    }

    /// Launches a browser server and returns its websocket endpoint.
    pub async fn launch_server(&self) -> Result<LaunchedServer> {
        self.launch_server_with_options(LaunchOptions::default())
            .await
    }

    /// Launches a browser server with custom options and returns a handle.
    pub async fn launch_server_with_options(
        &self,
        options: LaunchOptions,
    ) -> Result<LaunchedServer> {
        #[derive(Deserialize)]
        struct LaunchServerResponse {
            #[serde(rename = "wsEndpoint")]
            ws_endpoint: String,
            browser: BrowserRef,
        }

        let params = options.normalize();
        let response: LaunchServerResponse = self.channel().send("launchServer", params).await?;

        let browser_arc = self.connection().get_object(&response.browser.guid).await?;
        let browser = browser_arc.downcast_ref::<Browser>().ok_or_else(|| {
            pw_runtime::Error::ProtocolError(format!(
                "Expected Browser object, got {}",
                browser_arc.type_name()
            ))
        })?;

        Ok(LaunchedServer {
            ws_endpoint: response.ws_endpoint,
            browser: browser.clone(),
        })
    }

    /// Connects to an existing browser over the Chrome DevTools Protocol.
    ///
    /// This keeps the standard Playwright driver in the loop while reusing a
    /// running GUI browser (for example, the extension relay). The returned
    /// default context, when present, represents the persistent browser profile.
    pub async fn connect_over_cdp(
        &self,
        endpoint_url: impl Into<String>,
    ) -> Result<ConnectOverCDPResult> {
        #[derive(Serialize)]
        struct ConnectParams {
            #[serde(rename = "endpointURL")]
            endpoint_url: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            timeout: Option<u64>,
        }

        #[derive(Deserialize)]
        struct ConnectResponse {
            browser: BrowserRef,
            #[serde(rename = "defaultContext")]
            default_context: Option<BrowserContextRef>,
        }

        let params = ConnectParams {
            endpoint_url: endpoint_url.into(),
            timeout: Some(30000),
        };

        let params_json = serde_json::to_value(params).map_err(|e| {
            pw_runtime::Error::ProtocolError(format!(
                "Failed to serialize connectOverCDP params: {}",
                e
            ))
        })?;

        let response: ConnectResponse = self.channel().send("connectOverCDP", params_json).await?;

        let browser_arc = self.connection().get_object(&response.browser.guid).await?;
        let browser = browser_arc.downcast_ref::<Browser>().ok_or_else(|| {
            pw_runtime::Error::ProtocolError(format!(
                "Expected Browser object, got {}",
                browser_arc.type_name()
            ))
        })?;

        let default_context = if let Some(ctx_ref) = response.default_context {
            let ctx_arc = self.connection().get_object(&ctx_ref.guid).await?;
            let ctx = ctx_arc.downcast_ref::<BrowserContext>().ok_or_else(|| {
                pw_runtime::Error::ProtocolError(format!(
                    "Expected BrowserContext object, got {}",
                    ctx_arc.type_name()
                ))
            })?;
            Some(ctx.clone())
        } else {
            None
        };

        Ok(ConnectOverCDPResult {
            browser: browser.clone(),
            default_context,
        })
    }
}

/// Response from BrowserType.launch() protocol call
#[derive(Debug, Deserialize, Serialize)]
struct LaunchResponse {
    browser: BrowserRef,
}

/// Reference to a Browser object in the protocol
#[derive(Debug, Deserialize, Serialize)]
struct BrowserRef {
    #[serde(
        serialize_with = "pw_runtime::connection::serialize_arc_str",
        deserialize_with = "pw_runtime::connection::deserialize_arc_str"
    )]
    guid: Arc<str>,
}

/// Reference to a BrowserContext returned by connectOverCDP
#[derive(Debug, Deserialize, Serialize)]
struct BrowserContextRef {
    #[serde(
        serialize_with = "pw_runtime::connection::serialize_arc_str",
        deserialize_with = "pw_runtime::connection::deserialize_arc_str"
    )]
    guid: Arc<str>,
}

/// Result of connecting to an existing browser via CDP
#[derive(Clone, Debug)]
pub struct ConnectOverCDPResult {
    pub browser: Browser,
    pub default_context: Option<BrowserContext>,
}

#[derive(Clone, Debug)]
pub struct LaunchedServer {
    ws_endpoint: String,
    browser: Browser,
}

impl LaunchedServer {
    pub fn ws_endpoint(&self) -> &str {
        &self.ws_endpoint
    }

    pub fn browser(&self) -> &Browser {
        &self.browser
    }

    pub async fn close(self) -> Result<()> {
        self.browser.close().await
    }
}

impl pw_runtime::channel_owner::private::Sealed for BrowserType {}

impl ChannelOwner for BrowserType {
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

impl std::fmt::Debug for BrowserType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserType")
            .field("guid", &self.guid())
            .field("name", &self.name)
            .field("executable_path", &self.executable_path)
            .finish()
    }
}

// Note: BrowserType testing is done via integration tests since it requires:
// - A real Connection with object registry
// - Protocol messages from the server
// See: crates/playwright-core/tests/connection_integration.rs
