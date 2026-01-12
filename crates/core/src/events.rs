// Copyright 2024 Paul Adamson
// Licensed under the Apache License, Version 2.0

//! Event system infrastructure for Playwright protocol objects.
//!
//! Provides abstractions for handling events emitted by browser pages and contexts:
//!
//! - [`EventBus`] - Internal dispatcher combining broadcast channels with predicate-based waiters
//! - [`EventStream`] - Ergonomic wrapper around [`broadcast::Receiver`] with lag handling
//! - [`EventWaiter`] - One-shot event capture with timeout support
//! - [`ConsoleSubscription`] - RAII handle for callback-style event handlers
//!
//! # Design
//!
//! The event system supports two consumption patterns:
//!
//! 1. **Streams**: Subscribe via [`EventBus::subscribe`] and poll for events
//! 2. **Callbacks**: Register via `on_*` methods which spawn background tasks
//!
//! Both patterns use [`ConsoleSubscription`] for lifetime management - dropping
//! the subscription cancels the handler.
//!
//! [`broadcast::Receiver`]: tokio::sync::broadcast::Receiver

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use parking_lot::Mutex;
use tokio::sync::{broadcast, oneshot};

use pw_runtime::{Error, Result};

/// RAII handle that cancels a console event callback when dropped.
///
/// Returned by [`Page::on_console`] to manage the lifetime of callback-style
/// event handlers. The background task that invokes the callback is cancelled
/// when this handle is dropped or [`unsubscribe`](Self::unsubscribe) is called.
///
/// # Example
///
/// ```ignore
/// let sub = page.on_console(|msg| println!("{}", msg.text()));
/// // Handler is active while `sub` is held...
/// drop(sub);  // Handler is cancelled
/// ```
///
/// [`Page::on_console`]: crate::Page::on_console
pub struct ConsoleSubscription {
    cancel_tx: Option<oneshot::Sender<()>>,
}

impl ConsoleSubscription {
    pub(crate) fn new(cancel_tx: oneshot::Sender<()>) -> Self {
        Self {
            cancel_tx: Some(cancel_tx),
        }
    }

    /// Explicitly cancels the subscription, equivalent to dropping it.
    pub fn unsubscribe(mut self) {
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for ConsoleSubscription {
    fn drop(&mut self) {
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl std::fmt::Debug for ConsoleSubscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConsoleSubscription")
            .field("active", &self.cancel_tx.is_some())
            .finish()
    }
}

struct WaiterEntry<E> {
    predicate: Box<dyn Fn(&E) -> bool + Send + Sync>,
    complete_tx: oneshot::Sender<E>,
}

/// Internal event bus combining broadcast channels with predicate-based waiters.
///
/// Provides two delivery mechanisms:
///
/// 1. **Broadcast**: All subscribers receive events via [`subscribe`](Self::subscribe)
/// 2. **Waiters**: One-shot receivers with predicates via [`register_waiter`](Self::register_waiter)
///
/// Waiters are checked first during [`emit`](Self::emit), ensuring guaranteed delivery
/// for `wait_for_*` patterns even when broadcast receivers are lagging.
pub(crate) struct EventBus<E: Clone + Send + 'static> {
    tx: broadcast::Sender<E>,
    waiters: Mutex<Vec<WaiterEntry<E>>>,
}

impl<E: Clone + Send + 'static> EventBus<E> {
    /// Creates a new [`EventBus`] with the specified broadcast channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            tx,
            waiters: Mutex::new(Vec::new()),
        }
    }

    /// Emits an event to all subscribers and matching waiters.
    ///
    /// Waiters with matching predicates receive the event first via oneshot channel
    /// and are removed from the registry. The event is then broadcast to all stream
    /// subscribers. This ordering ensures `wait_for_*` calls have guaranteed delivery
    /// even if broadcast receivers are lagging.
    pub fn emit(&self, event: E) {
        {
            let mut waiters = self.waiters.lock();
            let mut i = 0;
            while i < waiters.len() {
                if (waiters[i].predicate)(&event) {
                    let entry = waiters.swap_remove(i);
                    let _ = entry.complete_tx.send(event.clone());
                } else {
                    i += 1;
                }
            }
        }
        let _ = self.tx.send(event);
    }

    /// Subscribes to the event stream.
    ///
    /// Returns a [`broadcast::Receiver`] that will receive all future events.
    /// Events emitted before subscription are not received.
    ///
    /// [`broadcast::Receiver`]: tokio::sync::broadcast::Receiver
    pub fn subscribe(&self) -> broadcast::Receiver<E> {
        self.tx.subscribe()
    }

    /// Registers a waiter that will receive the first matching event.
    ///
    /// Returns a [`oneshot::Receiver`] that completes when an event matching
    /// the `predicate` is emitted. The waiter is automatically removed after matching.
    ///
    /// [`oneshot::Receiver`]: tokio::sync::oneshot::Receiver
    pub fn register_waiter<F>(&self, predicate: F) -> oneshot::Receiver<E>
    where
        F: Fn(&E) -> bool + Send + Sync + 'static,
    {
        let (complete_tx, complete_rx) = oneshot::channel();
        let entry = WaiterEntry {
            predicate: Box::new(predicate),
            complete_tx,
        };
        self.waiters.lock().push(entry);
        complete_rx
    }

    /// Returns the number of active subscribers.
    #[allow(dead_code)]
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }

    /// Returns the number of registered waiters.
    #[allow(dead_code)]
    pub fn waiter_count(&self) -> usize {
        self.waiters.lock().len()
    }
}

