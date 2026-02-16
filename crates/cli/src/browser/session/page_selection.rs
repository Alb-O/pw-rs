use tracing::debug;

use crate::error::Result;

/// Picks a page for command execution, optionally reusing an existing page.
pub(crate) async fn select_page(
	context: &pw_rs::BrowserContext,
	reuse_existing_page: bool,
	protected_urls: &[String],
	preferred_url: Option<&str>,
) -> Result<pw_rs::Page> {
	if !reuse_existing_page {
		return context.new_page().await.map_err(Into::into);
	}

	let existing_pages = context.pages();
	let mut preferred_page = None;
	let mut fallback_page = None;

	for page in existing_pages {
		let url = page.url();
		if is_protected_url(&url, protected_urls) {
			debug!(target = "pw", url = %url, "skipping protected page");
			continue;
		}

		if let Some(preferred) = preferred_url {
			if is_preferred_match(&url, preferred) {
				debug!(target = "pw", url = %url, preferred = %preferred, "found preferred page");
				preferred_page = Some(page);
				break;
			}
		}

		if fallback_page.is_none() {
			fallback_page = Some(page);
		}
	}

	match preferred_page.or(fallback_page) {
		Some(page) => {
			debug!(target = "pw", url = %page.url(), "reusing existing page");
			Ok(page)
		}
		None => {
			debug!(target = "pw", "no suitable pages found, creating new");
			Ok(context.new_page().await?)
		}
	}
}

pub(crate) fn is_protected_url(url: &str, protected_patterns: &[String]) -> bool {
	let url_lower = url.to_lowercase();
	protected_patterns.iter().any(|pattern| url_lower.contains(&pattern.to_lowercase()))
}

pub(crate) fn is_preferred_match(url: &str, preferred: &str) -> bool {
	url.starts_with(preferred) || preferred.starts_with(url) || urls_match_loosely(url, preferred)
}

fn urls_match_loosely(a: &str, b: &str) -> bool {
	fn host(url: &str) -> Option<&str> {
		let url = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
		url.split('/').next()
	}

	match (host(a), host(b)) {
		(Some(lhs), Some(rhs)) => lhs == rhs,
		_ => false,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn protected_url_matching_is_case_insensitive() {
		let patterns = vec!["Admin".to_string(), "settings".to_string()];
		assert!(is_protected_url("https://example.com/admin/panel", &patterns));
		assert!(is_protected_url("https://example.com/SETTINGS", &patterns));
		assert!(!is_protected_url("https://example.com/public", &patterns));
	}

	#[test]
	fn preferred_match_accepts_prefix_and_same_host() {
		assert!(is_preferred_match("https://example.com/dashboard", "https://example.com"));
		assert!(is_preferred_match("https://example.com", "https://example.com/dashboard"));
		assert!(is_preferred_match("https://example.com/a", "http://example.com/b"));
	}

	#[test]
	fn preferred_match_rejects_different_hosts_or_invalid_urls() {
		assert!(!is_preferred_match("https://example.com", "https://other.example.com"));
		assert!(!is_preferred_match("about:blank", "https://example.com"));
		assert!(!is_preferred_match("data:text/plain,hi", "https://example.com"));
	}
}
