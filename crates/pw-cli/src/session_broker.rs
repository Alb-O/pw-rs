use std::fs;
use std::path::{Path, PathBuf};

use crate::browser::BrowserSession;
use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::types::BrowserKind;
use pw::{StorageState, WaitUntil};
use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SessionDescriptor {
    pub(crate) pid: u32,
    pub(crate) browser: BrowserKind,
    pub(crate) headless: bool,
    pub(crate) cdp_endpoint: Option<String>,
    pub(crate) created_at: u64,
}

impl SessionDescriptor {
    pub(crate) fn load(path: &Path) -> Result<Option<Self>> {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(PwError::Io(err)),
        };

        let parsed: Self = serde_json::from_str(&content)?;
        Ok(Some(parsed))
    }

    pub(crate) fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub(crate) fn matches(&self, request: &SessionRequest<'_>) -> bool {
        self.browser == request.browser
            && self.headless == request.headless
            && self.cdp_endpoint.is_some()
            && self.cdp_endpoint.as_deref() == request.cdp_endpoint
    }

    pub(crate) fn is_alive(&self) -> bool {
        // Best-effort: on Linux, check /proc; otherwise assume alive if pid matches current process
        let proc_path = PathBuf::from("/proc").join(self.pid.to_string());
        proc_path.exists()
    }
}

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

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
    descriptor_path: Option<PathBuf>,
    refresh: bool,
}

impl<'a> SessionBroker<'a> {
    pub fn new(ctx: &'a CommandContext, descriptor_path: Option<PathBuf>, refresh: bool) -> Self {
        Self {
            ctx,
            descriptor_path,
            refresh,
        }
    }

    pub async fn session(&mut self, request: SessionRequest<'_>) -> Result<SessionHandle> {
        let storage_state = match request.auth_file {
            Some(path) => Some(load_storage_state(path)?),
            None => None,
        };

        if let Some(path) = &self.descriptor_path {
            if self.refresh {
                let _ = fs::remove_file(path);
            } else if let Some(descriptor) = SessionDescriptor::load(path)? {
                if descriptor.matches(&request) && descriptor.is_alive() {
                    if let Some(endpoint) = descriptor.cdp_endpoint.as_deref() {
                        debug!(
                            target = "pw.session",
                            %endpoint,
                            pid = descriptor.pid,
                            "reusing existing browser via cdp"
                        );
                        let session = BrowserSession::with_options(
                            request.wait_until,
                            storage_state.clone(),
                            request.headless,
                            request.browser,
                            Some(endpoint),
                        )
                        .await?;
                        return Ok(SessionHandle { session });
                    } else {
                        debug!(
                            target = "pw.session",
                            "descriptor lacks cdp endpoint; ignoring"
                        );
                    }
                }
            }
        }

        let session = BrowserSession::with_options(
            request.wait_until,
            storage_state,
            request.headless,
            request.browser,
            request.cdp_endpoint,
        )
        .await?;

        if let (Some(path), Some(endpoint)) = (&self.descriptor_path, request.cdp_endpoint) {
            let descriptor = SessionDescriptor {
                pid: std::process::id(),
                browser: request.browser,
                headless: request.headless,
                cdp_endpoint: Some(endpoint.to_string()),
                created_at: now_ts(),
            };
            let _ = descriptor.save(path);
        } else if self.descriptor_path.is_some() {
            debug!(
                target = "pw.session",
                "no cdp endpoint available; skipping descriptor save"
            );
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn descriptor_round_trip_and_match() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");

        let desc = SessionDescriptor {
            pid: std::process::id(),
            browser: BrowserKind::Chromium,
            headless: true,
            cdp_endpoint: Some("ws://localhost:1234".into()),
            created_at: 123,
        };

        desc.save(&path).unwrap();
        let loaded = SessionDescriptor::load(&path).unwrap().unwrap();
        assert!(loaded.is_alive());

        let req = SessionRequest {
            wait_until: WaitUntil::NetworkIdle,
            headless: true,
            auth_file: None,
            browser: BrowserKind::Chromium,
            cdp_endpoint: Some("ws://localhost:1234"),
        };
        assert!(loaded.matches(&req));
    }

    #[test]
    fn descriptor_mismatch_when_endpoint_differs() {
        let desc = SessionDescriptor {
            pid: std::process::id(),
            browser: BrowserKind::Chromium,
            headless: true,
            cdp_endpoint: Some("ws://localhost:9999".into()),
            created_at: 0,
        };

        let req = SessionRequest {
            wait_until: WaitUntil::NetworkIdle,
            headless: true,
            auth_file: None,
            browser: BrowserKind::Chromium,
            cdp_endpoint: Some("ws://localhost:1234"),
        };

        assert!(!desc.matches(&req));
    }
}
