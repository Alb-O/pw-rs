use pw_rs::{BrowserContextOptions, Playwright, StorageState};
use tracing::debug;

use super::types::SessionEndpoints;
use crate::context::{DownloadConfig, HarConfig};
use crate::error::{PwError, Result};
use crate::types::BrowserKind;

/// Input for browser/context acquisition flows.
pub(crate) struct ContextFactoryInput<'a> {
	pub(crate) storage_state: Option<StorageState>,
	pub(crate) headless: bool,
	pub(crate) browser_kind: BrowserKind,
	pub(crate) cdp_endpoint: Option<&'a str>,
	pub(crate) launch_server: bool,
	pub(crate) needs_custom_context: bool,
	pub(crate) har: &'a HarConfig,
	pub(crate) download: &'a DownloadConfig,
}

/// Browser/context build output used by session assembly.
pub(crate) struct ContextBuildResult {
	pub(crate) browser: pw_rs::Browser,
	pub(crate) context: pw_rs::BrowserContext,
	pub(crate) endpoints: SessionEndpoints,
	pub(crate) launched_server: Option<pw_rs::LaunchedServer>,
	pub(crate) reuse_existing_page: bool,
}

/// Builds browser/context for attach, launch-server, and fresh-launch flows.
pub(crate) async fn build_browser_context(playwright: &mut Playwright, input: ContextFactoryInput<'_>) -> Result<ContextBuildResult> {
	let ContextFactoryInput {
		storage_state,
		headless,
		browser_kind,
		cdp_endpoint,
		launch_server,
		needs_custom_context,
		har,
		download,
	} = input;

	if let Some(endpoint) = cdp_endpoint {
		if browser_kind != BrowserKind::Chromium {
			return Err(PwError::BrowserLaunch("CDP endpoint connections require the chromium browser".to_string()));
		}

		let connect_result = playwright
			.chromium()
			.connect_over_cdp(endpoint)
			.await
			.map_err(|e| PwError::BrowserLaunch(e.to_string()))?;

		let browser = connect_result.browser;
		let mut reuse_existing_page = false;
		let context = if needs_custom_context {
			let options = build_context_options(storage_state, har, download);
			browser.new_context_with_options(options).await?
		} else if let Some(default_ctx) = connect_result.default_context {
			reuse_existing_page = true;
			default_ctx
		} else {
			browser.new_context().await?
		};

		return Ok(ContextBuildResult {
			browser,
			context,
			endpoints: SessionEndpoints {
				ws: None,
				cdp: Some(endpoint.to_string()),
			},
			launched_server: None,
			reuse_existing_page,
		});
	}

	if launch_server {
		playwright.keep_server_running();
		let launch_options = pw_rs::LaunchOptions {
			headless: Some(headless),
			..Default::default()
		};
		let launched = match browser_kind {
			BrowserKind::Chromium => playwright
				.chromium()
				.launch_server_with_options(launch_options)
				.await
				.map_err(|e| PwError::BrowserLaunch(e.to_string()))?,
			BrowserKind::Firefox => playwright
				.firefox()
				.launch_server_with_options(launch_options)
				.await
				.map_err(|e| PwError::BrowserLaunch(e.to_string()))?,
			BrowserKind::Webkit => playwright
				.webkit()
				.launch_server_with_options(launch_options)
				.await
				.map_err(|e| PwError::BrowserLaunch(e.to_string()))?,
		};

		let browser = launched.browser().clone();
		let context = if needs_custom_context {
			let options = build_context_options(storage_state, har, download);
			browser.new_context_with_options(options).await?
		} else {
			browser.new_context().await?
		};

		return Ok(ContextBuildResult {
			browser,
			context,
			endpoints: SessionEndpoints {
				ws: Some(launched.ws_endpoint().to_string()),
				cdp: None,
			},
			launched_server: Some(launched.clone()),
			reuse_existing_page: false,
		});
	}

	let launch_options = pw_rs::LaunchOptions {
		headless: Some(headless),
		..Default::default()
	};
	let browser = match browser_kind {
		BrowserKind::Chromium => playwright.chromium().launch_with_options(launch_options).await?,
		BrowserKind::Firefox => playwright.firefox().launch_with_options(launch_options).await?,
		BrowserKind::Webkit => playwright.webkit().launch_with_options(launch_options).await?,
	};
	let context = if needs_custom_context {
		let options = build_context_options(storage_state, har, download);
		browser.new_context_with_options(options).await?
	} else {
		browser.new_context().await?
	};

	Ok(ContextBuildResult {
		browser,
		context,
		endpoints: SessionEndpoints::default(),
		launched_server: None,
		reuse_existing_page: false,
	})
}

fn build_context_options(storage_state: Option<StorageState>, har_config: &HarConfig, download_config: &DownloadConfig) -> BrowserContextOptions {
	let mut builder = BrowserContextOptions::builder();

	if let Some(state) = storage_state {
		builder = builder.storage_state(state);
	}

	if download_config.is_enabled() {
		builder = builder.accept_downloads(true);
	}

	if let Some(path) = &har_config.path {
		debug!(
			target = "pw",
			har_path = %path.display(),
			"configuring HAR recording"
		);
		builder = builder.record_har_path(path.to_string_lossy());
		if let Some(policy) = har_config.content_policy {
			builder = builder.record_har_content(policy);
		}
		if let Some(mode) = har_config.mode {
			builder = builder.record_har_mode(mode);
		}
		if har_config.omit_content {
			builder = builder.record_har_omit_content(true);
		}
		if let Some(filter) = &har_config.url_filter {
			builder = builder.record_har_url_filter(filter);
		}
	}

	builder.build()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn endpoint_bundle_reports_empty_for_default() {
		assert!(SessionEndpoints::default().is_empty());
	}
}
