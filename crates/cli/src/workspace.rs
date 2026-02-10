//! Workspace and namespace identity utilities.
//!
//! Defines deterministic identifiers used for strict session isolation.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use pw_rs::dirs;

use crate::error::{PwError, Result};
use crate::project::Project;
use crate::types::BrowserKind;

pub const DEFAULT_NAMESPACE: &str = "default";
pub const STATE_VERSION_DIR: &str = ".pw-cli-v3";

/// Canonical identity for a workspace namespace.
#[derive(Debug, Clone)]
pub struct WorkspaceScope {
	root: PathBuf,
	workspace_id: String,
	namespace: String,
}

impl WorkspaceScope {
	/// Resolve workspace root + namespace from CLI values.
	///
	/// - `workspace`: explicit workspace path, or `"auto"` for detection.
	/// - `namespace`: optional namespace; defaults to [`DEFAULT_NAMESPACE`].
	/// - `no_project`: when true, skip playwright project-root detection.
	pub fn resolve(
		workspace: Option<&str>,
		namespace: Option<&str>,
		no_project: bool,
	) -> Result<Self> {
		let namespace = normalize_namespace(namespace.unwrap_or(DEFAULT_NAMESPACE));
		let root = resolve_workspace_root(workspace, no_project)?;
		Ok(Self::from_parts(root, namespace))
	}

	pub fn from_parts(root: PathBuf, namespace: String) -> Self {
		let canonical_root = canonicalize_or_self(root);
		let workspace_id = hash_hex(canonical_root.to_string_lossy().as_ref());
		Self {
			root: canonical_root,
			workspace_id,
			namespace,
		}
	}

	pub fn root(&self) -> &Path {
		&self.root
	}

	pub fn workspace_id(&self) -> &str {
		&self.workspace_id
	}

	pub fn namespace(&self) -> &str {
		&self.namespace
	}

	pub fn namespace_id(&self) -> String {
		format!("{}:{}", self.workspace_id, self.namespace)
	}

	/// Deterministic browser-session key for daemon/browser reuse.
	pub fn session_key(&self, browser: BrowserKind, headless: bool) -> String {
		format!(
			"{}:{}:{}",
			self.namespace_id(),
			browser,
			if headless { "headless" } else { "headful" }
		)
	}

	/// Root directory for all v3 state.
	pub fn state_root(&self) -> PathBuf {
		self.root.join(dirs::PLAYWRIGHT).join(STATE_VERSION_DIR)
	}

	/// Namespace-specific state directory.
	pub fn namespace_dir(&self) -> PathBuf {
		self.state_root().join("namespaces").join(&self.namespace)
	}
}

pub fn normalize_namespace(namespace: &str) -> String {
	let mut out = String::with_capacity(namespace.len());
	for c in namespace.chars() {
		if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
			out.push(c);
		} else {
			out.push('-');
		}
	}
	let trimmed = out.trim_matches('-');
	if trimmed.is_empty() {
		DEFAULT_NAMESPACE.to_string()
	} else {
		trimmed.to_string()
	}
}

fn resolve_workspace_root(workspace: Option<&str>, no_project: bool) -> Result<PathBuf> {
	if let Some(value) = workspace {
		if value == "auto" {
			return auto_workspace_root(no_project);
		}
		let explicit = PathBuf::from(value);
		let root = if explicit.is_absolute() {
			explicit
		} else {
			std::env::current_dir()?.join(explicit)
		};
		return Ok(canonicalize_or_self(root));
	}

	auto_workspace_root(no_project)
}

fn auto_workspace_root(no_project: bool) -> Result<PathBuf> {
	if !no_project {
		if let Some(project) = Project::detect() {
			return Ok(canonicalize_or_self(project.paths.root));
		}
	}

	std::env::current_dir()
		.map(canonicalize_or_self)
		.map_err(PwError::Io)
}

fn canonicalize_or_self(path: PathBuf) -> PathBuf {
	path.canonicalize().unwrap_or(path)
}

fn hash_hex(input: &str) -> String {
	let mut hasher = DefaultHasher::new();
	input.hash(&mut hasher);
	format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn normalize_namespace_sanitizes_invalid_chars() {
		let ns = normalize_namespace("prod/team A");
		assert_eq!(ns, "prod-team-A");
	}

	#[test]
	fn normalize_namespace_defaults_when_empty() {
		let ns = normalize_namespace("////");
		assert_eq!(ns, DEFAULT_NAMESPACE);
	}

	#[test]
	fn session_key_is_deterministic() {
		let scope = WorkspaceScope::from_parts(PathBuf::from("/tmp/ws"), "abc".to_string());
		let key1 = scope.session_key(BrowserKind::Chromium, true);
		let key2 = scope.session_key(BrowserKind::Chromium, true);
		assert_eq!(key1, key2);
	}
}
