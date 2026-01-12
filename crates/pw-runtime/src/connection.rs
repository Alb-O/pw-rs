//! JSON-RPC Connection layer for Playwright protocol
//!
//! This module implements the request/response correlation layer on top of the transport.
//! It handles:
//! - Generating unique request IDs
//! - Correlating responses with pending requests
//! - Distinguishing events from responses
//! - Dispatching events to protocol objects
//!
//! # Message Flow
//!
//! 1. Client calls `send_message()` with GUID, method, and params
//! 2. Connection generates unique ID and creates oneshot channel
//! 3. Request is serialized and sent via transport
//! 4. Client awaits on the oneshot receiver
//! 5. Message loop receives response from transport
//! 6. Response is correlated by ID and sent via oneshot channel
//! 7. Client receives result

use crate::channel_owner::{ChannelOwner, DisposeReason, ParentOrConnection};
use crate::error::{Error, Result};
use crate::transport::{Transport, TransportParts, TransportReceiver};
use parking_lot::Mutex as ParkingLotMutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::Mutex as TokioMutex;
use tokio::sync::{Notify, mpsc, oneshot};

/// Trait defining the interface that ChannelOwner needs from a Connection
///
/// This trait allows ChannelOwner to work with Connection without needing to know
/// the generic parameters W and R. The Connection struct implements this trait.
pub trait ConnectionLike: Send + Sync {
    /// Send a message to the Playwright server and await response
    fn send_message(
        &self,
        guid: &str,
        method: &str,
        params: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + '_>>;

    /// Register an object in the connection's registry
    fn register_object(
        &self,
        guid: Arc<str>,
        object: Arc<dyn ChannelOwner>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;

    /// Unregister an object from the connection's registry (synchronous)
    ///
    /// This is intentionally synchronous to allow calling from Drop impls
    /// and dispose() without needing a runtime or spawning tasks.
    fn unregister_object(&self, guid: &str);

    /// Get an object by GUID
    fn get_object(&self, guid: &str) -> AsyncChannelOwnerResult<'_>;

    /// Wait for an object to be registered, with timeout
    ///
    /// This is useful when a response contains a GUID reference to an object
    /// that might not have been created yet (the `__create__` event may arrive
    /// after the response).
    ///
    /// Uses notification-based waiting rather than polling for efficiency.
    fn wait_for_object(
        &self,
        guid: &str,
        timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = Result<Arc<dyn ChannelOwner>>> + Send + '_>>;
}

/// Type alias for complex async return type
pub type AsyncChannelOwnerResult<'a> =
    Pin<Box<dyn Future<Output = Result<Arc<dyn ChannelOwner>>> + Send + 'a>>;

/// Factory trait for creating protocol objects.
///
/// This trait decouples the Connection from specific protocol object types,
/// allowing pw-runtime to be independent of pw-api. The factory is implemented
/// in pw-api and passed to Connection during initialization.
pub trait ObjectFactory: Send + Sync {
    /// Create a protocol object from a `__create__` message.
    ///
    /// # Arguments
    /// * `parent` - The parent object or connection
    /// * `type_name` - Protocol type name (e.g., "Browser", "Page")
    /// * `guid` - Unique identifier for the object
    /// * `initializer` - JSON initializer from the server
    ///
    /// # Returns
    /// The created protocol object, or an error if the type is unknown.
    fn create_object(
        &self,
        parent: ParentOrConnection,
        type_name: String,
        guid: Arc<str>,
        initializer: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Arc<dyn ChannelOwner>>> + Send + '_>>;
}

/// Metadata attached to every Playwright protocol message
///
/// Contains timing information and optional location data for debugging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    /// Unix timestamp in milliseconds
    #[serde(rename = "wallTime")]
    pub wall_time: i64,
    /// Whether this is an internal call (not user-facing API)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal: Option<bool>,
    /// Source location where the API was called
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    /// Optional title for the operation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Source code location for a protocol call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    /// Source file path
    pub file: String,
    /// Line number (1-indexed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<i32>,
    /// Column number (1-indexed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<i32>,
}

impl Metadata {
    /// Create minimal metadata with current timestamp
    pub fn now() -> Self {
        Self {
            wall_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
            internal: Some(false),
            location: None,
            title: None,
        }
    }
}

/// Protocol request message sent to Playwright server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Unique request ID for correlating responses
    pub id: u32,
    /// GUID of the target object (format: "type@hash")
    #[serde(
        serialize_with = "serialize_arc_str",
        deserialize_with = "deserialize_arc_str"
    )]
    pub guid: Arc<str>,
    /// Method name to invoke
    pub method: String,
    /// Method parameters as JSON object
    pub params: Value,
    /// Metadata with timing and location information
    pub metadata: Metadata,
}

