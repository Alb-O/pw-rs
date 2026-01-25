//! CLI state types: [`CliConfig`], [`CliCache`], [`CliSecrets`].

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::types::BrowserKind;

/// Schema version for config/cache files.
pub const SCHEMA_VERSION: u32 = 2;

/// Default settings applied when no profile override exists.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Defaults {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub browser: Option<BrowserKind>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub headless: Option<bool>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub base_url: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub cdp_endpoint: Option<String>,
}

/// Profile-specific configuration overrides.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileConfig {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub base_url: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub browser: Option<BrowserKind>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub headless: Option<bool>,
}

/// Durable CLI configuration.
///
/// Global: `~/.config/pw/cli/config.json`, project: `playwright/.pw-cli/config.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CliConfig {
	#[serde(default)]
	pub schema: u32,
	#[serde(default)]
	pub defaults: Defaults,
	#[serde(default, skip_serializing_if = "HashMap::is_empty")]
	pub profiles: HashMap<String, ProfileConfig>,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub protected_urls: Vec<String>,
}

impl CliConfig {
	/// Creates a config with current [`SCHEMA_VERSION`].
	pub fn new() -> Self {
		Self {
			schema: SCHEMA_VERSION,
			..Default::default()
		}
	}

	/// Merges `other` into self (other takes precedence for set fields).
	pub fn merge(&mut self, other: &CliConfig) {
		macro_rules! merge_opt {
			($field:ident) => {
				if other.defaults.$field.is_some() {
					self.defaults.$field = other.defaults.$field.clone();
				}
			};
		}
		merge_opt!(browser);
		merge_opt!(headless);
		merge_opt!(base_url);
		merge_opt!(cdp_endpoint);

		self.profiles.extend(other.profiles.clone());
		for url in &other.protected_urls {
			if !self.protected_urls.contains(url) {
				self.protected_urls.push(url.clone());
			}
		}
	}
}

/// Ephemeral session cache.
///
/// Global: `~/.cache/pw/cli/cache.json`, project: `playwright/.pw-cli/cache.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CliCache {
	#[serde(default)]
	pub schema: u32,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub active_profile: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub last_url: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub last_selector: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub last_output: Option<String>,
	/// Unix epoch seconds.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub last_used_at: Option<u64>,
}

impl CliCache {
	/// Creates a cache with current [`SCHEMA_VERSION`].
	pub fn new() -> Self {
		Self {
			schema: SCHEMA_VERSION,
			..Default::default()
		}
	}

	/// Returns true if `last_used_at` exceeds `timeout_secs`.
	pub fn is_stale(&self, timeout_secs: u64) -> bool {
		let now = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap_or_default()
			.as_secs();
		self.last_used_at
			.is_some_and(|last| now.saturating_sub(last) > timeout_secs)
	}

	/// Clears session data (last_url, last_selector, last_output).
	pub fn clear_session(&mut self) {
		self.last_url = None;
		self.last_selector = None;
		self.last_output = None;
	}
}

/// Sensitive credentials (global only, 0600 permissions).
///
/// Stored in `~/.config/pw/cli/secrets.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CliSecrets {
	#[serde(default)]
	pub schema: u32,
	/// Auth file path per profile name.
	#[serde(default, skip_serializing_if = "HashMap::is_empty")]
	pub auth_files: HashMap<String, String>,
}

impl CliSecrets {
	/// Creates secrets with current [`SCHEMA_VERSION`].
	pub fn new() -> Self {
		Self {
			schema: SCHEMA_VERSION,
			..Default::default()
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_config_merge() {
		let mut base = CliConfig {
			defaults: Defaults {
				browser: Some(BrowserKind::Chromium),
				headless: Some(true),
				base_url: Some("https://base.com".into()),
				cdp_endpoint: None,
			},
			profiles: HashMap::from([("dev".into(), ProfileConfig::default())]),
			protected_urls: vec!["admin".into()],
			..Default::default()
		};

		let project = CliConfig {
			defaults: Defaults {
				browser: None,
				headless: Some(false),
				base_url: Some("https://project.com".into()),
				cdp_endpoint: Some("ws://localhost:9222".into()),
			},
			profiles: HashMap::from([(
				"staging".into(),
				ProfileConfig {
					base_url: Some("https://staging.com".into()),
					..Default::default()
				},
			)]),
			protected_urls: vec!["settings".into()],
			..Default::default()
		};

		base.merge(&project);

		// Browser unchanged (project didn't override)
		assert_eq!(base.defaults.browser, Some(BrowserKind::Chromium));
		// Headless overridden
		assert_eq!(base.defaults.headless, Some(false));
		// Base URL overridden
		assert_eq!(
			base.defaults.base_url,
			Some("https://project.com".to_string())
		);
		// CDP endpoint added
		assert_eq!(
			base.defaults.cdp_endpoint,
			Some("ws://localhost:9222".to_string())
		);
		// Profiles merged
		assert!(base.profiles.contains_key("dev"));
		assert!(base.profiles.contains_key("staging"));
		// Protected URLs merged
		assert!(base.protected_urls.contains(&"admin".to_string()));
		assert!(base.protected_urls.contains(&"settings".to_string()));
	}

	#[test]
	fn test_cache_staleness() {
		let fresh = CliCache {
			last_used_at: Some(
				std::time::SystemTime::now()
					.duration_since(std::time::UNIX_EPOCH)
					.unwrap()
					.as_secs(),
			),
			..Default::default()
		};
		assert!(!fresh.is_stale(3600));

		let stale = CliCache {
			last_used_at: Some(0), // Unix epoch
			..Default::default()
		};
		assert!(stale.is_stale(3600));

		let no_timestamp = CliCache::default();
		assert!(!no_timestamp.is_stale(3600));
	}

	#[test]
	fn test_cache_clear_session() {
		let mut cache = CliCache {
			active_profile: Some("dev".into()),
			last_url: Some("https://example.com".into()),
			last_selector: Some("#button".into()),
			last_output: Some("screenshot.png".into()),
			last_used_at: Some(12345),
			..Default::default()
		};

		cache.clear_session();

		assert_eq!(cache.active_profile, Some("dev".into())); // Preserved
		assert_eq!(cache.last_url, None);
		assert_eq!(cache.last_selector, None);
		assert_eq!(cache.last_output, None);
		assert_eq!(cache.last_used_at, Some(12345)); // Preserved
	}
}
