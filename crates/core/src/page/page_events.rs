//! Event handling methods for [`Page`] (download, dialog, console).

use std::future::Future;
use std::sync::Arc;

use pw_runtime::{Error, Result};
use tokio::sync::broadcast;

use crate::handlers::{HandlerEntry, HandlerFn, HandlerFuture, Subscription, next_handler_id};
use crate::{Dialog, Download};

use super::{ConsoleMessage, Page};

impl Page {
	/// Registers a download event handler.
	///
	/// The handler is called when the page initiates a file download.
	/// Returns a [`Subscription`] that unregisters the handler when dropped.
	///
	/// See <https://playwright.dev/docs/api/class-page#page-event-download>
	pub fn on_download<F, Fut>(&self, handler: F) -> Subscription
	where
		F: Fn(Download) -> Fut + Send + Sync + 'static,
		Fut: Future<Output = Result<()>> + Send + 'static,
	{
		let id = next_handler_id();
		let handler: HandlerFn<Download> =
			Arc::new(move |download: Download| -> HandlerFuture { Box::pin(handler(download)) });

		self.download_handlers.lock().insert(
			id,
			HandlerEntry {
				id,
				meta: (),
				handler,
			},
		);

		Subscription::from_handler_map(id, &self.download_handlers)
	}

	/// Registers a dialog event handler.
	///
	/// The handler is called when a JavaScript dialog (alert, confirm, prompt, beforeunload) appears.
	/// The dialog must be explicitly accepted or dismissed, otherwise the page will freeze.
	/// Returns a [`Subscription`] that unregisters the handler when dropped.
	///
	/// See <https://playwright.dev/docs/api/class-page#page-event-dialog>
	pub fn on_dialog<F, Fut>(&self, handler: F) -> Subscription
	where
		F: Fn(Dialog) -> Fut + Send + Sync + 'static,
		Fut: Future<Output = Result<()>> + Send + 'static,
	{
		let id = next_handler_id();
		let handler: HandlerFn<Dialog> =
			Arc::new(move |dialog: Dialog| -> HandlerFuture { Box::pin(handler(dialog)) });

		self.dialog_handlers.lock().insert(
			id,
			HandlerEntry {
				id,
				meta: (),
				handler,
			},
		);

		Subscription::from_handler_map(id, &self.dialog_handlers)
	}

	/// Returns a broadcast receiver for console messages.
	///
	/// See <https://playwright.dev/docs/api/class-page#page-event-console>
	pub fn console_messages(&self) -> broadcast::Receiver<ConsoleMessage> {
		self.console_tx.subscribe()
	}

	/// Waits for a console message matching the predicate.
	///
	/// # Errors
	///
	/// Returns [`Error::Timeout`](pw_runtime::Error::Timeout) or
	/// [`Error::ChannelClosed`](pw_runtime::Error::ChannelClosed).
	pub async fn wait_for_console<F>(
		&self,
		predicate: F,
		timeout: std::time::Duration,
	) -> Result<ConsoleMessage>
	where
		F: Fn(&ConsoleMessage) -> bool,
	{
		let mut rx = self.console_messages();

		tokio::time::timeout(timeout, async move {
			loop {
				match rx.recv().await {
					Ok(msg) if predicate(&msg) => return Ok(msg),
					Ok(_) => continue,
					Err(broadcast::error::RecvError::Lagged(n)) => {
						tracing::warn!(dropped = n, "Console message receiver lagged");
					}
					Err(broadcast::error::RecvError::Closed) => {
						return Err(Error::ChannelClosed);
					}
				}
			}
		})
		.await
		.map_err(|_| Error::Timeout("Timeout waiting for console message".to_string()))?
	}

	/// Registers a console message callback via a background task.
	///
	/// Returns a [`ConsoleSubscription`](crate::events::ConsoleSubscription) that
	/// cancels the task when dropped.
	///
	/// See <https://playwright.dev/docs/api/class-page#page-event-console>
	pub fn on_console<F>(&self, handler: F) -> crate::events::ConsoleSubscription
	where
		F: Fn(ConsoleMessage) + Send + Sync + 'static,
	{
		use tokio::sync::oneshot;

		let mut rx = self.console_messages();
		let (cancel_tx, mut cancel_rx) = oneshot::channel::<()>();

		tokio::spawn(async move {
			loop {
				tokio::select! {
					result = rx.recv() => {
						match result {
							Ok(msg) => handler(msg),
							Err(broadcast::error::RecvError::Lagged(n)) => {
								tracing::warn!(dropped = n, "Console callback lagged");
							}
							Err(broadcast::error::RecvError::Closed) => break,
						}
					}
					_ = &mut cancel_rx => break,
				}
			}
		});

		crate::events::ConsoleSubscription::new(cancel_tx)
	}

	/// Dispatches a download event to all registered handlers.
	pub(super) async fn on_download_event(&self, download: Download) {
		let handlers: Vec<_> = {
			let map = self.download_handlers.lock();
			map.values().map(|e| (e.id, e.handler.clone())).collect()
		};

		for (id, handler) in handlers {
			if let Err(e) = handler(download.clone()).await {
				tracing::error!(error = %e, handler_id = id, "Download handler error");
			}
		}
	}

	/// Dispatches a dialog event to all registered handlers.
	pub(super) async fn on_dialog_event(&self, dialog: Dialog) {
		let handlers: Vec<_> = {
			let map = self.dialog_handlers.lock();
			map.values().map(|e| (e.id, e.handler.clone())).collect()
		};

		for (id, handler) in handlers {
			if let Err(e) = handler(dialog.clone()).await {
				tracing::error!(error = %e, handler_id = id, "Dialog handler error");
			}
		}
	}

	/// Triggers a dialog event (called by [`BrowserContext`](crate::BrowserContext)).
	pub async fn trigger_dialog_event(&self, dialog: Dialog) {
		self.on_dialog_event(dialog).await;
	}
}
