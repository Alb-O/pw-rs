//! Session descriptor persistence and validation.
//!
//! Descriptors cache reconnect metadata for profile-scoped browser reuse.

use std::fs;
use std::path::Path;

use pw_runtime::pid_is_alive;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::types::BrowserKind;
use crate::workspace::ensure_state_gitignore_for;

/// Driver build/version marker stored in session descriptors.
pub const DRIVER_HASH: &str = env!("CARGO_PKG_VERSION");
/// Current on-disk schema version for session descriptors.
pub const SESSION_DESCRIPTOR_SCHEMA_VERSION: u32 = 1;

fn session_descriptor_schema_version() -> u32 {
	SESSION_DESCRIPTOR_SCHEMA_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDescriptor {
	/// Descriptor schema version.
	#[serde(default = "session_descriptor_schema_version")]
	pub schema_version: u32,
	/// PID of the process that wrote this descriptor.
	pub pid: u32,
	/// Browser engine for the associated session.
	pub browser: BrowserKind,
	/// Whether the browser runs headless.
	pub headless: bool,
	/// CDP endpoint for reconnection when available.
	pub cdp_endpoint: Option<String>,
	/// WebSocket endpoint for reconnection when available.
	pub ws_endpoint: Option<String>,
	/// Workspace identity that owns this descriptor.
	pub workspace_id: Option<String>,
	/// Profile/namespace that owns this descriptor.
	pub namespace: Option<String>,
	/// Deterministic session key used for daemon/browser reuse.
	pub session_key: Option<String>,
	/// Driver hash written when the descriptor was created.
	pub driver_hash: Option<String>,
	/// Unix epoch seconds when the descriptor was created.
	pub created_at: u64,
}

impl SessionDescriptor {
	/// Loads a descriptor from disk, handling old/missing schema versions.
	pub fn load(path: &Path) -> Result<Option<Self>> {
		let content = match fs::read_to_string(path) {
			Ok(c) => c,
			Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
			Err(err) => return Err(PwError::Io(err)),
		};

		let value: serde_json::Value = serde_json::from_str(&content)?;
		let schema_version = value.get("schema_version").and_then(|v| v.as_u64()).unwrap_or(0);
		if schema_version == 0 {
			debug!(target = "pw.session", path = %path.display(), "removing v0 session descriptor without schema_version");
			let _ = fs::remove_file(path);
			return Ok(None);
		}
		if schema_version != SESSION_DESCRIPTOR_SCHEMA_VERSION as u64 {
			return Err(PwError::Context(format!(
				"unsupported session descriptor schema_version {schema_version} (expected {SESSION_DESCRIPTOR_SCHEMA_VERSION})"
			)));
		}

		let parsed: Self = serde_json::from_value(value)?;
		Ok(Some(parsed))
	}

	/// Saves a descriptor to disk and ensures state-root `.gitignore` exists.
	pub fn save(&self, path: &Path) -> Result<()> {
		ensure_state_gitignore_for(path)?;
		if let Some(parent) = path.parent() {
			fs::create_dir_all(parent)?;
		}
		let mut normalized = self.clone();
		normalized.schema_version = SESSION_DESCRIPTOR_SCHEMA_VERSION;
		let content = serde_json::to_string_pretty(&normalized)?;
		fs::write(path, content)?;
		Ok(())
	}

	/// Returns `true` when descriptor metadata matches the requested session.
	pub fn matches(&self, browser: BrowserKind, headless: bool, cdp_endpoint: Option<&str>, driver_hash: Option<&str>) -> bool {
		let endpoint_match = if let Some(req_endpoint) = cdp_endpoint {
			self.cdp_endpoint.as_deref() == Some(req_endpoint) || self.ws_endpoint.as_deref() == Some(req_endpoint)
		} else {
			self.ws_endpoint.is_some() || self.cdp_endpoint.is_some()
		};

		let driver_match = match (driver_hash, self.driver_hash.as_deref()) {
			(Some(expected), Some(actual)) => expected == actual,
			(None, _) => true,
			(_, None) => true,
		};

		self.browser == browser && self.headless == headless && endpoint_match && driver_match
	}

	/// Returns `true` when the descriptor PID appears alive.
	pub fn is_alive(&self) -> bool {
		pid_is_alive(self.pid)
	}

	/// Returns `true` when descriptor workspace/namespace match `ctx`.
	pub fn belongs_to(&self, ctx: &CommandContext) -> bool {
		let workspace_ok = match self.workspace_id.as_deref() {
			Some(v) => v == ctx.workspace_id(),
			None => false,
		};
		let namespace_ok = match self.namespace.as_deref() {
			Some(v) => v == ctx.namespace(),
			None => false,
		};
		workspace_ok && namespace_ok
	}
}

/// Current Unix timestamp in seconds.
pub fn now_ts() -> u64 {
	std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

#[cfg(test)]
mod tests {
	use tempfile::tempdir;

	use super::*;

	#[test]
	fn descriptor_without_schema_version_is_removed() {
		let dir = tempdir().unwrap();
		let path = dir.path().join("session.json");
		let descriptor = SessionDescriptor {
			schema_version: SESSION_DESCRIPTOR_SCHEMA_VERSION,
			pid: std::process::id(),
			browser: BrowserKind::Chromium,
			headless: true,
			cdp_endpoint: Some("ws://localhost:1234".into()),
			ws_endpoint: Some("ws://localhost:1234".into()),
			workspace_id: Some("ws".into()),
			namespace: Some("default".into()),
			session_key: Some("ws:default:chromium:headless".into()),
			driver_hash: Some(DRIVER_HASH.to_string()),
			created_at: 123,
		};
		let mut value = serde_json::to_value(descriptor).unwrap();
		value.as_object_mut().unwrap().remove("schema_version");
		std::fs::write(&path, serde_json::to_string(&value).unwrap()).unwrap();

		let loaded = SessionDescriptor::load(&path).unwrap();
		assert!(loaded.is_none());
		assert!(!path.exists());
	}

	#[test]
	fn descriptor_with_unknown_schema_version_errors() {
		let dir = tempdir().unwrap();
		let path = dir.path().join("session.json");
		let descriptor = SessionDescriptor {
			schema_version: 99,
			pid: std::process::id(),
			browser: BrowserKind::Chromium,
			headless: true,
			cdp_endpoint: Some("ws://localhost:1234".into()),
			ws_endpoint: Some("ws://localhost:1234".into()),
			workspace_id: Some("ws".into()),
			namespace: Some("default".into()),
			session_key: Some("ws:default:chromium:headless".into()),
			driver_hash: Some(DRIVER_HASH.to_string()),
			created_at: 123,
		};
		std::fs::write(&path, serde_json::to_string(&descriptor).unwrap()).unwrap();

		let err = SessionDescriptor::load(&path).unwrap_err();
		assert!(
			err.to_string().contains("unsupported session descriptor schema_version"),
			"unexpected error: {err}"
		);
	}
}
