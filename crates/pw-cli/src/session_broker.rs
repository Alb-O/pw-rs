use std::path::Path;

use crate::browser::BrowserSession;
use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::types::BrowserKind;
use pw::{StorageState, WaitUntil};

/// Request for a browser session; future reuse/daemon logic will live here.
pub struct SessionRequest<'a> {
    pub wait_until: WaitUntil,
    pub headless: bool,
    pub auth_file: Option<&'a Path>,
    pub browser: BrowserKind,
    pub cdp_endpoint: Option<&'a str>,
}

impl<'a> SessionRequest<'a> {
    pub fn from_context(wait_until: WaitUntil, ctx: &'a CommandContext) -> Self {
        Self {
            wait_until,
            headless: true,
            auth_file: ctx.auth_file(),
            browser: ctx.browser,
            cdp_endpoint: ctx.cdp_endpoint(),
        }
    }

    pub fn with_headless(mut self, headless: bool) -> Self {
        self.headless = headless;
        self
    }

    pub fn with_auth_file(mut self, auth_file: Option<&'a Path>) -> Self {
        self.auth_file = auth_file;
        self
    }

    pub fn with_browser(mut self, browser: BrowserKind) -> Self {
        self.browser = browser;
        self
    }

    pub fn with_cdp_endpoint(mut self, endpoint: Option<&'a str>) -> Self {
        self.cdp_endpoint = endpoint;
        self
    }
}

pub struct SessionBroker<'a> {
    ctx: &'a CommandContext,
}

impl<'a> SessionBroker<'a> {
    pub fn new(ctx: &'a CommandContext) -> Self {
        Self { ctx }
    }

    pub async fn session(&mut self, request: SessionRequest<'_>) -> Result<SessionHandle> {
        // TODO: reuse live sessions when available.
        let storage_state = match request.auth_file {
            Some(path) => Some(load_storage_state(path)?),
            None => None,
        };

        let session = BrowserSession::with_options(
            request.wait_until,
            storage_state,
            request.headless,
            request.browser,
            request.cdp_endpoint,
        )
        .await?;

        Ok(SessionHandle { session })
    }

    pub fn context(&self) -> &'a CommandContext {
        self.ctx
    }
}

pub struct SessionHandle {
    session: BrowserSession,
}

impl SessionHandle {
    pub async fn goto(&self, url: &str) -> Result<()> {
        self.session.goto(url).await
    }

    pub fn page(&self) -> &pw::protocol::Page {
        self.session.page()
    }

    pub fn context(&self) -> &pw::protocol::BrowserContext {
        self.session.context()
    }

    pub async fn close(self) -> Result<()> {
        self.session.close().await
    }
}

fn load_storage_state(path: &Path) -> Result<StorageState> {
    StorageState::from_file(path)
        .map_err(|e| PwError::BrowserLaunch(format!("Failed to load auth file: {}", e)))
}
