//! ChannelOwner - Base trait for all Playwright protocol objects.
//!
//! All Playwright objects (Browser, Page, etc.) implement ChannelOwner to:
//! - Represent remote objects on the server via GUID
//! - Participate in parent-child lifecycle management
//! - Handle protocol events
//! - Communicate via Channel proxy

use crate::channel::Channel;
use crate::connection::ConnectionLike;
use downcast_rs::{DowncastSync, impl_downcast};
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

/// Private module for the sealed trait pattern.
pub mod private {
    /// Marker trait that seals `ChannelOwner`.
    pub trait Sealed {}
}

/// Type alias for the children registry.
type ChildrenRegistry = HashMap<Arc<str>, Arc<dyn ChannelOwner>>;

/// Reason why an object was disposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisposeReason {
    /// Object was explicitly closed by user code.
    Closed,
    /// Object was garbage collected by the server.
    GarbageCollected,
}

/// Parent can be either another ChannelOwner or the root Connection.
pub enum ParentOrConnection {
    Parent(Arc<dyn ChannelOwner>),
    Connection(Arc<dyn ConnectionLike>),
}

/// Base trait for all Playwright protocol objects.
///
/// This trait is sealed - it cannot be implemented outside of pw-rs crates.
pub trait ChannelOwner: private::Sealed + DowncastSync {
    /// Returns the unique GUID for this object.
    fn guid(&self) -> &str;

    /// Returns the protocol type name (e.g., "Browser", "Page").
    fn type_name(&self) -> &str;

    /// Returns the parent object, if any.
    fn parent(&self) -> Option<Arc<dyn ChannelOwner>>;

    /// Returns the connection this object belongs to.
    fn connection(&self) -> Arc<dyn ConnectionLike>;

    /// Returns the raw initializer JSON from the server.
    fn initializer(&self) -> &Value;

    /// Returns the channel for RPC communication.
    fn channel(&self) -> &Channel;

    /// Disposes this object and all its children.
    fn dispose(&self, reason: DisposeReason);

    /// Adopts a child object (moves from old parent to this parent).
    fn adopt(&self, child: Arc<dyn ChannelOwner>);

    /// Adds a child object to this parent's registry.
    fn add_child(&self, guid: Arc<str>, child: Arc<dyn ChannelOwner>);

    /// Removes a child object from this parent's registry.
    fn remove_child(&self, guid: &str);

    /// Handles a protocol event sent to this object.
    fn on_event(&self, method: &str, params: Value);

    /// Returns true if this object was garbage collected.
    fn was_collected(&self) -> bool;
}

impl_downcast!(sync ChannelOwner);

/// Base implementation of ChannelOwner that can be embedded in protocol objects.
pub struct ChannelOwnerImpl {
    guid: Arc<str>,
    type_name: String,
    parent: Option<Weak<dyn ChannelOwner>>,
    connection: Arc<dyn ConnectionLike>,
    children: Arc<Mutex<ChildrenRegistry>>,
    channel: Channel,
    initializer: Value,
    was_collected: AtomicBool,
}

impl Clone for ChannelOwnerImpl {
    fn clone(&self) -> Self {
        Self {
            guid: self.guid.clone(),
            type_name: self.type_name.clone(),
            parent: self.parent.clone(),
            connection: Arc::clone(&self.connection),
            children: Arc::clone(&self.children),
            channel: self.channel.clone(),
            initializer: self.initializer.clone(),
            was_collected: AtomicBool::new(self.was_collected.load(Ordering::SeqCst)),
        }
    }
}

impl ChannelOwnerImpl {
    /// Creates a new ChannelOwner base implementation.
    pub fn new(
        parent: ParentOrConnection,
        type_name: String,
        guid: Arc<str>,
        initializer: Value,
    ) -> Self {
        let (connection, parent_opt) = match parent {
            ParentOrConnection::Parent(p) => {
                let conn = p.connection();
                (conn, Some(Arc::downgrade(&p)))
            }
            ParentOrConnection::Connection(c) => (c, None),
        };

        let channel = Channel::new(Arc::clone(&guid), connection.clone());

        Self {
            guid,
            type_name,
            parent: parent_opt,
            connection,
            children: Arc::new(Mutex::new(HashMap::new())),
            channel,
            initializer,
            was_collected: AtomicBool::new(false),
        }
    }

    /// Returns the unique GUID for this object.
    pub fn guid(&self) -> &str {
        &self.guid
    }

    /// Returns the protocol type name.
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Returns the parent object, if any.
    pub fn parent(&self) -> Option<Arc<dyn ChannelOwner>> {
        self.parent.as_ref().and_then(|p| p.upgrade())
    }

    /// Returns the connection.
    pub fn connection(&self) -> Arc<dyn ConnectionLike> {
        self.connection.clone()
    }

    /// Returns the initializer JSON.
    pub fn initializer(&self) -> &Value {
        &self.initializer
    }

    /// Returns the channel for RPC.
    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    /// Disposes this object and all children recursively.
    pub fn dispose(&self, reason: DisposeReason) {
        if reason == DisposeReason::GarbageCollected {
            self.was_collected.store(true, Ordering::SeqCst);
        }

        if let Some(parent) = self.parent() {
            parent.remove_child(&self.guid);
        }

        self.connection.unregister_object(&self.guid);

        let children: Vec<_> = {
            let guard = self.children.lock();
            guard.values().cloned().collect()
        };

        for child in children {
            child.dispose(reason);
        }

        self.children.lock().clear();
    }

    /// Adopts a child object (moves from old parent to this parent).
    pub fn adopt(&self, child: Arc<dyn ChannelOwner>) {
        if let Some(old_parent) = child.parent() {
            old_parent.remove_child(child.guid());
        }
        self.add_child(Arc::from(child.guid()), child);
    }

    /// Adds a child to this parent's registry.
    pub fn add_child(&self, guid: Arc<str>, child: Arc<dyn ChannelOwner>) {
        self.children.lock().insert(guid, child);
    }

    /// Removes a child from this parent's registry.
    pub fn remove_child(&self, guid: &str) {
        let guid_arc: Arc<str> = Arc::from(guid);
        self.children.lock().remove(&guid_arc);
    }

    /// Returns all children of this object.
    pub fn children(&self) -> Vec<Arc<dyn ChannelOwner>> {
        self.children.lock().values().cloned().collect()
    }

    /// Handles a protocol event (default implementation logs it).
    pub fn on_event(&self, method: &str, params: Value) {
        tracing::debug!(
            "Event on {} ({}): {} -> {:?}",
            self.guid,
            self.type_name,
            method,
            params
        );
    }

    /// Returns true if object was garbage collected.
    pub fn was_collected(&self) -> bool {
        self.was_collected.load(Ordering::SeqCst)
    }
}
