//! playwright: High-level Rust bindings for Microsoft Playwright
//!
//! This crate provides the public API for browser automation using Playwright.
//!
//! # Examples
//!
//! ## Basic Navigation and Interaction
//!
//! ```ignore
//! use pw::{Playwright, SelectOption};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let playwright = Playwright::launch().await?;
//!     let browser = playwright.chromium().launch().await?;
//!     let page = browser.new_page().await?;
//!
//!     // Navigate using data URL for self-contained test
//!     let _ = page.goto(
//!         "data:text/html,<html><body>\
//!             <h1 id='title'>Welcome</h1>\
//!             <button id='btn' onclick='this.textContent=\"Clicked\"'>Click me</button>\
//!         </body></html>",
//!         None
//!     ).await;
//!
//!     // Query elements with locators
//!     let heading = page.locator("#title").await;
//!     let text = heading.text_content().await?;
//!     assert_eq!(text, Some("Welcome".to_string()));
//!
//!     // Click button and verify result
//!     let button = page.locator("#btn").await;
//!     button.click(None).await?;
//!     let button_text = button.text_content().await?;
//!     assert_eq!(button_text, Some("Clicked".to_string()));
//!
//!     browser.close().await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Form Interaction
//!
//! ```ignore
//! use pw::{Playwright, SelectOption};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let playwright = Playwright::launch().await?;
//!     let browser = playwright.chromium().launch().await?;
//!     let page = browser.new_page().await?;
//!
//!     // Create form with data URL
//!     let _ = page.goto(
//!         "data:text/html,<html><body>\
//!             <input type='text' id='name' />\
//!             <input type='checkbox' id='agree' />\
//!             <select id='country'>\
//!                 <option value='us'>USA</option>\
//!                 <option value='uk'>UK</option>\
//!                 <option value='ca'>Canada</option>\
//!             </select>\
//!         </body></html>",
//!         None
//!     ).await;
//!
//!     // Fill text input
//!     let name = page.locator("#name").await;
//!     name.fill("John Doe", None).await?;
//!     assert_eq!(name.input_value(None).await?, "John Doe");
//!
//!     // Check checkbox
//!     let checkbox = page.locator("#agree").await;
//!     checkbox.set_checked(true, None).await?;
//!     assert!(checkbox.is_checked().await?);
//!
//!     // Select option
//!     let select = page.locator("#country").await;
//!     select.select_option("uk", None).await?;
//!     assert_eq!(select.input_value(None).await?, "uk");
//!
//!     browser.close().await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Element Screenshots
//!
//! ```ignore
//! use pw::Playwright;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let playwright = Playwright::launch().await?;
//!     let browser = playwright.chromium().launch().await?;
//!     let page = browser.new_page().await?;
//!
//!     // Create element to screenshot
//!     let _ = page.goto(
//!         "data:text/html,<html><body>\
//!             <div id='box' style='width:100px;height:100px;background:blue'></div>\
//!         </body></html>",
//!         None
//!     ).await;
//!
//!     // Take screenshot of specific element
//!     let element = page.locator("#box").await;
//!     let screenshot = element.screenshot(None).await?;
//!     assert!(!screenshot.is_empty());
//!
//!     browser.close().await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Assertions (expect API)
//!
//! ```ignore
//! use pw::{expect, Playwright};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let playwright = Playwright::launch().await?;
//!     let browser = playwright.chromium().launch().await?;
//!     let page = browser.new_page().await?;
//!
//!     let _ = page.goto(
//!         "data:text/html,<html><body>\
//!             <button id='enabled'>Enabled</button>\
//!             <button id='disabled' disabled>Disabled</button>\
//!             <input type='checkbox' id='checked' checked />\
//!         </body></html>",
//!         None
//!     ).await;
//!
//!     // Assert button states with auto-retry
//!     let enabled_btn = page.locator("#enabled").await;
//!     expect(enabled_btn.clone()).to_be_enabled().await?;
//!
//!     let disabled_btn = page.locator("#disabled").await;
//!     expect(disabled_btn).to_be_disabled().await?;
//!
//!     // Assert checkbox state
//!     let checkbox = page.locator("#checked").await;
//!     expect(checkbox).to_be_checked().await?;
//!
//!     browser.close().await?;
//!     Ok(())
//! }
//! ```

mod assertions;
mod init;
mod object_factory;

