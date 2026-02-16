//! Persistent profile-scoped context storage for CLI state across invocations.
//!
//! State categories:
//! * [`CliConfig`]: durable settings (base URL, browser defaults, protected URLs)
//! * [`CliCache`]: ephemeral command cache (last URL, selector, output)

use std::path::{Path, PathBuf};

use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::types::BrowserKind;

pub mod storage;
pub mod types;

#[cfg(test)]
mod tests;

pub use storage::LoadedState;
pub use types::{CliCache, CliConfig, Defaults, HarDefaults};

const SESSION_TIMEOUT_SECS: u64 = 3600;

/// Runtime context state manager.
///
/// Uses [`LoadedState`] for profile-scoped storage.
/// Auto-refreshes stale sessions after [`SESSION_TIMEOUT_SECS`].
#[derive(Debug)]
pub struct ContextState {
	state: LoadedState,
	workspace_id: String,
	profile: String,
	base_url_override: Option<String>,
	no_context: bool,
	no_save: bool,
	refresh: bool,
}

impl ContextState {
	#[allow(clippy::too_many_arguments)]
	pub fn new(
		workspace_root: PathBuf,
		workspace_id: String,
		profile: String,
		base_url_override: Option<String>,
		no_context: bool,
		no_save: bool,
		refresh: bool,
	) -> Result<Self> {
		let state = LoadedState::load(&workspace_root, &profile)?;
		let is_stale = state.cache.is_stale(SESSION_TIMEOUT_SECS);

		Ok(Self {
			refresh: refresh || is_stale,
			state,
			workspace_id,
			profile,
			base_url_override,
			no_context,
			no_save,
		})
	}

	#[cfg(test)]
	pub(crate) fn test_new(state: LoadedState, workspace_id: String, profile: String) -> Self {
		Self {
			state,
			workspace_id,
			profile,
			base_url_override: None,
			no_context: false,
			no_save: false,
			refresh: false,
		}
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

	pub fn namespace(&self) -> &str {
		self.profile()
	}

	pub fn namespace_id(&self) -> String {
		self.profile_id()
	}

	pub fn workspace_root(&self) -> &Path {
		&self.state.paths.workspace_root
	}

	pub fn session_key(&self, browser: BrowserKind, headless: bool) -> String {
		format!("{}:{}:{}", self.profile_id(), browser, if headless { "headless" } else { "headful" })
	}

	pub fn session_descriptor_path(&self) -> Option<PathBuf> {
		if self.no_context {
			return None;
		}
		Some(self.state.paths.session_descriptor.clone())
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
		(!self.refresh && self.state.cache.last_url.is_some()) || self.state.config.defaults.base_url.is_some()
	}

	pub fn resolve_selector(&self, provided: Option<String>, fallback: Option<&str>) -> Result<String> {
		if let Some(selector) = provided {
			return Ok(selector);
		}

		if self.no_context {
			return fallback
				.map(String::from)
				.ok_or_else(|| PwError::Context("Selector is required when context usage is disabled".into()));
		}

		if !self.refresh {
			if let Some(selector) = &self.state.cache.last_selector {
				return Ok(selector.clone());
			}
		}

		fallback.map(String::from).ok_or_else(|| PwError::Context("No selector available".into()))
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

	/// Returns persisted HAR defaults from config.
	pub fn har_defaults(&self) -> Option<&HarDefaults> {
		if self.no_context {
			return None;
		}
		self.state.config.har.as_ref()
	}

	/// Sets persisted HAR defaults. Returns `true` when the value changed.
	pub fn set_har_defaults(&mut self, har: HarDefaults) -> bool {
		if self.no_save || self.no_context {
			return false;
		}
		let changed = self.state.config.har.as_ref() != Some(&har);
		self.state.config.har = Some(har);
		changed
	}

	/// Clears persisted HAR defaults. Returns `true` when a value was removed.
	pub fn clear_har_defaults(&mut self) -> bool {
		if self.no_save || self.no_context {
			return false;
		}
		self.state.config.har.take().is_some()
	}

	/// Builds effective runtime HAR config from persisted defaults.
	pub fn effective_har_config(&self) -> crate::context::HarConfig {
		let Some(har) = self.har_defaults() else {
			return crate::context::HarConfig::default();
		};
		crate::context::HarConfig {
			path: Some(har.path.clone()),
			content_policy: Some(har.content_policy),
			mode: Some(har.mode),
			omit_content: har.omit_content,
			url_filter: har.url_filter.clone(),
		}
	}

	/// Returns true if the URL matches any protected pattern.
	pub fn is_protected(&self, url: &str) -> bool {
		let url_lower = url.to_lowercase();
		self.protected_urls().iter().any(|pattern| url_lower.contains(&pattern.to_lowercase()))
	}

	/// Adds a URL pattern to the protected list. Returns true if added.
	pub fn add_protected(&mut self, pattern: String) -> bool {
		if self.no_save || self.no_context {
			return false;
		}
		let pattern_lower = pattern.to_lowercase();
		if self.state.config.protected_urls.iter().any(|p| p.to_lowercase() == pattern_lower) {
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
		self.state.config.protected_urls.retain(|p| p.to_lowercase() != pattern_lower);
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
	pub fn record_from_target(&mut self, target: &crate::target::ResolvedTarget, selector: Option<&str>) {
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
		self.base_url_override.as_deref().or(self.state.config.defaults.base_url.as_deref())
	}

	/// Returns the loaded state.
	pub fn state(&self) -> &LoadedState {
		&self.state
	}

	/// Returns mutable access to the loaded state.
	pub fn state_mut(&mut self) -> &mut LoadedState {
		&mut self.state
	}
}

fn now_ts() -> u64 {
	std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}
