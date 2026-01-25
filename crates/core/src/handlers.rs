//! Generic event handler infrastructure.
//!
//! Unified types for event handlers and subscriptions using [`HandlerEntry<E, M>`]
//! with [`IndexMap`] storage for O(1) removal and stable insertion order.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};

use indexmap::IndexMap;
use parking_lot::Mutex;

/// Unique identifier for event handlers.
pub type HandlerId = u64;

static NEXT_HANDLER_ID: AtomicU64 = AtomicU64::new(1);

/// Returns a new globally-unique handler ID.
pub fn next_handler_id() -> HandlerId {
	NEXT_HANDLER_ID.fetch_add(1, Ordering::SeqCst)
}

/// Boxed async handler future.
pub type HandlerFuture = Pin<Box<dyn Future<Output = pw_runtime::Result<()>> + Send>>;

/// Handler function: `E` → async `Result<()>`.
pub type HandlerFn<E> = Arc<dyn Fn(E) -> HandlerFuture + Send + Sync>;

/// Event handler entry with optional metadata `M`.
///
/// - `E`: event type ([`Route`], [`Download`], [`Dialog`])
/// - `M`: metadata (e.g., [`RouteMeta`] for compiled matchers)
///
/// [`Route`]: crate::Route
/// [`Download`]: crate::Download
/// [`Dialog`]: crate::Dialog
pub struct HandlerEntry<E, M = ()> {
	pub id: HandlerId,
	pub meta: M,
	pub handler: HandlerFn<E>,
}

impl<E, M: Clone> Clone for HandlerEntry<E, M> {
	fn clone(&self) -> Self {
		Self {
			id: self.id,
			meta: self.meta.clone(),
			handler: Arc::clone(&self.handler),
		}
	}
}

/// Handler storage: [`IndexMap`] for O(1) removal with stable insertion order.
pub type HandlerMap<E, M = ()> = Arc<Mutex<IndexMap<HandlerId, HandlerEntry<E, M>>>>;

/// Compiled glob pattern for URL matching.
///
/// Compiles once at registration; invalid patterns fall back to exact matching.
#[derive(Clone)]
pub struct RouteMatcher {
	pattern: glob::Pattern,
}

impl RouteMatcher {
	/// Compiles a glob pattern, falling back to literal matching on invalid patterns.
	pub fn new(pattern: &str) -> Self {
		let pattern = glob::Pattern::new(pattern).unwrap_or_else(|_| {
			glob::Pattern::new(&glob::Pattern::escape(pattern))
				.expect("escaped pattern is always valid")
		});
		Self { pattern }
	}

	/// Returns `true` if the URL matches this pattern.
	pub fn is_match(&self, url: &str) -> bool {
		self.pattern.matches(url)
	}

	/// Returns the pattern string.
	pub fn as_str(&self) -> &str {
		self.pattern.as_str()
	}
}

/// Route handler metadata containing the compiled [`RouteMatcher`].
#[derive(Clone)]
pub struct RouteMeta {
	pub matcher: RouteMatcher,
}

/// RAII handle that unregisters an event handler on drop.
///
/// Holds a weak reference to the handler map, so dropping after the owning
/// [`Page`] is closed is safe (becomes a no-op).
///
/// [`Page`]: crate::Page
pub struct Subscription {
	id: HandlerId,
	dropper: Option<Arc<dyn Fn(HandlerId) + Send + Sync>>,
}

impl Subscription {
	/// Creates a subscription with a custom dropper function.
	pub fn new(id: HandlerId, dropper: Arc<dyn Fn(HandlerId) + Send + Sync>) -> Self {
		Self {
			id,
			dropper: Some(dropper),
		}
	}

	/// Creates a subscription from a handler map using a weak reference.
	pub fn from_handler_map<E, M>(id: HandlerId, handlers: &HandlerMap<E, M>) -> Self
	where
		E: Send + Sync + 'static,
		M: Send + Sync + 'static,
	{
		let weak: Weak<Mutex<IndexMap<HandlerId, HandlerEntry<E, M>>>> = Arc::downgrade(handlers);
		let dropper = Arc::new(move |id: HandlerId| {
			if let Some(map) = weak.upgrade() {
				map.lock().shift_remove(&id);
			}
		});
		Self::new(id, dropper)
	}

