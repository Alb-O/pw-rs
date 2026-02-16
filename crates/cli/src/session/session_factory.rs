//! Browser session acquisition helpers extracted from session manager orchestration.

use std::path::Path;

use pw_rs::StorageState;
use tracing::debug;

use super::daemon_lease::DaemonLease;
use super::descriptor::{DRIVER_HASH, SessionDescriptor};
use super::outcome::SessionHandle;
use super::spec::SessionRequest;
use super::strategy::PrimarySessionStrategy;
use crate::browser::{BrowserSession, SessionConfig, ShutdownMode};
use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::output::SessionSource;
use crate::types::BrowserKind;

/// Session creation helper that owns browser-launch and attach mechanics.
pub(super) struct SessionFactory<'a> {
	ctx: &'a CommandContext,
}

impl<'a> SessionFactory<'a> {
	/// Creates a helper bound to immutable command context.
	pub(super) fn new(ctx: &'a CommandContext) -> Self {
		Self { ctx }
	}

	/// Loads storage-state used by session acquisition requests.
	pub(super) fn load_storage_state(path: &Path) -> Result<StorageState> {
		StorageState::from_file(path).map_err(|e| PwError::BrowserLaunch(format!("Failed to load auth file: {}", e)))
	}

	/// Attempts session reuse from a descriptor when metadata still matches request constraints.
	pub(super) async fn acquire_from_descriptor(
		&self,
		descriptor: &SessionDescriptor,
		request: &SessionRequest<'_>,
		storage_state: Option<StorageState>,
	) -> Result<Option<SessionHandle>> {
		if !(descriptor.belongs_to(self.ctx)
			&& descriptor.matches(request.browser, request.headless, request.cdp_endpoint, Some(DRIVER_HASH))
			&& descriptor.is_alive())
		{
			return Ok(None);
		}

		let Some(endpoint) = descriptor.cdp_endpoint.as_deref().or(descriptor.ws_endpoint.as_deref()) else {
			debug!(target = "pw.session", "descriptor lacks endpoint; ignoring");
			return Ok(None);
		};

		debug!(
			target = "pw.session",
			%endpoint,
			pid = descriptor.pid,
			"reusing existing browser via cdp"
		);

		let mut session = self.session_with_config(request, storage_state, Some(endpoint)).await?;
		session.set_shutdown_mode(ShutdownMode::KeepBrowserAlive);

		Ok(Some(SessionHandle {
			session,
			source: SessionSource::CachedDescriptor,
		}))
	}

	/// Acquires a new or attached browser session based on the selected primary strategy.
	pub(super) async fn acquire_primary(
		&self,
		request: &SessionRequest<'_>,
		primary: PrimarySessionStrategy,
		storage_state: Option<StorageState>,
		daemon_lease: Option<&DaemonLease>,
	) -> Result<(BrowserSession, SessionSource)> {
		if let Some(lease) = daemon_lease {
			let mut session = self.session_with_config(request, storage_state.clone(), Some(lease.endpoint.as_str())).await?;
			session.set_shutdown_mode(ShutdownMode::KeepBrowserAlive);
			return Ok((session, SessionSource::Daemon));
		}

		match primary {
			PrimarySessionStrategy::AttachCdp => {
				let endpoint = request
					.cdp_endpoint
					.ok_or_else(|| PwError::Context("missing CDP endpoint for attach strategy".to_string()))?;
				let mut session = self.session_with_config(request, storage_state, Some(endpoint)).await?;
				session.set_shutdown_mode(ShutdownMode::KeepBrowserAlive);
				Ok((session, SessionSource::CdpConnect))
			}
			PrimarySessionStrategy::PersistentDebug => {
				let port = request
					.remote_debugging_port
					.ok_or_else(|| PwError::Context("missing remote_debugging_port for persistent strategy".to_string()))?;
				if request.browser != BrowserKind::Chromium {
					return Err(PwError::BrowserLaunch(
						"Persistent sessions with remote_debugging_port require Chromium".to_string(),
					));
				}
				let session =
					BrowserSession::launch_persistent(request.wait_until, storage_state, request.headless, port, request.keep_browser_running).await?;
				Ok((session, SessionSource::PersistentDebug))
			}
			PrimarySessionStrategy::LaunchServer => {
				let session = BrowserSession::launch_server_session(request.wait_until, storage_state, request.headless, request.browser).await?;
				Ok((session, SessionSource::BrowserServer))
			}
			PrimarySessionStrategy::FreshLaunch => {
				let session = self.session_with_config(request, storage_state, None).await?;
				Ok((session, SessionSource::Fresh))
			}
		}
	}

	/// Injects project auth files when attaching to existing browser endpoints.
	pub(super) async fn auto_inject_auth_if_needed(
		&self,
		request: &SessionRequest<'_>,
		daemon_lease: Option<&DaemonLease>,
		session: &mut BrowserSession,
	) -> Result<()> {
		let attached_endpoint = request.cdp_endpoint.is_some() || daemon_lease.is_some();
		if attached_endpoint && request.auth_file.is_none() {
			let auth_files = self.ctx.auth_files();
			if !auth_files.is_empty() {
				debug!(
					target = "pw.session",
					count = auth_files.len(),
					"auto-injecting cookies from project auth files"
				);
				let report = session.inject_auth_files(&auth_files).await?;
				debug!(
					target = "pw.session",
					files_seen = report.files_seen,
					files_loaded = report.files_loaded,
					cookies_added = report.cookies_added,
					"auth injection summary"
				);
			}
		}
		Ok(())
	}

	async fn session_with_config(
		&self,
		request: &SessionRequest<'_>,
		storage_state: Option<StorageState>,
		cdp_endpoint: Option<&str>,
	) -> Result<BrowserSession> {
		BrowserSession::with_config(SessionConfig {
			wait_until: request.wait_until,
			storage_state,
			headless: request.headless,
			browser_kind: request.browser,
			cdp_endpoint: cdp_endpoint.map(str::to_string),
			launch_server: false,
			protected_urls: request.protected_urls.to_vec(),
			preferred_url: request.preferred_url.map(str::to_string),
			har: request.har_config.clone(),
			block: request.block_config.clone(),
			download: request.download_config.clone(),
		})
		.await
	}
}
