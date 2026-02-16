use pw_rs::Subscription;
use tracing::debug;

use crate::context::BlockConfig;
use crate::error::{PwError, Result};

/// Installs request-blocking routes and returns RAII subscriptions.
pub(crate) async fn install_routes(page: &pw_rs::Page, block_config: &BlockConfig) -> Result<Vec<Subscription>> {
	let mut route_subscriptions = Vec::with_capacity(block_config.patterns.len());
	for pattern in &block_config.patterns {
		debug!(target = "pw", %pattern, "blocking pattern");
		let subscription = page
			.route(pattern, |route| async move { route.abort(None).await })
			.await
			.map_err(|e| PwError::BrowserLaunch(format!("route setup failed: {e}")))?;
		route_subscriptions.push(subscription);
	}
	Ok(route_subscriptions)
}
