//! File storage for CLI state (Config/Cache/Secrets).

use std::fs;
use std::path::{Path, PathBuf};

use pw_rs::dirs;

use super::types::{CliCache, CliConfig, CliSecrets};
use crate::error::Result;

/// File paths for CLI state storage.
///
/// Global paths use XDG directories (`~/.config/pw/cli/`, `~/.cache/pw/cli/`).
/// Project paths use `playwright/.pw-cli/` when a project root is detected.
#[derive(Debug, Clone)]
pub struct StatePaths {
	pub global_config: PathBuf,
	pub global_cache: PathBuf,
	pub global_secrets: PathBuf,
	pub global_sessions: PathBuf,
	pub project_config: Option<PathBuf>,
	pub project_cache: Option<PathBuf>,
	pub project_sessions: Option<PathBuf>,
}

impl StatePaths {
	pub fn new(project_root: Option<&Path>) -> Self {
		let config_home = std::env::var_os("XDG_CONFIG_HOME")
			.map(PathBuf::from)
			.or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
			.unwrap_or_else(|| PathBuf::from("."));

		let cache_home = std::env::var_os("XDG_CACHE_HOME")
			.map(PathBuf::from)
			.or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
			.unwrap_or_else(|| PathBuf::from("."));

		let cli_config_dir = config_home.join("pw/cli");
		let cli_cache_dir = cache_home.join("pw/cli");

		let (project_config, project_cache, project_sessions) = if let Some(root) = project_root {
			let pw_cli = root.join(dirs::PLAYWRIGHT).join(".pw-cli");
			(
				Some(pw_cli.join("config.json")),
				Some(pw_cli.join("cache.json")),
				Some(pw_cli.join("sessions")),
			)
		} else {
			(None, None, None)
		};

		Self {
			global_config: cli_config_dir.join("config.json"),
			global_cache: cli_cache_dir.join("cache.json"),
			global_secrets: cli_config_dir.join("secrets.json"),
			global_sessions: cli_config_dir.join("sessions"),
			project_config,
			project_cache,
			project_sessions,
		}
	}

	pub fn sessions_dir(&self, is_project: bool) -> Option<&Path> {
		if is_project {
			self.project_sessions.as_deref()
		} else {
			Some(&self.global_sessions)
		}
	}
}

/// Loaded CLI state from disk.
///
/// Config is merged (global + project), cache prefers project scope,
/// secrets are global-only.
#[derive(Debug)]
pub struct LoadedState {
	pub config: CliConfig,
	pub cache: CliCache,
	pub secrets: CliSecrets,
	pub is_project: bool,
	pub paths: StatePaths,
}

impl LoadedState {
	pub fn load(project_root: Option<&Path>) -> Result<Self> {
		let paths = StatePaths::new(project_root);

		let mut config = load_json::<CliConfig>(&paths.global_config).unwrap_or_default();
		let is_project = paths.project_config.is_some();
		if let Some(ref project_path) = paths.project_config {
			if let Some(project_config) = load_json::<CliConfig>(project_path) {
				config.merge(&project_config);
			}
		}

		let cache = paths
			.project_cache
			.as_ref()
			.and_then(|p| load_json::<CliCache>(p))
			.or_else(|| load_json::<CliCache>(&paths.global_cache))
			.unwrap_or_default();

		let secrets = load_json::<CliSecrets>(&paths.global_secrets).unwrap_or_default();

		Ok(Self { config, cache, secrets, is_project, paths })
	}

	pub fn save(&self) -> Result<()> {
		if self.is_project {
			if let Some(ref path) = self.paths.project_config {
				save_json(path, &self.config)?;
			}
			if let Some(ref path) = self.paths.project_cache {
				save_json(path, &self.cache)?;
			}
		} else {
			save_json(&self.paths.global_config, &self.config)?;
			save_json(&self.paths.global_cache, &self.cache)?;
		}
		save_secrets(&self.paths.global_secrets, &self.secrets)?;
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

fn save_secrets(path: &Path, secrets: &CliSecrets) -> Result<()> {
	save_json(path, secrets)?;
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::TempDir;

	#[test]
	fn test_state_paths_global_only() {
		let paths = StatePaths::new(None);
		assert!(paths.global_config.ends_with("pw/cli/config.json"));
		assert!(paths.global_cache.ends_with("pw/cli/cache.json"));
		assert!(paths.global_secrets.ends_with("pw/cli/secrets.json"));
		assert!(paths.project_config.is_none());
		assert!(paths.project_cache.is_none());
	}

	#[test]
	fn test_state_paths_with_project() {
		let tmp = TempDir::new().unwrap();
		let paths = StatePaths::new(Some(tmp.path()));

		assert!(paths.project_config.is_some());
		assert!(paths.project_cache.is_some());
		assert!(paths.project_sessions.is_some());
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
			schema: 2,
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
