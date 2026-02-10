//! CLI state types: [`CliConfig`] and [`CliCache`].

use serde::{Deserialize, Serialize};

use crate::types::BrowserKind;

/// Schema version for config/cache files.
pub const SCHEMA_VERSION: u32 = 3;

/// Default settings applied for a namespace.
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

/// Durable CLI configuration scoped to a namespace.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CliConfig {
	#[serde(default)]
	pub schema: u32,
	#[serde(default)]
	pub defaults: Defaults,
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
}

/// Ephemeral namespace cache.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CliCache {
	#[serde(default)]
	pub schema: u32,
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

#[cfg(test)]
mod tests {
	use super::*;

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
			last_url: Some("https://example.com".into()),
			last_selector: Some("#button".into()),
			last_output: Some("screenshot.png".into()),
			last_used_at: Some(12345),
			..Default::default()
		};

		cache.clear_session();

		assert_eq!(cache.last_url, None);
		assert_eq!(cache.last_selector, None);
		assert_eq!(cache.last_output, None);
		assert_eq!(cache.last_used_at, Some(12345)); // Preserved
	}
}
