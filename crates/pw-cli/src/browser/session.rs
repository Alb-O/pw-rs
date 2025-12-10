use pw::{GotoOptions, Playwright, WaitUntil};
use tracing::debug;

use crate::error::{PwError, Result};

pub struct BrowserSession {
    _playwright: Playwright,
    browser: pw::protocol::Browser,
    page: pw::protocol::Page,
    wait_until: WaitUntil,
}

impl BrowserSession {
    pub async fn new(wait_until: WaitUntil) -> Result<Self> {
        debug!(target = "pw", "starting Playwright...");
        let playwright = Playwright::launch()
            .await
            .map_err(|e| PwError::BrowserLaunch(e.to_string()))?;
        let browser = playwright.chromium().launch().await?;
        let page = browser.new_page().await?;

        Ok(Self {
            _playwright: playwright,
            browser,
            page,
            wait_until,
        })
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

    pub async fn close(self) -> Result<()> {
        self.browser.close().await?;
        Ok(())
    }
}
