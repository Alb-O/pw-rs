//! Playwright initialization helpers
//!
//! This module provides the `initialize_playwright` function that performs
//! the Playwright protocol handshake with the server.

use crate::{Playwright, Root};
use pw_runtime::channel_owner::{ChannelOwner, ParentOrConnection};
use pw_runtime::connection::{Connection, ConnectionLike, ObjectFactory};
use pw_runtime::{Error, Result};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

/// Initialize the Playwright connection and return the root Playwright object.
///
/// This function performs the initialization handshake with the Playwright server:
/// 1. Sets the object factory for creating protocol objects
/// 2. Creates a temporary Root object
/// 3. Sends "initialize" message with sdkLanguage="rust"
/// 4. Server creates BrowserType objects (sends `__create__` messages)
/// 5. Server responds with Playwright GUID
/// 6. Looks up Playwright object from registry (guaranteed to exist)
///
/// # Returns
///
/// An `Arc<dyn ChannelOwner>` that is the Playwright object.
///
/// # Errors
///
/// Returns error if:
/// - Initialize message fails to send
/// - Server returns protocol error
/// - Response is missing Playwright GUID
/// - Playwright object not found in registry
/// - Timeout after 30 seconds
pub async fn initialize_playwright(connection: &Arc<Connection>) -> Result<Arc<dyn ChannelOwner>> {
    // Set the object factory before running
    connection.set_factory(Arc::new(DefaultObjectFactory)).await;

    // Create temporary Root object for initialization
    let root = Arc::new(Root::new(
        Arc::clone(connection) as Arc<dyn pw_runtime::ConnectionLike>
    )) as Arc<dyn ChannelOwner>;

    // Register Root in objects map with empty GUID
    connection
        .register_object(Arc::from(""), root.clone())
        .await;

    tracing::debug!("Root object registered, sending initialize message");

    let root_typed = root
        .downcast_ref::<Root>()
        .expect("Root object should be Root type");

    // Send initialize with timeout
    let response = tokio::time::timeout(Duration::from_secs(30), root_typed.initialize())
        .await
        .map_err(|_| {
            Error::Timeout("Playwright initialization timeout after 30 seconds".to_string())
        })??;

    // Extract Playwright GUID from response
    let playwright_guid = response["playwright"]["guid"].as_str().ok_or_else(|| {
        Error::ProtocolError("Initialize response missing 'playwright.guid' field".to_string())
    })?;

    tracing::debug!("Initialized Playwright with GUID: {}", playwright_guid);

    // Get Playwright object from registry
    let playwright_obj = connection.get_object(playwright_guid).await?;

    // Verify it's actually a Playwright object
    playwright_obj.downcast_ref::<Playwright>().ok_or_else(|| {
        Error::ProtocolError(format!(
            "Object with GUID '{}' is not a Playwright instance",
            playwright_guid
        ))
    })?;

    // Cleanup: Unregister Root after initialization
    connection.unregister_object("");
    tracing::debug!("Root object unregistered after successful initialization");

    Ok(playwright_obj)
}

/// Default object factory that creates protocol objects.
struct DefaultObjectFactory;

impl ObjectFactory for DefaultObjectFactory {
    fn create_object(
        &self,
        parent: ParentOrConnection,
        type_name: String,
        guid: Arc<str>,
        initializer: Value,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Arc<dyn ChannelOwner>>> + Send + '_>,
    > {
        Box::pin(async move {
            crate::object_factory::create_object(parent, type_name, guid, initializer).await
        })
    }
}