	/// Returns this subscription's handler ID.
	pub fn id(&self) -> HandlerId {
		self.id
	}

	/// Explicitly unsubscribes. Equivalent to dropping.
	pub fn unsubscribe(mut self) {
		if let Some(dropper) = self.dropper.take() {
			(dropper)(self.id);
		}
	}
}

impl Drop for Subscription {
	fn drop(&mut self) {
		if let Some(dropper) = self.dropper.take() {
			(dropper)(self.id);
		}
	}
}

impl std::fmt::Debug for Subscription {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Subscription")
			.field("id", &self.id)
			.field("active", &self.dropper.is_some())
			.finish()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_handler_id_increments() {
		let id1 = next_handler_id();
		let id2 = next_handler_id();
		let id3 = next_handler_id();
		assert!(id2 > id1);
		assert!(id3 > id2);
	}

	#[test]
	fn test_route_matcher_glob() {
		let matcher = RouteMatcher::new("**/*.png");
		assert!(matcher.is_match("https://example.com/image.png"));
		assert!(matcher.is_match("https://example.com/path/to/image.png"));
		assert!(!matcher.is_match("https://example.com/image.jpg"));
	}

	#[test]
	fn test_route_matcher_exact() {
		let matcher = RouteMatcher::new("https://example.com/api");
		assert!(matcher.is_match("https://example.com/api"));
		assert!(!matcher.is_match("https://example.com/api/v2"));
	}

	#[test]
	fn test_subscription_unsubscribe() {
		use std::sync::atomic::{AtomicBool, Ordering};

		let called = Arc::new(AtomicBool::new(false));
		let called_clone = Arc::clone(&called);

		let dropper = Arc::new(move |_id: HandlerId| {
			called_clone.store(true, Ordering::SeqCst);
		});

		let sub = Subscription::new(1, dropper);
		assert!(!called.load(Ordering::SeqCst));

		sub.unsubscribe();
		assert!(called.load(Ordering::SeqCst));
	}

	#[test]
	fn test_subscription_drop() {
		use std::sync::atomic::{AtomicBool, Ordering};

		let called = Arc::new(AtomicBool::new(false));
		let called_clone = Arc::clone(&called);

		let dropper = Arc::new(move |_id: HandlerId| {
			called_clone.store(true, Ordering::SeqCst);
		});

		{
			let _sub = Subscription::new(1, dropper);
			assert!(!called.load(Ordering::SeqCst));
		}
		// Subscription dropped here
		assert!(called.load(Ordering::SeqCst));
	}

	#[test]
	fn test_subscription_from_handler_map() {
		// Create a handler map
		let map: HandlerMap<String> = Arc::new(Mutex::new(IndexMap::new()));

		// Insert a handler
		let id = next_handler_id();
		map.lock().insert(
			id,
			HandlerEntry {
				id,
				meta: (),
				handler: Arc::new(|_: String| Box::pin(async { Ok(()) })),
			},
		);
		assert_eq!(map.lock().len(), 1);

		// Create subscription and drop it
		{
			let _sub = Subscription::from_handler_map(id, &map);
		}

		// Handler should be removed
		assert_eq!(map.lock().len(), 0);
	}

	#[test]
	fn test_subscription_weak_reference() {
		// Create a handler map
		let map: HandlerMap<String> = Arc::new(Mutex::new(IndexMap::new()));

		// Insert a handler
		let id = next_handler_id();
		map.lock().insert(
			id,
			HandlerEntry {
				id,
				meta: (),
				handler: Arc::new(|_: String| Box::pin(async { Ok(()) })),
			},
		);

		// Create subscription
		let sub = Subscription::from_handler_map(id, &map);

		// Drop the map before the subscription
		drop(map);

		// Dropping subscription should not panic (weak ref is dead)
		drop(sub);
	}
}
