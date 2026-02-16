//! Workspace and profile identity utilities.
//!
//! Defines deterministic identifiers used for strict session isolation.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use pw_rs::dirs;

use crate::error::{PwError, Result};
use crate::project::Project;
use crate::types::BrowserKind;

pub const DEFAULT_PROFILE: &str = "default";
pub const STATE_VERSION_DIR: &str = ".pw-cli-v4";
pub const STATE_GITIGNORE_CONTENT: &str = "*\n";
pub const CDP_PORT_RANGE_START: u16 = 9222;
pub const CDP_PORT_RANGE_SIZE: u16 = 1000;

/// Canonical identity for a workspace profile.
#[derive(Debug, Clone)]
pub struct WorkspaceScope {
	root: PathBuf,
	workspace_id: String,
	profile: String,
}

impl WorkspaceScope {
	/// Resolve workspace root + profile from CLI values.
	///
	/// * `workspace`: explicit workspace path, or `"auto"` for detection.
	/// * `profile`: optional profile; defaults to [`DEFAULT_PROFILE`].
	/// * `no_project`: when true, skip playwright project-root detection.
	pub fn resolve(workspace: Option<&str>, profile: Option<&str>, no_project: bool) -> Result<Self> {
		let profile = normalize_profile(profile.unwrap_or(DEFAULT_PROFILE));
		let root = resolve_workspace_root(workspace, no_project)?;
		Ok(Self::from_parts(root, profile))
	}

	pub fn from_parts(root: PathBuf, profile: String) -> Self {
		let canonical_root = canonicalize_or_self(root);
		let workspace_id = hash_hex(canonical_root.to_string_lossy().as_ref());
		Self {
			root: canonical_root,
			workspace_id,
			profile,
		}
	}

	pub fn root(&self) -> &Path {
		&self.root
	}

	pub fn workspace_id(&self) -> &str {
		&self.workspace_id
	}

	pub fn profile(&self) -> &str {
		&self.profile
	}

	pub fn profile_id(&self) -> String {
		format!("{}:{}", self.workspace_id, self.profile)
	}

	/// Backward compatibility alias.
	pub fn namespace(&self) -> &str {
		self.profile()
	}

	/// Backward compatibility alias.
	pub fn namespace_id(&self) -> String {
		self.profile_id()
	}

	/// Deterministic browser-session key for daemon/browser reuse.
	pub fn session_key(&self, browser: BrowserKind, headless: bool) -> String {
		format!("{}:{}:{}", self.profile_id(), browser, if headless { "headless" } else { "headful" })
	}

	/// Root directory for all v4 state.
	pub fn state_root(&self) -> PathBuf {
		self.root.join(dirs::PLAYWRIGHT).join(STATE_VERSION_DIR)
	}

	/// Profile-specific state directory.
	pub fn profile_dir(&self) -> PathBuf {
		self.state_root().join("profiles").join(&self.profile)
	}

	/// Backward compatibility alias.
	pub fn namespace_dir(&self) -> PathBuf {
		self.profile_dir()
	}
}

pub fn normalize_profile(profile: &str) -> String {
	let mut out = String::with_capacity(profile.len());
	for c in profile.chars() {
		if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
			out.push(c);
		} else {
			out.push('-');
		}
	}
	let trimmed = out.trim_matches('-');
	if trimmed.is_empty() {
		DEFAULT_PROFILE.to_string()
	} else {
		trimmed.to_string()
	}
}

/// Backward compatibility alias.
pub fn normalize_namespace(namespace: &str) -> String {
	normalize_profile(namespace)
}

/// Ensures the runtime state root has a local .gitignore that hides generated files.
///
/// The file uses a single `*` pattern so all transient files under `.pw-cli-v4/`
/// stay out of git status, even when `pw init` was never run.
pub fn ensure_state_root_gitignore(state_root: &Path) -> Result<()> {
	std::fs::create_dir_all(state_root)?;
	let gitignore_path = state_root.join(".gitignore");
	if !gitignore_path.exists() {
		std::fs::write(gitignore_path, STATE_GITIGNORE_CONTENT)?;
	}
	Ok(())
}

/// Best-effort helper: find `.pw-cli-v4` in the path ancestry and ensure its `.gitignore`.
///
/// Returns `Ok(())` even when the path is not under `.pw-cli-v4`.
pub fn ensure_state_gitignore_for(path: &Path) -> Result<()> {
	if let Some(state_root) = find_state_root(path) {
		ensure_state_root_gitignore(&state_root)?;
	}
	Ok(())
}

fn find_state_root(path: &Path) -> Option<PathBuf> {
	let mut current = Some(path);
	while let Some(candidate) = current {
		if candidate.file_name().and_then(|name| name.to_str()) == Some(STATE_VERSION_DIR) {
			return Some(candidate.to_path_buf());
		}
		current = candidate.parent();
	}
	None
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

	std::env::current_dir().map(canonicalize_or_self).map_err(PwError::Io)
}

