//! Session acquisition outcomes and active session handle types.

use std::path::Path;

use crate::artifact_collector::{CollectedArtifacts, collect_failure_artifacts};
use crate::browser::{BrowserSession, DownloadInfo, SessionEndpoints, ShutdownMode};
use crate::error::Result;
use crate::output::SessionSource;
use crate::target::Target;

/// Active session handle used by command flows.
pub struct SessionHandle {
	pub(crate) session: BrowserSession,
	pub(crate) source: SessionSource,
}

impl SessionHandle {
	/// Returns where this session was sourced from.
	pub fn source(&self) -> SessionSource {
		self.source
	}

	/// Navigates to a URL.
	pub async fn goto(&self, url: &str, timeout_ms: Option<u64>) -> Result<()> {
		self.session.goto(url, timeout_ms).await
	}

	/// Navigates only when current URL differs from `url`.
	pub async fn goto_if_needed(&self, url: &str, timeout_ms: Option<u64>) -> Result<bool> {
		let current_url = self.page().evaluate_value("window.location.href").await.unwrap_or_else(|_| self.page().url());
		let current = current_url.trim_matches('"');

		if urls_match(current, url) {
			Ok(false)
		} else {
			self.session.goto(url, timeout_ms).await?;
			Ok(true)
		}
	}

	/// Navigates according to typed [`Target`] semantics.
	pub async fn goto_target(&self, target: &Target, timeout_ms: Option<u64>) -> Result<bool> {
		match target {
			Target::Navigate(url) => self.goto_if_needed(url.as_str(), timeout_ms).await,
			Target::CurrentPage => Ok(false),
		}
	}

	/// Returns the active page.
	pub fn page(&self) -> &pw_rs::Page {
		self.session.page()
	}

	/// Returns the active browser context.
	pub fn context(&self) -> &pw_rs::BrowserContext {
		self.session.context()
	}

	/// Returns discovered session endpoints.
	pub fn endpoints(&self) -> SessionEndpoints {
		self.session.endpoints().clone()
	}

	/// Returns browser handle.
	pub fn browser(&self) -> &pw_rs::Browser {
		self.session.browser()
	}

	/// Returns downloads observed in this session.
	pub fn downloads(&self) -> Vec<DownloadInfo> {
		self.session.downloads()
	}

	/// Shuts down session resources with an explicit mode.
	pub async fn shutdown(self, mode: ShutdownMode) -> Result<()> {
		self.session.shutdown(mode).await
	}

	/// Closes session resources.
	pub async fn close(self) -> Result<()> {
		let mode = self.session.shutdown_mode();
		self.session.shutdown(mode).await
	}

	/// Shuts down launched browser server (when applicable).
	pub async fn shutdown_server(self) -> Result<()> {
		self.session.shutdown(ShutdownMode::ShutdownServer).await
	}

	/// Collects failure artifacts from current page state.
	pub async fn collect_failure_artifacts(&self, artifacts_dir: Option<&Path>, command_name: &str) -> CollectedArtifacts {
		match artifacts_dir {
			Some(dir) => collect_failure_artifacts(self.page(), dir, command_name).await,
			None => CollectedArtifacts::default(),
		}
	}
}

fn urls_match(current: &str, target: &str) -> bool {
	if current == target {
		return true;
	}

	let current_normalized = current.trim_end_matches('/');
	let target_normalized = target.trim_end_matches('/');

	current_normalized == target_normalized
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn urls_match_treats_trailing_slash_as_equal() {
		assert!(urls_match("https://example.com", "https://example.com"));
		assert!(urls_match("https://example.com/", "https://example.com"));
		assert!(urls_match("https://example.com/path/", "https://example.com/path"));
	}

	#[test]
	fn urls_match_rejects_different_targets() {
		assert!(!urls_match("https://example.com", "https://other.example.com"));
		assert!(!urls_match("https://example.com/a", "https://example.com/b"));
		assert!(!urls_match("https://example.com", "http://example.com"));
	}
}
