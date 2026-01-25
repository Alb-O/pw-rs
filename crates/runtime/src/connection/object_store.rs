//! Thread-safe object registry with per-GUID notification.
//!
//! Uses [`DashMap`] for lock-free concurrent access. Per-GUID [`Notify`]
//! ensures only relevant waiters wake up, and [`ObjectStore::wait_for`]
//! registers waiters before checking to prevent lost wakeups.

use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tokio::sync::Notify;

use crate::channel_owner::ChannelOwner;
use crate::error::{Error, Result};

/// Thread-safe registry of protocol objects by GUID.
pub struct ObjectStore {
    objects: DashMap<Arc<str>, Arc<dyn ChannelOwner>>,
    waiters: DashMap<Arc<str>, Arc<Notify>>,
}

impl Default for ObjectStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjectStore {
    pub fn new() -> Self {
        Self {
            objects: DashMap::new(),
            waiters: DashMap::new(),
        }
    }

    /// Inserts an object and notifies any waiters for this GUID.
    pub fn insert(&self, guid: Arc<str>, obj: Arc<dyn ChannelOwner>) {
        self.objects.insert(guid.clone(), obj);
        if let Some((_, notify)) = self.waiters.remove(&guid) {
            notify.notify_waiters();
        }
    }

    pub fn remove(&self, guid: &str) {
        self.objects.remove(&Arc::from(guid) as &Arc<str>);
    }

    /// Synchronous lookup.
    pub fn try_get(&self, guid: &str) -> Option<Arc<dyn ChannelOwner>> {
        self.objects
            .get(&Arc::from(guid) as &Arc<str>)
            .map(|r| r.value().clone())
    }

    /// Waits for an object to be registered, with timeout.
    ///
    /// Registers waiter before checking to prevent lost wakeups.
    pub async fn wait_for(&self, guid: &str, timeout: Duration) -> Result<Arc<dyn ChannelOwner>> {
        let g: Arc<str> = Arc::from(guid);
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let notify = self
                .waiters
                .entry(g.clone())
                .or_insert_with(|| Arc::new(Notify::new()))
                .clone();
            let notified = notify.notified();

            if let Some(obj) = self.objects.get(&g) {
                return Ok(obj.value().clone());
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(Self::timeout_error(&g));
            }

            tokio::select! {
                biased;
                _ = notified => {}
                _ = tokio::time::sleep(remaining) => {
                    return Err(Self::timeout_error(&g));
                }
            }
        }
    }

    fn timeout_error(guid: &str) -> Error {
        let target_type = match () {
            _ if guid.starts_with("page@") => "Page",
            _ if guid.starts_with("frame@") => "Frame",
            _ if guid.starts_with("browser-context@") => "BrowserContext",
            _ if guid.starts_with("browser@") => "Browser",
            _ if guid.starts_with("response@") => "Response",
            _ => return Error::Timeout(format!("Timeout waiting for object: {guid}")),
        };
        Error::Timeout(format!("Timeout waiting for {target_type} object: {guid}"))
    }
}