fn auto_workspace_root(no_project: bool) -> Result<PathBuf> {
	if !no_project {
		if let Some(project) = Project::detect() {
			return Ok(canonicalize_or_self(project.paths.root));
		}
	}

	std::env::current_dir().map(canonicalize_or_self).map_err(PwError::Io)
}

fn canonicalize_or_self(path: PathBuf) -> PathBuf {
	path.canonicalize().unwrap_or(path)
}

fn hash_hex(input: &str) -> String {
	let mut hasher = DefaultHasher::new();
	input.hash(&mut hasher);
	format!("{:016x}", hasher.finish())
}

/// Compute a deterministic CDP port for a profile identity.
///
/// Uses a bounded range starting at [`CDP_PORT_RANGE_START`], sized by
/// [`CDP_PORT_RANGE_SIZE`].
pub fn compute_cdp_port(profile_id: &str) -> u16 {
	let mut hasher = DefaultHasher::new();
	profile_id.hash(&mut hasher);
	let hash = hasher.finish();
	CDP_PORT_RANGE_START + (hash % u64::from(CDP_PORT_RANGE_SIZE)) as u16
}

#[cfg(test)]
mod tests {
	use tempfile::TempDir;

	use super::*;

	#[test]
	fn normalize_profile_sanitizes_invalid_chars() {
		let profile = normalize_profile("prod/team A");
		assert_eq!(profile, "prod-team-A");
	}

	#[test]
	fn normalize_profile_defaults_when_empty() {
		let profile = normalize_profile("////");
		assert_eq!(profile, DEFAULT_PROFILE);
	}

	#[test]
	fn session_key_is_deterministic() {
		let scope = WorkspaceScope::from_parts(PathBuf::from("/tmp/ws"), "abc".to_string());
		let key1 = scope.session_key(BrowserKind::Chromium, true);
		let key2 = scope.session_key(BrowserKind::Chromium, true);
		assert_eq!(key1, key2);
	}

	#[test]
	fn default_workspace_uses_current_directory_not_project_root() {
		let _cwd_lock = crate::test_sync::lock_cwd();
		let temp = TempDir::new().unwrap();
		let project_root = temp.path().join("project");
		let nested = project_root.join("agents").join("agent-a");
		std::fs::create_dir_all(&nested).unwrap();
		std::fs::write(project_root.join(pw_rs::dirs::CONFIG_JS), "export default {}").unwrap();

		let original_dir = std::env::current_dir().unwrap();
		struct CwdGuard(PathBuf);
		impl Drop for CwdGuard {
			fn drop(&mut self) {
				let _ = std::env::set_current_dir(&self.0);
			}
		}
		let _guard = CwdGuard(original_dir);
		std::env::set_current_dir(&nested).unwrap();

		let scope = WorkspaceScope::resolve(None, Some("default"), false).unwrap();
		assert_eq!(scope.root(), nested.as_path());
	}

	#[test]
	fn auto_workspace_still_detects_project_root() {
		let _cwd_lock = crate::test_sync::lock_cwd();
		let temp = TempDir::new().unwrap();
		let project_root = temp.path().join("project");
		let nested = project_root.join("agents").join("agent-a");
		std::fs::create_dir_all(&nested).unwrap();
		std::fs::write(project_root.join(pw_rs::dirs::CONFIG_JS), "export default {}").unwrap();

		let original_dir = std::env::current_dir().unwrap();
		struct CwdGuard(PathBuf);
		impl Drop for CwdGuard {
			fn drop(&mut self) {
				let _ = std::env::set_current_dir(&self.0);
			}
		}
		let _guard = CwdGuard(original_dir);
		std::env::set_current_dir(&nested).unwrap();

		let scope = WorkspaceScope::resolve(Some("auto"), Some("default"), false).unwrap();
		assert_eq!(scope.root(), project_root.as_path());
	}

	#[test]
	fn ensure_state_root_gitignore_creates_wildcard_ignore() {
		let temp = TempDir::new().unwrap();
		let state_root = temp.path().join("playwright").join(STATE_VERSION_DIR);
		ensure_state_root_gitignore(&state_root).unwrap();

		let ignore = std::fs::read_to_string(state_root.join(".gitignore")).unwrap();
		assert_eq!(ignore, STATE_GITIGNORE_CONTENT);
	}

	#[test]
	fn ensure_state_gitignore_for_uses_ancestor_state_root() {
		let temp = TempDir::new().unwrap();
		let target = temp
			.path()
			.join("playwright")
			.join(STATE_VERSION_DIR)
			.join("profiles")
			.join("default")
			.join("cache.json");
		ensure_state_gitignore_for(&target).unwrap();

		assert!(temp.path().join("playwright").join(STATE_VERSION_DIR).join(".gitignore").exists());
	}

	#[test]
	fn compute_cdp_port_is_stable_and_bounded() {
		let profile_id = "workspace:agent-a";
		let port = compute_cdp_port(profile_id);
		assert_eq!(port, compute_cdp_port(profile_id));
		assert!((CDP_PORT_RANGE_START..CDP_PORT_RANGE_START + CDP_PORT_RANGE_SIZE).contains(&port));
	}
}
