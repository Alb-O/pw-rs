use pw::{BrowserContextOptions, GotoOptions, Playwright, StorageState, WaitUntil};
use std::path::Path;
use tracing::debug;

use crate::error::{PwError, Result};

pub struct BrowserSession {
    _playwright: Playwright,
    browser: pw::protocol::Browser,
    context: pw::protocol::BrowserContext,
    page: pw::protocol::Page,
    wait_until: WaitUntil,
}

impl BrowserSession {
    pub async fn new(wait_until: WaitUntil) -> Result<Self> {
        Self::with_options(wait_until, None, true).await
    }

    /// Create a session with optional auth file (convenience for commands)
    pub async fn with_auth(wait_until: WaitUntil, auth_file: Option<&Path>) -> Result<Self> {
        match auth_file {
            Some(path) => Self::with_auth_file(wait_until, path).await,
            None => Self::new(wait_until).await,
        }
    }

    /// Create a new session with optional storage state and headless mode
    pub async fn with_options(
        wait_until: WaitUntil,
        storage_state: Option<StorageState>,
        headless: bool,
    ) -> Result<Self> {
        debug!(target = "pw", "starting Playwright...");
        let playwright = Playwright::launch()
            .await
            .map_err(|e| PwError::BrowserLaunch(e.to_string()))?;

        let launch_options = pw::LaunchOptions {
            headless: Some(headless),
            ..Default::default()
        };

        let browser = playwright
            .chromium()
            .launch_with_options(launch_options)
            .await?;

        // Create context with optional storage state
        let context = if let Some(state) = storage_state {
            let options = BrowserContextOptions::builder()
                .storage_state(state)
                .build();
            browser.new_context_with_options(options).await?
        } else {
            browser.new_context().await?
        };

        let page = context.new_page().await?;

        Ok(Self {
            _playwright: playwright,
            browser,
            context,
            page,
            wait_until,
        })
    }

    /// Create a session with auth loaded from a file
    pub async fn with_auth_file(wait_until: WaitUntil, auth_file: &Path) -> Result<Self> {
        let storage_state = StorageState::from_file(auth_file).map_err(|e| {
            PwError::BrowserLaunch(format!("Failed to load auth file: {}", e))
        })?;
        Self::with_options(wait_until, Some(storage_state), true).await
    }

    pub async fn goto(&self, url: &str) -> Result<()> {
        let goto_opts = GotoOptions {
            wait_until: Some(self.wait_until),
            ..Default::default()
        };

        self.page
            .goto(url, Some(goto_opts))
            .await
            .map(|_| ())
            .map_err(|e| PwError::Navigation {
                url: url.to_string(),
                source: anyhow::Error::new(e),
            })
    }

    pub fn page(&self) -> &pw::protocol::Page {
        &self.page
    }

    pub fn context(&self) -> &pw::protocol::BrowserContext {
        &self.context
    }

    pub async fn close(self) -> Result<()> {
        self.browser.close().await?;
        Ok(())
    }
}
