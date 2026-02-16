use pw_rs::Playwright;
use tracing::debug;

use super::config::SessionConfig;
use super::context_factory::{ContextFactoryInput, build_browser_context};
use super::features::{blocking, downloads, har};
use super::{BrowserSession, ShutdownMode, page_selection};
use crate::error::{PwError, Result};

/// Builds a fully initialized [`BrowserSession`] from owned config.
pub(crate) async fn build(config: SessionConfig) -> Result<BrowserSession> {
	let needs_custom_context = config.needs_custom_context();

	let SessionConfig {
		wait_until,
		storage_state,
		headless,
		browser_kind,
		cdp_endpoint,
		launch_server,
		protected_urls,
		preferred_url,
		har,
		block,
		download,
	} = config;

	debug!(
		target = "pw",
		browser = %browser_kind,
		cdp = cdp_endpoint.is_some(),
		launch_server,
		"starting Playwright..."
	);

	let mut playwright = Playwright::launch().await.map_err(|e| PwError::BrowserLaunch(e.to_string()))?;
	let context_build = build_browser_context(
		&mut playwright,
		ContextFactoryInput {
			storage_state,
			headless,
			browser_kind,
			cdp_endpoint: cdp_endpoint.as_deref(),
			launch_server,
			needs_custom_context,
			har: &har,
			download: &download,
		},
	)
	.await?;
	let page = page_selection::select_page(
		&context_build.context,
		context_build.reuse_existing_page,
		&protected_urls,
		preferred_url.as_deref(),
	)
	.await?;
	let har_recording = har::start_if_enabled(&context_build.context, &har).await?;
	let route_subscriptions = blocking::install_routes(&page, &block).await?;
	let download_tracking = downloads::install_tracking(&page, &download)?;
	let shutdown_mode = if context_build.launched_server.is_some() {
		ShutdownMode::KeepBrowserAlive
	} else {
		ShutdownMode::CloseSessionOnly
	};

	Ok(BrowserSession {
		_playwright: playwright,
		browser: context_build.browser,
		context: context_build.context,
		page,
		wait_until,
		endpoints: context_build.endpoints,
		launched_server: context_build.launched_server,
		shutdown_mode,
		har_recording,
		route_subscriptions,
		download_subscription: download_tracking.subscription,
		downloads: download_tracking.downloads,
	})
}
