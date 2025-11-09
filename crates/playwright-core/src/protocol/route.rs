// Route protocol object
//
// Represents a route handler for network interception.
// Routes are created when page.route() matches a request.
//
// See: https://playwright.dev/docs/api/class-route

use crate::channel_owner::{ChannelOwner, ChannelOwnerImpl, ParentOrConnection};
use crate::error::Result;
use crate::protocol::Request;
use serde_json::{json, Value};
use std::any::Any;
use std::sync::Arc;

/// Route represents a network route handler.
///
/// Routes allow intercepting, aborting, continuing, or fulfilling network requests.
///
/// See: <https://playwright.dev/docs/api/class-route>
#[derive(Clone)]
pub struct Route {
    base: ChannelOwnerImpl,
}

impl Route {
    /// Creates a new Route from protocol initialization
    ///
    /// This is called by the object factory when the server sends a `__create__` message
    /// for a Route object.
    pub fn new(
        parent: Arc<dyn ChannelOwner>,
        type_name: String,
        guid: String,
        initializer: Value,
    ) -> Result<Self> {
        let base = ChannelOwnerImpl::new(
            ParentOrConnection::Parent(parent.clone()),
            type_name,
            guid,
            initializer,
        );

        Ok(Self { base })
    }

    /// Returns the request that is being routed.
    ///
    /// See: <https://playwright.dev/docs/api/class-route#route-request>
    pub fn request(&self) -> Request {
        // The Route's parent is the Request object
        // Try to downcast the parent to Request
        if let Some(parent) = self.parent() {
            if let Some(request) = parent.as_any().downcast_ref::<Request>() {
                return request.clone();
            }
        }

        // Fallback: Create a stub Request from initializer data
        // This should rarely happen in practice
        let request_data = self
            .initializer()
            .get("request")
            .cloned()
            .unwrap_or_else(|| {
                serde_json::json!({
                    "url": "",
                    "method": "GET"
                })
            });

        let parent = self
            .parent()
            .unwrap_or_else(|| Arc::new(self.clone()) as Arc<dyn ChannelOwner>);

        let request_guid = request_data
            .get("guid")
            .and_then(|v| v.as_str())
            .unwrap_or("request-stub");

        Request::new(
            parent,
            "Request".to_string(),
            request_guid.to_string(),
            request_data,
        )
        .unwrap()
    }

    /// Aborts the route's request.
    ///
    /// # Arguments
    ///
    /// * `error_code` - Optional error code (default: "failed")
    ///
    /// Available error codes:
    /// - "aborted" - User-initiated cancellation
    /// - "accessdenied" - Permission denied
    /// - "addressunreachable" - Host unreachable
    /// - "blockedbyclient" - Client blocked request
    /// - "connectionaborted", "connectionclosed", "connectionfailed", "connectionrefused", "connectionreset"
    /// - "internetdisconnected"
    /// - "namenotresolved"
    /// - "timedout"
    /// - "failed" - Generic error (default)
    ///
    /// See: <https://playwright.dev/docs/api/class-route#route-abort>
    pub async fn abort(&self, error_code: Option<&str>) -> Result<()> {
        let params = json!({
            "errorCode": error_code.unwrap_or("failed")
        });

        self.channel()
            .send::<_, serde_json::Value>("abort", params)
            .await
            .map(|_| ())
    }

    /// Continues the route's request with optional modifications.
    ///
    /// # Arguments
    ///
    /// * `overrides` - Optional modifications to apply to the request
    ///
    /// See: <https://playwright.dev/docs/api/class-route#route-continue>
    pub async fn continue_(&self, _overrides: Option<ContinueOptions>) -> Result<()> {
        // For now, just continue without modifications
        // TODO: Support overrides in future implementation
        let params = json!({
            "isFallback": false
        });

        self.channel()
            .send::<_, serde_json::Value>("continue", params)
            .await
            .map(|_| ())
    }
}

/// Options for continuing a request with modifications.
///
/// See: <https://playwright.dev/docs/api/class-route#route-continue>
#[derive(Debug, Clone, Default)]
pub struct ContinueOptions {
    // TODO: Add fields for request modifications
    // pub headers: Option<HashMap<String, String>>,
    // pub method: Option<String>,
    // pub post_data: Option<Vec<u8>>,
    // pub url: Option<String>,
}

impl ChannelOwner for Route {
    fn guid(&self) -> &str {
        self.base.guid()
    }

    fn type_name(&self) -> &str {
        self.base.type_name()
    }

    fn parent(&self) -> Option<Arc<dyn ChannelOwner>> {
        self.base.parent()
    }

    fn connection(&self) -> Arc<dyn crate::connection::ConnectionLike> {
        self.base.connection()
    }

    fn initializer(&self) -> &Value {
        self.base.initializer()
    }

    fn channel(&self) -> &crate::channel::Channel {
        self.base.channel()
    }

    fn dispose(&self, reason: crate::channel_owner::DisposeReason) {
        self.base.dispose(reason)
    }

    fn adopt(&self, child: Arc<dyn ChannelOwner>) {
        self.base.adopt(child)
    }

    fn add_child(&self, guid: String, child: Arc<dyn ChannelOwner>) {
        self.base.add_child(guid, child)
    }

    fn remove_child(&self, guid: &str) {
        self.base.remove_child(guid)
    }

    fn on_event(&self, _method: &str, _params: Value) {
        // Route events will be handled in future phases
    }

    fn was_collected(&self) -> bool {
        self.base.was_collected()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl std::fmt::Debug for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Route")
            .field("guid", &self.guid())
            .field("request", &self.request().guid())
            .finish()
    }
}
