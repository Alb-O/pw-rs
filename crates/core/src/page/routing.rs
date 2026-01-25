//! Route handling methods for [`Page`].

use std::future::Future;
use std::sync::Arc;

use pw_runtime::Result;

use crate::handlers::{
	HandlerEntry, HandlerFn, HandlerFuture, RouteMatcher, RouteMeta, Subscription, next_handler_id,
};
use crate::Route;

use super::Page;

impl Page {
	/// Registers a route handler for network interception.
	///
	/// When a request URL matches `pattern` (supports glob patterns like `**/*.png`),
	/// the handler receives a [`Route`] that can abort, continue, or fulfill the request.
	/// Returns a [`Subscription`] that unregisters the handler when dropped.
	///
	/// See <https://playwright.dev/docs/api/class-page#page-route>
	///
	/// # Example
	///
	/// ```ignore
	/// let _sub = page.route("**/*.png", |route| async move {
	///     route.abort(None).await
	/// }).await?;
	/// ```
	pub async fn route<F, Fut>(&self, pattern: &str, handler: F) -> Result<Subscription>
	where
		F: Fn(Route) -> Fut + Send + Sync + 'static,
		Fut: Future<Output = Result<()>> + Send + 'static,
	{
		let id = next_handler_id();
		let handler: HandlerFn<Route> =
			Arc::new(move |route: Route| -> HandlerFuture { Box::pin(handler(route)) });
		let matcher = RouteMatcher::new(pattern);

		self.route_handlers.lock().insert(
			id,
			HandlerEntry {
				id,
				meta: RouteMeta { matcher },
				handler,
			},
		);

		self.enable_network_interception().await?;
		Ok(Subscription::from_handler_map(id, &self.route_handlers))
	}

	/// Sends current route patterns to the browser for network interception.
	pub(super) async fn enable_network_interception(&self) -> Result<()> {
		let patterns: Vec<serde_json::Value> = self
			.route_handlers
			.lock()
			.values()
			.map(|entry| serde_json::json!({ "glob": entry.meta.matcher.as_str() }))
			.collect();

		self.channel()
			.send_no_result(
				"setNetworkInterceptionPatterns",
				serde_json::json!({ "patterns": patterns }),
			)
			.await
	}

	/// Dispatches a route event to the matching handler (last-registered wins).
	pub(super) async fn on_route_event(&self, route: Route) {
		let url = route.request().url().to_string();

		let handler = {
			let handlers = self.route_handlers.lock();
			handlers
				.values()
				.rev()
				.find(|entry| entry.meta.matcher.is_match(&url))
				.map(|entry| entry.handler.clone())
		};

		if let Some(handler) = handler {
			if let Err(e) = handler(route).await {
				tracing::error!(error = %e, "Route handler error");
			}
		}
	}
}
