//! File storage for namespace-scoped CLI state.

use std::fs;
use std::path::{Path, PathBuf};

use pw_rs::dirs;

use super::types::{CliCache, CliConfig};
use crate::error::Result;
use crate::workspace::STATE_VERSION_DIR;

/// File paths for namespace-scoped CLI state.
///
/// Layout:
/// `<workspace>/playwright/.pw-cli-v3/namespaces/<namespace>/...`
#[derive(Debug, Clone)]
pub struct StatePaths {
	pub workspace_root: PathBuf,
	pub namespace: String,
	pub state_root: PathBuf,
	pub namespace_dir: PathBuf,
	pub config: PathBuf,
	pub cache: PathBuf,
	pub sessions_dir: PathBuf,
	pub session_descriptor: PathBuf,
	pub auth_dir: PathBuf,
}

impl StatePaths {
	pub fn new(workspace_root: &Path, namespace: &str) -> Self {
		let state_root = workspace_root
			.join(dirs::PLAYWRIGHT)
			.join(STATE_VERSION_DIR);
		let namespace_dir = state_root.join("namespaces").join(namespace);
		let sessions_dir = namespace_dir.join("sessions");
		Self {
			workspace_root: workspace_root.to_path_buf(),
			namespace: namespace.to_string(),
			state_root,
			namespace_dir: namespace_dir.clone(),
			config: namespace_dir.join("config.json"),
			cache: namespace_dir.join("cache.json"),
			sessions_dir: sessions_dir.clone(),
			session_descriptor: sessions_dir.join("session.json"),
			auth_dir: namespace_dir.join("auth"),
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
	pub fn load(workspace_root: &Path, namespace: &str) -> Result<Self> {
		let paths = StatePaths::new(workspace_root, namespace);
		let config = load_json::<CliConfig>(&paths.config).unwrap_or_default();
		let cache = load_json::<CliCache>(&paths.cache).unwrap_or_default();

		Ok(Self {
			config,
			cache,
			paths,
		})
	}

	pub fn save(&self) -> Result<()> {
		save_json(&self.paths.config, &self.config)?;
		save_json(&self.paths.cache, &self.cache)?;
		Ok(())
	}
}

fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
	fs::read_to_string(path)
		.ok()
		.and_then(|content| serde_json::from_str(&content).ok())
}

fn save_json<T: serde::Serialize>(path: &Path, data: &T) -> Result<()> {
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

		assert!(
			paths
				.config
				.ends_with("playwright/.pw-cli-v3/namespaces/default/config.json")
		);
		assert!(
			paths
				.cache
				.ends_with("playwright/.pw-cli-v3/namespaces/default/cache.json")
		);
		assert!(
			paths
				.session_descriptor
				.ends_with("playwright/.pw-cli-v3/namespaces/default/sessions/session.json")
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
			schema: 3,
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
}