/// Serde helpers for `Arc<str>` serialization
pub fn serialize_arc_str<S>(arc: &Arc<str>, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(arc)
}

pub fn deserialize_arc_str<'de, D>(deserializer: D) -> std::result::Result<Arc<str>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = serde::Deserialize::deserialize(deserializer)?;
    Ok(Arc::from(s.as_str()))
}

/// Protocol response message from Playwright server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Request ID this response correlates to
    pub id: u32,
    /// Success result (mutually exclusive with error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error result (mutually exclusive with result)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorWrapper>,
}

/// Wrapper for protocol error payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorWrapper {
    pub error: ErrorPayload,
}

/// Protocol error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    /// Error message
    pub message: String,
    /// Error type name (e.g., "TimeoutError", "TargetClosedError")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Stack trace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
}

/// Protocol event message from Playwright server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// GUID of the object that emitted the event
    #[serde(
        serialize_with = "serialize_arc_str",
        deserialize_with = "deserialize_arc_str"
    )]
    pub guid: Arc<str>,
    /// Event method name
    pub method: String,
    /// Event parameters as JSON object
    pub params: Value,
}

/// Discriminated union of protocol messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
    /// Response message (has `id` field)
    Response(Response),
    /// Event message (no `id` field)
    Event(Event),
    /// Unknown message type (forward-compatible catch-all)
    Unknown(Value),
}

/// Type alias for the object registry mapping GUIDs to ChannelOwner objects
type ObjectRegistry = HashMap<Arc<str>, Arc<dyn ChannelOwner>>;

/// Pending request callbacks keyed by request ID.
type CallbackMap = Arc<TokioMutex<HashMap<u32, oneshot::Sender<Result<Value>>>>>;

/// RAII guard ensuring callback cleanup when a request future is dropped.
struct CancelGuard {
    id: u32,
    callbacks: CallbackMap,
    completed: bool,
}

impl CancelGuard {
    fn new(id: u32, callbacks: CallbackMap) -> Self {
        Self {
            id,
            callbacks,
            completed: false,
        }
    }

    fn complete(&mut self) {
        self.completed = true;
    }
}

impl Drop for CancelGuard {
    fn drop(&mut self) {
        if self.completed {
            return;
        }

        let id = self.id;
        let callbacks = Arc::clone(&self.callbacks);

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                if callbacks.lock().await.remove(&id).is_some() {
                    tracing::debug!(id, "CancelGuard: removed orphaned callback");
                }
            });
        }
    }
}

/// Future returned by [`Connection::send_message`] with automatic cancellation cleanup.
struct ResponseFuture {
    rx: oneshot::Receiver<Result<Value>>,
    guard: CancelGuard,
}

