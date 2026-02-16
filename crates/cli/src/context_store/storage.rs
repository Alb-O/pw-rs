//! File storage for profile-scoped CLI state.

use std::fs;
use std::path::{Path, PathBuf};

use pw_rs::dirs;

use super::types::{CliCache, CliConfig};
use crate::error::Result;
use crate::workspace::{STATE_VERSION_DIR, ensure_state_gitignore_for};

/// File paths for profile-scoped CLI state.
///
/// Layout:
/// `<workspace>/playwright/.pw-cli-v4/profiles/<profile>/...`
#[derive(Debug, Clone)]
pub struct StatePaths {
	pub workspace_root: PathBuf,
	pub profile: String,
	pub state_root: PathBuf,
	pub profile_dir: PathBuf,
	pub config: PathBuf,
	pub cache: PathBuf,
	pub sessions_dir: PathBuf,
	pub session_descriptor: PathBuf,
	pub auth_dir: PathBuf,
}

impl StatePaths {
	pub fn new(workspace_root: &Path, profile: &str) -> Self {
		let state_root = workspace_root.join(dirs::PLAYWRIGHT).join(STATE_VERSION_DIR);
		let profile_dir = state_root.join("profiles").join(profile);
		let sessions_dir = profile_dir.join("sessions");
		Self {
			workspace_root: workspace_root.to_path_buf(),
			profile: profile.to_string(),
			state_root,
			profile_dir: profile_dir.clone(),
			config: profile_dir.join("config.json"),
			cache: profile_dir.join("cache.json"),
			sessions_dir: sessions_dir.clone(),
			session_descriptor: sessions_dir.join("session.json"),
			auth_dir: profile_dir.join("auth"),
		}
	}
}

/// Loaded namespace state from disk.
#[derive(Debug)]
pub struct LoadedState {
	pub config: CliConfig,
	pub cache: CliCache,
	pub paths: StatePaths,
}

impl LoadedState {
	pub fn load(workspace_root: &Path, profile: &str) -> Result<Self> {
		let paths = StatePaths::new(workspace_root, profile);
		let config = load_json::<CliConfig>(&paths.config).unwrap_or_default();
		let cache = load_json::<CliCache>(&paths.cache).unwrap_or_default();

		Ok(Self { config, cache, paths })
	}

	pub fn save(&self) -> Result<()> {
		save_json(&self.paths.config, &self.config)?;
		save_json(&self.paths.cache, &self.cache)?;
		Ok(())
	}
}

fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
	fs::read_to_string(path).ok().and_then(|content| serde_json::from_str(&content).ok())
}

fn save_json<T: serde::Serialize>(path: &Path, data: &T) -> Result<()> {
	ensure_state_gitignore_for(path)?;
	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent)?;
	}
	fs::write(path, serde_json::to_string_pretty(data)?)?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use tempfile::TempDir;

	use super::*;

	#[test]
	fn test_state_paths_layout() {
		let tmp = TempDir::new().unwrap();
		let paths = StatePaths::new(tmp.path(), "default");

		assert!(paths.config.ends_with("playwright/.pw-cli-v4/profiles/default/config.json"));
		assert!(paths.cache.ends_with("playwright/.pw-cli-v4/profiles/default/cache.json"));
		assert!(
			paths
				.session_descriptor
				.ends_with("playwright/.pw-cli-v4/profiles/default/sessions/session.json")
		);
	}

	#[test]
	fn test_load_json_missing_file() {
		let tmp = TempDir::new().unwrap();
		let missing = tmp.path().join("nonexistent.json");
		let result: Option<CliConfig> = load_json(&missing);
		assert!(result.is_none());
	}

	#[test]
	fn test_save_and_load_json() {
		let tmp = TempDir::new().unwrap();
		let path = tmp.path().join("test.json");

		let config = CliConfig {
			schema: super::super::types::SCHEMA_VERSION,
			defaults: super::super::types::Defaults {
				browser: Some(crate::types::BrowserKind::Firefox),
				..Default::default()
			},
			..Default::default()
		};

		save_json(&path, &config).unwrap();
		let loaded: CliConfig = load_json(&path).unwrap();
		assert_eq!(loaded, config);
	}

	#[test]
	fn test_save_json_creates_state_gitignore_when_under_state_root() {
		let tmp = TempDir::new().unwrap();
		let path = tmp
			.path()
			.join("playwright")
			.join(STATE_VERSION_DIR)
			.join("profiles")
			.join("default")
			.join("cache.json");

		let cache = CliCache::default();
		save_json(&path, &cache).unwrap();

		let gitignore = tmp.path().join("playwright").join(STATE_VERSION_DIR).join(".gitignore");
		assert!(gitignore.exists());
		assert_eq!(fs::read_to_string(gitignore).unwrap(), "*\n");
	}
}