pub mod accessibility;
pub mod action_options;
pub mod artifact;
pub mod browser;
pub mod browser_context;
pub mod browser_type;
pub mod click;
pub mod cookie;
pub mod dialog;
pub mod download;
pub mod element_handle;
pub mod events;
pub mod file_payload;
pub mod frame;
pub mod keyboard;
pub mod launch_options;
pub mod locator;
pub mod mouse;
pub mod page;
pub mod playwright;
pub mod request;
pub mod response;
pub mod root;
pub mod route;
pub mod screenshot;
pub mod select_option;
pub mod tracing;
pub mod video;

pub use accessibility::{
    Accessibility, AccessibilityNode, AccessibilitySnapshotOptions,
    AccessibilitySnapshotOptionsBuilder, AccessibilityValue, CheckedState, PressedState,
};
pub use action_options::{
    CheckOptions, FillOptions, HoverOptions, KeyboardOptions, MouseOptions, PressOptions,
    SelectOptions,
};
pub use browser::Browser;
pub use browser_context::{
    BrowserContext, BrowserContextOptions, BrowserContextOptionsBuilder, Geolocation,
    HarContentPolicy, HarMode, HarNotFound, HarStartOptions, RouteFromHarOptions, Viewport,
};
pub use browser_type::{BrowserType, ConnectOverCDPResult, LaunchedServer};
pub use click::{ClickOptions, KeyboardModifier, MouseButton, Position};
pub use cookie::{
    ClearCookiesOptions, Cookie, LocalStorageEntry, OriginState, SameSite, StorageState,
    StorageStateOptions,
};
pub use dialog::Dialog;
pub use download::Download;
pub use element_handle::ElementHandle;
pub use events::{ConsoleSubscription, EventStream, EventWaiter};
pub use file_payload::{FilePayload, FilePayloadBuilder};
pub use frame::Frame;
pub use keyboard::Keyboard;
pub use launch_options::{IgnoreDefaultArgs, LaunchOptions, ProxySettings};

// Re-export initialization function
pub use init::initialize_playwright;

// Re-export assertions
pub use assertions::{Expectation, expect};
pub use locator::Locator;
pub use mouse::Mouse;
pub use page::{
    ConsoleLocation, ConsoleMessage, ConsoleMessageKind, GotoOptions, Page, Response, Subscription,
    WaitUntil,
};
pub use playwright::Playwright;
pub use request::Request;
pub use response::ResponseObject;
pub use root::Root;
pub use route::{
    ContinueOptions, ContinueOptionsBuilder, FulfillOptions, FulfillOptionsBuilder, Route,
};
pub use screenshot::{ScreenshotClip, ScreenshotOptions, ScreenshotType};
pub use select_option::SelectOption;
pub use tracing::{
    Tracing, TracingStartChunkOptions, TracingStartOptions, TracingStartOptionsBuilder,
    TracingStopOptions,
};
pub use video::Video;

/// Default timeout in milliseconds for Playwright operations.
///
/// This matches Playwright's standard default across all language implementations (Python, Java, .NET, JS).
/// Required in Playwright 1.56.1+ when timeout parameter is not explicitly provided.
///
/// See: <https://playwright.dev/docs/test-timeouts>
pub const DEFAULT_TIMEOUT_MS: f64 = pw_protocol::options::DEFAULT_TIMEOUT_MS;

// Re-export pw-protocol types for convenience
pub use pw_protocol;

// Re-export pw-runtime for internal use
pub use pw_runtime;

// Re-export Error and Result from pw-runtime
pub use pw_runtime::{Error, Result};

/// Directory name constants for playwright project structure.
///
/// These match the scaffold structure created by `pw init` and are used
/// consistently across build.rs and runtime code.
pub mod dirs {
    /// Main playwright directory under project root (contains tests, auth, screenshots, etc.)
    pub const PLAYWRIGHT: &str = "playwright";
    /// Drivers directory name (where playwright driver is downloaded)
    pub const DRIVERS: &str = "drivers";
    /// Tests directory name (inside playwright/)
    pub const TESTS: &str = "tests";
    /// Results/output directory name (inside playwright/)
    pub const RESULTS: &str = "results";
    /// Screenshots directory name (inside playwright/)
    pub const SCREENSHOTS: &str = "screenshots";
    /// Auth state directory name (inside playwright/)
    pub const AUTH: &str = "auth";
    /// Reports directory name (inside playwright/)
    pub const REPORTS: &str = "reports";
    /// Scripts directory name (inside playwright/)
    pub const SCRIPTS: &str = "scripts";
    /// Browsers directory name (inside playwright/, for Nix browser symlinks)
    pub const BROWSERS: &str = "browsers";

    /// JavaScript config file name
    pub const CONFIG_JS: &str = "playwright.config.js";
    /// TypeScript config file name
    pub const CONFIG_TS: &str = "playwright.config.ts";
}