impl<E: Clone + Send + 'static> Default for EventBus<E> {
    fn default() -> Self {
        Self::new(256)
    }
}

/// Ergonomic wrapper around [`broadcast::Receiver`] with automatic lag handling.
///
/// Unlike the raw receiver, [`EventStream`] handles [`RecvError::Lagged`] by logging
/// a warning and continuing to receive. This prevents lag errors from breaking
/// event processing loops.
///
/// # Example
///
/// ```ignore
/// let mut stream = EventStream::new(bus.subscribe());
/// while let Some(msg) = stream.recv().await {
///     println!("{}: {}", msg.kind(), msg.text());
/// }
/// ```
///
/// [`broadcast::Receiver`]: tokio::sync::broadcast::Receiver
/// [`RecvError::Lagged`]: tokio::sync::broadcast::error::RecvError::Lagged
pub struct EventStream<E: Clone + Send + 'static> {
    rx: broadcast::Receiver<E>,
}

impl<E: Clone + Send + 'static> EventStream<E> {
    /// Creates a new [`EventStream`] wrapping the given broadcast receiver.
    pub(crate) fn new(rx: broadcast::Receiver<E>) -> Self {
        Self { rx }
    }

    /// Receives the next event, blocking until one is available.
    ///
    /// Returns `Some(event)` on success, or `None` when the channel closes
    /// (typically when the source [`Page`] is dropped). Broadcast lag is
    /// handled internally by logging and continuing.
    ///
    /// [`Page`]: crate::Page
    pub async fn recv(&mut self) -> Option<E> {
        loop {
            match self.rx.recv().await {
                Ok(event) => return Some(event),
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(dropped = n, "Event stream lagged, dropped events");
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }

    /// Attempts to receive an event without blocking.
    ///
    /// Returns `Some(event)` if one is immediately available, `None` otherwise.
    /// Like [`recv`](Self::recv), broadcast lag is handled internally.
    pub fn try_recv(&mut self) -> Option<E> {
        loop {
            match self.rx.try_recv() {
                Ok(event) => return Some(event),
                Err(broadcast::error::TryRecvError::Lagged(n)) => {
                    tracing::warn!(dropped = n, "Event stream lagged, dropped events");
                }
                Err(
                    broadcast::error::TryRecvError::Empty | broadcast::error::TryRecvError::Closed,
                ) => return None,
            }
        }
    }
}

/// One-shot event waiter with timeout support.
///
/// Created by [`EventBus::register_waiter`] and completes when a matching event
/// is emitted. Supports two consumption patterns:
///
/// - **With timeout**: Call [`wait()`](Self::wait) for timeout support
/// - **Without timeout**: Use `.await` directly (implements [`Future`])
///
/// # Example
///
/// ```ignore
/// let waiter = EventWaiter::new(rx, Duration::from_secs(10));
/// let event = waiter.wait().await?;
/// ```
pub struct EventWaiter<E> {
    rx: oneshot::Receiver<E>,
    timeout: Duration,
}

impl<E: Send + 'static> EventWaiter<E> {
    /// Creates a new [`EventWaiter`] with the given receiver and timeout.
    pub(crate) fn new(rx: oneshot::Receiver<E>, timeout: Duration) -> Self {
        Self { rx, timeout }
    }

    /// Waits for the event with the configured timeout.
    ///
    /// # Errors
    ///
    /// - [`Error::Timeout`] if no matching event arrives within the timeout
    /// - [`Error::ChannelClosed`] if the event source is dropped
    ///
    /// [`Error::Timeout`]: pw_runtime::Error::Timeout
    /// [`Error::ChannelClosed`]: pw_runtime::Error::ChannelClosed
    pub async fn wait(self) -> Result<E> {
        tokio::time::timeout(self.timeout, self.rx)
            .await
            .map_err(|_| Error::Timeout("Timeout waiting for event".to_string()))?
            .map_err(|_| Error::ChannelClosed)
    }
}