impl Future for ResponseFuture {
    type Output = Result<Value>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.rx).poll(cx) {
            Poll::Ready(result) => {
                self.guard.complete();
                Poll::Ready(result.map_err(|_| Error::ChannelClosed).and_then(|r| r))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// JSON-RPC connection to Playwright server
///
/// Manages request/response correlation and event dispatch.
/// Uses sequential request IDs and oneshot channels for correlation.
pub struct Connection {
    /// Sequential request ID counter (atomic for thread safety)
    last_id: AtomicU32,
    /// Pending request callbacks keyed by request ID
    callbacks: Arc<TokioMutex<HashMap<u32, oneshot::Sender<Result<Value>>>>>,
    /// Channel for sending outbound messages to the writer task
    outbound_tx: mpsc::UnboundedSender<Value>,
    /// Transport sender (taken by run() to start writer task)
    transport_sender: Arc<TokioMutex<Option<Box<dyn Transport>>>>,
    /// Receiver for incoming messages from transport
    message_rx: Arc<TokioMutex<Option<mpsc::UnboundedReceiver<Value>>>>,
    /// Receiver half of transport (owned by run loop, only needed once)
    transport_receiver: Arc<TokioMutex<Option<Box<dyn TransportReceiver>>>>,
    /// Receiver for outbound messages (taken by run() to start writer task)
    outbound_rx: Arc<TokioMutex<Option<mpsc::UnboundedReceiver<Value>>>>,
    /// Registry of all protocol objects by GUID (parking_lot for sync+async access)
    objects: Arc<ParkingLotMutex<ObjectRegistry>>,
    /// Notification broadcast when any object is registered
    object_registered: Arc<Notify>,
    /// Factory for creating protocol objects (set via set_factory before run())
    factory: Arc<TokioMutex<Option<Arc<dyn ObjectFactory>>>>,
}

impl Connection {
    /// Create a new Connection with the given transport
    pub fn new(parts: TransportParts) -> Self {
        let TransportParts {
            sender,
            receiver,
            message_rx,
        } = parts;

        let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();

        Self {
            last_id: AtomicU32::new(0),
            callbacks: Arc::new(TokioMutex::new(HashMap::new())),
            outbound_tx,
            transport_sender: Arc::new(TokioMutex::new(Some(sender))),
            message_rx: Arc::new(TokioMutex::new(Some(message_rx))),
            transport_receiver: Arc::new(TokioMutex::new(Some(receiver))),
            outbound_rx: Arc::new(TokioMutex::new(Some(outbound_rx))),
            objects: Arc::new(ParkingLotMutex::new(HashMap::new())),
            object_registered: Arc::new(Notify::new()),
            factory: Arc::new(TokioMutex::new(None)),
        }
    }

    /// Set the object factory for creating protocol objects.
    ///
    /// This must be called before `run()` for `__create__` messages to work.
    pub async fn set_factory(&self, factory: Arc<dyn ObjectFactory>) {
        *self.factory.lock().await = Some(factory);
    }

    /// Sends a message to the Playwright server and awaits the response.
    pub async fn send_message(&self, guid: &str, method: &str, params: Value) -> Result<Value> {
        let id = self.last_id.fetch_add(1, Ordering::SeqCst);

        tracing::debug!(
            "Sending message: id={}, guid='{}', method='{}'",
            id,
            guid,
            method
        );

        let (tx, rx) = oneshot::channel();
        self.callbacks.lock().await.insert(id, tx);

        let guard = CancelGuard::new(id, Arc::clone(&self.callbacks));

        let request = Request {
            id,
            guid: Arc::from(guid),
            method: method.to_string(),
            params,
            metadata: Metadata::now(),
        };

        let request_value = serde_json::to_value(&request)?;
        tracing::debug!("Request JSON: {}", request_value);

        if self.outbound_tx.send(request_value).is_err() {
            tracing::error!("Failed to queue message: outbound channel closed");
            return Err(Error::ChannelClosed);
        }

        tracing::debug!("Awaiting response for ID {}", id);

        ResponseFuture { rx, guard }.await
    }

    /// Run the message dispatch loop
    pub async fn run(self: &Arc<Self>) {
        let transport_receiver = self
            .transport_receiver
            .lock()
            .await
            .take()
            .expect("run() can only be called once - transport receiver already taken");

        let mut transport_sender = self
            .transport_sender
            .lock()
            .await
            .take()
            .expect("run() can only be called once - transport sender already taken");

        let mut outbound_rx = self
            .outbound_rx
            .lock()
            .await
            .take()
            .expect("run() can only be called once - outbound receiver already taken");

        let reader_handle = tokio::spawn(async move {
            if let Err(e) = transport_receiver.run().await {
                tracing::error!("Transport read error: {}", e);
            }
        });

        let writer_handle = tokio::spawn(async move {
            while let Some(message) = outbound_rx.recv().await {
                if let Err(e) = transport_sender.send(message).await {
                    tracing::error!("Transport write error: {}", e);
                    break;
                }
            }
        });

        let mut message_rx = self
            .message_rx
            .lock()
            .await
            .take()
            .expect("run() can only be called once - message receiver already taken");

        while let Some(message_value) = message_rx.recv().await {
            match serde_json::from_value::<Message>(message_value) {
                Ok(message) => {
                    if let Err(e) = self.dispatch_internal(message).await {
                        tracing::error!("Error dispatching message: {}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to parse message: {}", e);
                }
            }
        }

        let _ = reader_handle.await;
        let _ = writer_handle.await;
    }

    /// Dispatch an incoming message (test-only public version)
    #[cfg(test)]
    pub async fn dispatch(self: &Arc<Self>, message: Message) -> Result<()> {
        self.dispatch_internal(message).await
    }

    async fn dispatch_internal(self: &Arc<Self>, message: Message) -> Result<()> {
        tracing::debug!("Dispatching message: {:?}", message);
        match message {
            Message::Response(response) => {
                tracing::debug!("Processing response for ID: {}", response.id);
                let callback = self
                    .callbacks
                    .lock()
                    .await
                    .remove(&response.id)
                    .ok_or_else(|| {
                        Error::ProtocolError(format!(
                            "Cannot find request to respond: id={}",
                            response.id
                        ))
                    })?;

                let result = if let Some(error_wrapper) = response.error {
                    Err(parse_protocol_error(error_wrapper.error))
                } else {
                    Ok(response.result.unwrap_or(Value::Null))
                };

                let _ = callback.send(result);
                Ok(())
            }
            Message::Event(event) => match event.method.as_str() {
                "__create__" => self.handle_create(&event).await,
                "__dispose__" => self.handle_dispose(&event).await,
                "__adopt__" => self.handle_adopt(&event).await,
                _ => match self.objects.lock().get(&event.guid).cloned() {
                    Some(object) => {
                        object.on_event(&event.method, event.params);
                        Ok(())
                    }
                    None => {
                        tracing::debug!(
                            "Event for unknown object (ignored): guid={}, method={}",
                            event.guid,
                            event.method
                        );
                        Ok(())
                    }
                },
            },
            Message::Unknown(value) => {
                tracing::debug!(
                    "Unknown message type (forward-compatible, ignored): {}",
                    serde_json::to_string(&value)
                        .unwrap_or_else(|_| "<serialization failed>".to_string())
                );
                Ok(())
            }
        }
    }

    /// Handle `__create__` protocol message
    async fn handle_create(self: &Arc<Self>, event: &Event) -> Result<()> {
        let type_name = event.params["type"]
            .as_str()
            .ok_or_else(|| Error::ProtocolError("__create__ missing 'type'".to_string()))?
            .to_string();

        let object_guid: Arc<str> = Arc::from(
            event.params["guid"]
                .as_str()
                .ok_or_else(|| Error::ProtocolError("__create__ missing 'guid'".to_string()))?,
        );

        tracing::debug!(
            "__create__: type={}, guid={}, parent_guid={}",
            type_name,
            object_guid,
            event.guid
        );

        let initializer = event.params["initializer"].clone();

        // Get parent object
        let parent_obj = self
            .objects
            .lock()
            .get(&event.guid)
            .cloned()
            .ok_or_else(|| {
                tracing::debug!(
                    "Parent object not found for type={}, parent_guid={}",
                    type_name,
                    event.guid
                );
                Error::ProtocolError(format!("Parent object not found: {}", event.guid))
            })?;

        // Determine parent or connection
        let parent_or_conn = if type_name == "Playwright" && event.guid.is_empty() {
            ParentOrConnection::Connection(Arc::clone(self) as Arc<dyn ConnectionLike>)
        } else {
            ParentOrConnection::Parent(parent_obj.clone())
        };

        // Get factory and create object
        let factory = self.factory.lock().await;
        let factory = factory.as_ref().ok_or_else(|| {
            Error::ProtocolError(
                "ObjectFactory not set - call set_factory() before run()".to_string(),
            )
        })?;

        let object = match factory
            .create_object(
                parent_or_conn,
                type_name.clone(),
                object_guid.clone(),
                initializer,
            )
            .await
        {
            Ok(obj) => obj,
            Err(e) => {
                tracing::debug!(
                    "Failed to create object type={}, guid={}, error={}",
                    type_name,
                    object_guid,
                    e
                );
                return Err(e);
            }
        };

        // Register in connection
        self.register_object(Arc::clone(&object_guid), object.clone())
            .await;

        // Register in parent
        parent_obj.add_child(Arc::clone(&object_guid), object);

        tracing::debug!("Created object: type={}, guid={}", type_name, object_guid);

        Ok(())
    }

    /// Handle `__dispose__` protocol message
    async fn handle_dispose(&self, event: &Event) -> Result<()> {
        let reason = match event.params.get("reason").and_then(|r| r.as_str()) {
            Some("gc") => DisposeReason::GarbageCollected,
            _ => DisposeReason::Closed,
        };

        let object = self.objects.lock().get(&event.guid).cloned();

        if let Some(obj) = object {
            obj.dispose(reason);
            tracing::debug!("Disposed object: guid={}", event.guid);
        } else {
            tracing::debug!("Dispose for unknown object (ignored): guid={}", event.guid);
        }

        Ok(())
    }

    /// Handle `__adopt__` protocol message
    async fn handle_adopt(&self, event: &Event) -> Result<()> {
        let child_guid: Arc<str> = Arc::from(
            event.params["guid"]
                .as_str()
                .ok_or_else(|| Error::ProtocolError("__adopt__ missing 'guid'".to_string()))?,
        );

        let new_parent = self.objects.lock().get(&event.guid).cloned();
        let child = self.objects.lock().get(&child_guid).cloned();

        match (new_parent, child) {
            (Some(parent), Some(child_obj)) => {
                parent.adopt(child_obj);
                tracing::debug!(
                    "Adopted object: child={}, new_parent={}",
                    child_guid,
                    event.guid
                );
                Ok(())
            }
            (None, _) => Err(Error::ProtocolError(format!(
                "Parent object not found: {}",
                event.guid
            ))),
            (_, None) => Err(Error::ProtocolError(format!(
                "Child object not found: {}",
                child_guid
            ))),
        }
    }
}

/// Converts [`ErrorPayload`] from Playwright into [`Error::Remote`].
fn parse_protocol_error(error: ErrorPayload) -> Error {
    Error::Remote {
        name: error.name.unwrap_or_else(|| "Error".to_string()),
        message: error.message,
        stack: error.stack,
    }
}

// Implement ConnectionLike trait for Connection
impl ConnectionLike for Connection {
    fn send_message(
        &self,
        guid: &str,
        method: &str,
        params: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + '_>> {
        let guid = guid.to_string();
        let method = method.to_string();
        Box::pin(async move { Connection::send_message(self, &guid, &method, params).await })
    }

    fn register_object(
        &self,
        guid: Arc<str>,
        object: Arc<dyn ChannelOwner>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            self.objects.lock().insert(guid, object);
            self.object_registered.notify_waiters();
        })
    }

    fn unregister_object(&self, guid: &str) {
        let guid_arc: Arc<str> = Arc::from(guid);
        self.objects.lock().remove(&guid_arc);
    }

    fn get_object(&self, guid: &str) -> AsyncChannelOwnerResult<'_> {
        let guid_arc: Arc<str> = Arc::from(guid);
        Box::pin(async move {
            self.objects.lock().get(&guid_arc).cloned().ok_or_else(|| {
                let target_type = if guid_arc.starts_with("page@") {
                    "Page"
                } else if guid_arc.starts_with("frame@") {
                    "Frame"
                } else if guid_arc.starts_with("browser-context@") {
                    "BrowserContext"
                } else if guid_arc.starts_with("browser@") {
                    "Browser"
                } else {
                    return Error::ProtocolError(format!("Object not found: {}", guid_arc));
                };

                Error::TargetClosed {
                    target_type: target_type.to_string(),
                    context: format!("Object not found: {}", guid_arc),
                }
            })
        })
    }

    fn wait_for_object(
        &self,
        guid: &str,
        timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = Result<Arc<dyn ChannelOwner>>> + Send + '_>> {
        let guid_arc: Arc<str> = Arc::from(guid);
        Box::pin(async move {
            let deadline = tokio::time::Instant::now() + timeout;

            loop {
                if let Some(obj) = self.objects.lock().get(&guid_arc).cloned() {
                    return Ok(obj);
                }

                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    let target_type = if guid_arc.starts_with("page@") {
                        "Page"
                    } else if guid_arc.starts_with("frame@") {
                        "Frame"
                    } else if guid_arc.starts_with("browser-context@") {
                        "BrowserContext"
                    } else if guid_arc.starts_with("browser@") {
                        "Browser"
                    } else if guid_arc.starts_with("response@") {
                        "Response"
                    } else {
                        return Err(Error::Timeout(format!(
                            "Timeout waiting for object: {}",
                            guid_arc
                        )));
                    };

                    return Err(Error::Timeout(format!(
                        "Timeout waiting for {} object: {}",
                        target_type, guid_arc
                    )));
                }

                tokio::select! {
                    _ = self.object_registered.notified() => {}
                    _ = tokio::time::sleep(remaining) => {}
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::PipeTransport;
    use tokio::io::duplex;

    fn create_test_connection() -> (Connection, tokio::io::DuplexStream, tokio::io::DuplexStream) {
        let (stdin_read, stdin_write) = duplex(1024);
        let (stdout_read, stdout_write) = duplex(1024);

        let (transport, message_rx) = PipeTransport::new(stdin_write, stdout_read);
        let parts = transport.into_transport_parts(message_rx);
        let connection = Connection::new(parts);

        (connection, stdin_read, stdout_write)
    }

    #[test]
    fn test_request_id_increments() {
        let (connection, _, _) = create_test_connection();

        let id1 = connection.last_id.fetch_add(1, Ordering::SeqCst);
        let id2 = connection.last_id.fetch_add(1, Ordering::SeqCst);
        let id3 = connection.last_id.fetch_add(1, Ordering::SeqCst);

        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert_eq!(id3, 2);
    }

    #[test]
    fn test_request_format() {
        let request = Request {
            id: 0,
            guid: Arc::from("page@abc123"),
            method: "goto".to_string(),
            params: serde_json::json!({"url": "https://example.com"}),
            metadata: Metadata::now(),
        };

        assert_eq!(request.id, 0);
        assert_eq!(request.guid.as_ref(), "page@abc123");
        assert_eq!(request.method, "goto");
        assert_eq!(request.params["url"], "https://example.com");
    }

    #[tokio::test]
    async fn test_dispatch_response_success() {
        let (connection, _, _) = create_test_connection();

        let id = connection.last_id.fetch_add(1, Ordering::SeqCst);

        let (tx, rx) = oneshot::channel();
        connection.callbacks.lock().await.insert(id, tx);

        let response = Message::Response(Response {
            id,
            result: Some(serde_json::json!({"status": "ok"})),
            error: None,
        });

        Arc::new(connection).dispatch(response).await.unwrap();

        let result = rx.await.unwrap().unwrap();
        assert_eq!(result["status"], "ok");
    }

    #[tokio::test]
    async fn test_dispatch_response_error() {
        let (connection, _, _) = create_test_connection();

        let id = connection.last_id.fetch_add(1, Ordering::SeqCst);

        let (tx, rx) = oneshot::channel();
        connection.callbacks.lock().await.insert(id, tx);

        let response = Message::Response(Response {
            id,
            result: None,
            error: Some(ErrorWrapper {
                error: ErrorPayload {
                    message: "Navigation timeout".to_string(),
                    name: Some("TimeoutError".to_string()),
                    stack: None,
                },
            }),
        });

        Arc::new(connection).dispatch(response).await.unwrap();

        let result = rx.await.unwrap();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_timeout(), "Expected timeout error, got: {:?}", err);
    }

    #[test]
    fn test_message_deserialization_response() {
        let json = r#"{"id": 42, "result": {"status": "ok"}}"#;
        let message: Message = serde_json::from_str(json).unwrap();

        match message {
            Message::Response(response) => {
                assert_eq!(response.id, 42);
                assert!(response.result.is_some());
                assert!(response.error.is_none());
            }
            _ => panic!("Expected Response"),
        }
    }

    #[test]
    fn test_message_deserialization_event() {
        let json = r#"{"guid": "page@abc", "method": "console", "params": {"text": "hello"}}"#;
        let message: Message = serde_json::from_str(json).unwrap();

        match message {
            Message::Event(event) => {
                assert_eq!(event.guid.as_ref(), "page@abc");
                assert_eq!(event.method, "console");
                assert_eq!(event.params["text"], "hello");
            }
            _ => panic!("Expected Event"),
        }
    }

    #[test]
    fn test_error_type_parsing() {
        let error = parse_protocol_error(ErrorPayload {
            message: "timeout".to_string(),
            name: Some("TimeoutError".to_string()),
            stack: Some("stack trace".to_string()),
        });
        assert!(error.is_timeout());
        match &error {
            Error::Remote {
                name,
                message,
                stack,
            } => {
                assert_eq!(name, "TimeoutError");
                assert_eq!(message, "timeout");
                assert_eq!(stack.as_deref(), Some("stack trace"));
            }
            _ => panic!("Expected Remote error"),
        }
    }
}
