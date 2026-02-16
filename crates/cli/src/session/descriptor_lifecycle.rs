//! Descriptor persistence/read/clear orchestration for session management.

use std::path::Path;

use serde_json::json;
use tracing::{debug, info, warn};

use super::daemon_lease::DaemonLease;
use super::descriptor::{DRIVER_HASH, SESSION_DESCRIPTOR_SCHEMA_VERSION, SessionDescriptor, now_ts};
use super::repository::SessionRepository;
use super::spec::SessionRequest;
use crate::browser::BrowserSession;
use crate::context::CommandContext;
use crate::error::Result;

/// Descriptor lifecycle helper used by [`super::manager::SessionManager`].
pub(super) struct DescriptorLifecycle<'a> {
	ctx: &'a CommandContext,
	repository: &'a SessionRepository,
}

impl<'a> DescriptorLifecycle<'a> {
	/// Creates a lifecycle helper bound to the command context and repository.
	pub(super) fn new(ctx: &'a CommandContext, repository: &'a SessionRepository) -> Self {
		Self { ctx, repository }
	}

	/// Returns descriptor path when persistence is enabled.
	pub(super) fn path(&self) -> Option<&Path> {
		self.repository.path()
	}

	/// Loads descriptor metadata from persistence.
	pub(super) fn load(&self) -> Result<Option<SessionDescriptor>> {
		self.repository.load()
	}

	/// Clears descriptor metadata from persistence.
	pub(super) fn clear(&self) -> Result<bool> {
		self.repository.clear()
	}

	/// Returns the structured payload used by `session.status`.
	pub(super) fn status_payload(&self) -> Result<serde_json::Value> {
		let Some(path) = self.path().map(Path::to_path_buf) else {
			return Ok(json!({
				"active": false,
				"message": "No active namespace; session status unavailable"
			}));
		};

		match self.load()? {
			Some(desc) => {
				let alive = desc.is_alive();
				Ok(json!({
					"active": true,
					"path": path,
					"schema_version": desc.schema_version,
					"browser": desc.browser,
					"headless": desc.headless,
					"cdp_endpoint": desc.cdp_endpoint,
					"ws_endpoint": desc.ws_endpoint,
					"workspace_id": desc.workspace_id,
					"namespace": desc.namespace,
					"session_key": desc.session_key,
					"driver_hash": desc.driver_hash,
					"pid": desc.pid,
					"created_at": desc.created_at,
					"alive": alive,
				}))
			}
			None => Ok(json!({
				"active": false,
				"message": "No session descriptor for namespace; run a browser command to create one"
			})),
		}
	}

	/// Removes descriptor metadata and returns the structured payload for `session.clear`.
	pub(super) fn clear_payload(&self) -> Result<serde_json::Value> {
		let Some(path) = self.path().map(Path::to_path_buf) else {
			return Ok(json!({
				"cleared": false,
				"message": "No active namespace; nothing to clear"
			}));
		};

		if self.clear()? {
			info!(target = "pw.session", path = %path.display(), "session descriptor removed");
			Ok(json!({
				"cleared": true,
				"path": path,
			}))
		} else {
			warn!(target = "pw.session", path = %path.display(), "no session descriptor to remove");
			Ok(json!({
				"cleared": false,
				"path": path,
				"message": "No session descriptor found"
			}))
		}
	}

	/// Persists descriptor metadata for a newly acquired session.
	pub(super) fn persist_for_session(&self, request: &SessionRequest<'_>, session: &BrowserSession, daemon_lease: Option<&DaemonLease>) {
		if self.path().is_none() {
			return;
		}

		let endpoints = session.endpoints();
		let cdp = endpoints.cdp.clone();
		let ws = endpoints.ws.clone();
		if cdp.is_none() && ws.is_none() {
			debug!(target = "pw.session", "no endpoint available; skipping descriptor save");
			return;
		}

		let descriptor = SessionDescriptor {
			schema_version: SESSION_DESCRIPTOR_SCHEMA_VERSION,
			pid: std::process::id(),
			browser: request.browser,
			headless: request.headless,
			cdp_endpoint: cdp,
			ws_endpoint: ws,
			workspace_id: Some(self.ctx.workspace_id().to_string()),
			namespace: Some(self.ctx.namespace().to_string()),
			session_key: daemon_lease
				.map(|lease| lease.session_key.clone())
				.or_else(|| Some(self.ctx.session_key(request.browser, request.headless))),
			driver_hash: Some(DRIVER_HASH.to_string()),
			created_at: now_ts(),
		};

		if let Err(err) = self.repository.save(&descriptor) {
			if let Some(path) = self.path() {
				warn!(
					target = "pw.session",
					path = %path.display(),
					error = %err,
					"failed to save session descriptor"
				);
			} else {
				warn!(target = "pw.session", error = %err, "failed to save session descriptor");
			}
		} else {
			debug!(
				target = "pw.session",
				cdp = ?descriptor.cdp_endpoint,
				ws = ?descriptor.ws_endpoint,
				"saved session descriptor"
			);
		}
	}
}