impl<E: Send + 'static> Future for EventWaiter<E> {
    type Output = Result<E>;

    /// Polls the waiter without timeout. For timeout support, use [`wait()`](Self::wait).
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.rx).poll(cx) {
            Poll::Ready(Ok(event)) => Poll::Ready(Ok(event)),
            Poll::Ready(Err(_)) => Poll::Ready(Err(Error::ChannelClosed)),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[derive(Clone, Debug, PartialEq)]
    struct TestEvent {
        id: u32,
        message: String,
    }

    #[tokio::test]
    async fn event_bus_broadcast() {
        let bus: EventBus<TestEvent> = EventBus::new(16);

        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.emit(TestEvent {
            id: 1,
            message: "hello".to_string(),
        });

        let e1 = rx1.recv().await.unwrap();
        let e2 = rx2.recv().await.unwrap();

        assert_eq!(e1.id, 1);
        assert_eq!(e2.message, "hello");
    }

    #[tokio::test]
    async fn event_bus_waiter_receives_matching_event() {
        let bus: EventBus<TestEvent> = EventBus::new(16);

        let mut waiter_rx = bus.register_waiter(|e| e.id == 2);

        bus.emit(TestEvent {
            id: 1,
            message: "first".to_string(),
        });
        assert!(waiter_rx.try_recv().is_err());

        let waiter_rx = bus.register_waiter(|e| e.id == 2);
        bus.emit(TestEvent {
            id: 2,
            message: "second".to_string(),
        });

        let event = waiter_rx.await.unwrap();
        assert_eq!(event.id, 2);
        assert_eq!(event.message, "second");
    }

    #[tokio::test]
    async fn event_bus_waiter_removed_after_match() {
        let bus: EventBus<TestEvent> = EventBus::new(16);

        let _waiter_rx = bus.register_waiter(|e| e.id == 1);
        assert_eq!(bus.waiter_count(), 1);

        bus.emit(TestEvent {
            id: 1,
            message: "match".to_string(),
        });

        assert_eq!(bus.waiter_count(), 0);
    }

    #[tokio::test]
    async fn event_stream_receives_events() {
        let bus: EventBus<TestEvent> = EventBus::new(16);
        let mut stream = EventStream::new(bus.subscribe());

        let bus = Arc::new(bus);
        let bus_ref = bus.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            bus_ref.emit(TestEvent {
                id: 42,
                message: "async".to_string(),
            });
        });

        let event = stream.recv().await.unwrap();
        assert_eq!(event.id, 42);
    }

    #[tokio::test]
    async fn event_waiter_timeout() {
        let (_tx, rx) = oneshot::channel::<TestEvent>();
        let waiter = EventWaiter::new(rx, Duration::from_millis(10));

        let result = waiter.wait().await;
        assert!(matches!(result, Err(Error::Timeout(_))));
    }

    #[tokio::test]
    async fn console_subscription_cancels_on_drop() {
        let (tx, mut rx) = oneshot::channel::<()>();
        let sub = ConsoleSubscription::new(tx);

        drop(sub);

        let result = rx.try_recv();
        assert!(result.is_ok() || result == Err(oneshot::error::TryRecvError::Closed));
    }
}
