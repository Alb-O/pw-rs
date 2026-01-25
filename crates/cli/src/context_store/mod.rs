//! Persistent context storage for CLI state across invocations.
//!
//! State is split into three categories:
//! - [`CliConfig`]: Durable settings (browser, profiles, protected URLs)
//! - [`CliCache`]: Ephemeral data (last URL, selector, output)
//! - [`CliSecrets`]: Sensitive data (auth file paths)

use std::path::{Path, PathBuf};

use crate::context::CommandContext;
use crate::error::{PwError, Result};

pub mod storage;
pub mod types;

#[cfg(test)]
mod tests;

pub use storage::LoadedState;
pub use types::{CliCache, CliConfig, CliSecrets, Defaults, ProfileConfig};

const SESSION_TIMEOUT_SECS: u64 = 3600;

/// Runtime context state manager.
///
/// Uses [`LoadedState`] for storage with config/cache/secrets separation.
/// Auto-refreshes stale sessions after [`SESSION_TIMEOUT_SECS`].
#[derive(Debug)]
pub struct ContextState {
	state: LoadedState,
	base_url_override: Option<String>,
	no_context: bool,
	no_save: bool,
	refresh: bool,
}

impl ContextState {
	pub fn new(
		project_root: Option<PathBuf>,
		_requested_context: Option<String>,
		base_url_override: Option<String>,
		no_context: bool,
		no_save: bool,
		refresh: bool,
	) -> Result<Self> {
		let state = LoadedState::load(project_root.as_deref())?;
		let is_stale = state.cache.is_stale(SESSION_TIMEOUT_SECS);

		Ok(Self {
			refresh: refresh || is_stale,
			state,
			base_url_override,
			no_context,
			no_save,
		})
	}

	#[cfg(test)]
	pub(crate) fn test_new(state: LoadedState) -> Self {
		Self {
			state,
			base_url_override: None,
			no_context: false,
			no_save: false,
			refresh: false,
		}
	}

	pub fn active_name(&self) -> Option<&str> {
		self.state.cache.active_profile.as_deref()
	}

	pub fn session_descriptor_path(&self) -> Option<PathBuf> {
		if self.no_context {
			return None;
		}
		let profile = self.state.cache.active_profile.as_deref().unwrap_or("default");
		let dir = self.state.paths.sessions_dir(self.state.is_project)?;
		Some(dir.join(format!("{profile}.json")))
	}

	pub fn refresh_requested(&self) -> bool {
		self.refresh
	}

	/// Returns true if context has a URL available.
	pub fn has_context_url(&self) -> bool {
		if self.no_context {
			return false;
		}
		if self.base_url_override.is_some() {
			return true;
		}
		(!self.refresh && self.state.cache.last_url.is_some())
			|| self.state.config.defaults.base_url.is_some()
	}

	pub fn resolve_selector(
		&self,
		provided: Option<String>,
		fallback: Option<&str>,
	) -> Result<String> {
		if let Some(selector) = provided {
			return Ok(selector);
		}

		if self.no_context {
			return fallback.map(String::from).ok_or_else(|| {
				PwError::Context("Selector is required when context usage is disabled".into())
			});
		}

		if !self.refresh {
			if let Some(selector) = &self.state.cache.last_selector {
				return Ok(selector.clone());
			}
		}

		fallback
			.map(String::from)
			.ok_or_else(|| PwError::Context("No selector available".into()))
	}

	/// Returns the CDP endpoint from config defaults.
	pub fn cdp_endpoint(&self) -> Option<&str> {
		if self.no_context {
			return None;
		}
		self.state.config.defaults.cdp_endpoint.as_deref()
	}

	/// Returns the last URL from cache.
	pub fn last_url(&self) -> Option<&str> {
		if self.no_context {
			return None;
		}
		self.state.cache.last_url.as_deref()
	}

	/// Sets the CDP endpoint in config defaults.
	pub fn set_cdp_endpoint(&mut self, endpoint: Option<String>) {
		if self.no_save || self.no_context {
			return;
		}
		self.state.config.defaults.cdp_endpoint = endpoint;
	}

	/// Returns protected URL patterns from config.
	pub fn protected_urls(&self) -> &[String] {
		if self.no_context {
			return &[];
		}
		&self.state.config.protected_urls
	}

	/// Returns true if the URL matches any protected pattern.
	pub fn is_protected(&self, url: &str) -> bool {
		let url_lower = url.to_lowercase();
		self.protected_urls()
			.iter()
			.any(|pattern| url_lower.contains(&pattern.to_lowercase()))
	}

	/// Adds a URL pattern to the protected list. Returns true if added.
	pub fn add_protected(&mut self, pattern: String) -> bool {
		if self.no_save || self.no_context {
			return false;
		}
		let pattern_lower = pattern.to_lowercase();
		if self
			.state
			.config
			.protected_urls
			.iter()
			.any(|p| p.to_lowercase() == pattern_lower)
		{
			return false;
		}
		self.state.config.protected_urls.push(pattern);
		true
	}

	/// Removes a URL pattern from the protected list. Returns true if removed.
	pub fn remove_protected(&mut self, pattern: &str) -> bool {
		if self.no_save || self.no_context {
			return false;
		}
		let pattern_lower = pattern.to_lowercase();
		let before_len = self.state.config.protected_urls.len();
		self.state
			.config
			.protected_urls
			.retain(|p| p.to_lowercase() != pattern_lower);
		self.state.config.protected_urls.len() < before_len
	}

	pub fn resolve_output(&self, ctx: &CommandContext, provided: Option<PathBuf>) -> PathBuf {
		if let Some(output) = provided {
			return ctx.screenshot_path(&output);
		}

		if !self.no_context && !self.refresh {
			if let Some(last) = &self.state.cache.last_output {
				return ctx.screenshot_path(Path::new(last));
			}
		}

		ctx.screenshot_path(Path::new("screenshot.png"))
	}

	/// Applies context changes from command execution.
	pub fn apply_delta(&mut self, delta: crate::commands::def::ContextDelta) {
		if self.no_save || self.no_context {
			return;
		}
		if let Some(url) = delta.url {
			self.state.cache.last_url = Some(url);
		}
		if let Some(selector) = delta.selector {
			self.state.cache.last_selector = Some(selector);
		}
		if let Some(output) = delta.output {
			self.state.cache.last_output = Some(output.to_string_lossy().to_string());
		}
		self.state.cache.last_used_at = Some(now_ts());
	}

	/// Records context from a resolved target.
	pub fn record_from_target(
		&mut self,
		target: &crate::target::ResolvedTarget,
		selector: Option<&str>,
	) {
		self.apply_delta(crate::commands::def::ContextDelta {
			url: target.url_str().map(String::from),
			selector: selector.map(String::from),
			output: None,
		});
	}

	/// Persists the current context state to disk.
	pub fn persist(&mut self) -> Result<()> {
		if self.no_save || self.no_context {
			return Ok(());
		}
		self.state.save()
	}

	/// Returns the effective base URL.
	pub fn base_url(&self) -> Option<&str> {
		self.base_url_override
			.as_deref()
			.or(self.state.config.defaults.base_url.as_deref())
	}

	/// Returns the loaded state (for accessing config/cache/secrets).
	pub fn state(&self) -> &LoadedState {
		&self.state
	}

	/// Returns mutable access to the loaded state.
	pub fn state_mut(&mut self) -> &mut LoadedState {
		&mut self.state
	}
}

fn now_ts() -> u64 {
	std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.unwrap_or_default()
		.as_secs()
}
