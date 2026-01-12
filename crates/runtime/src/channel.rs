//! Channel - RPC communication proxy for ChannelOwner objects.
//!
//! The Channel provides a typed interface for sending JSON-RPC messages
//! to the Playwright server on behalf of a ChannelOwner object.

use crate::connection::ConnectionLike;
use crate::error::Result;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;

/// Channel provides RPC communication for a ChannelOwner.
///
/// Every ChannelOwner has a Channel that sends method calls to the
/// Playwright server and receives responses.
#[derive(Clone)]
pub struct Channel {
    guid: Arc<str>,
    connection: Arc<dyn ConnectionLike>,
}

impl Channel {
    /// Creates a new Channel for the given object GUID.
    pub fn new(guid: Arc<str>, connection: Arc<dyn ConnectionLike>) -> Self {
        Self { guid, connection }
    }

    /// Sends a method call to the Playwright server and awaits the response.
    pub async fn send<P: Serialize, R: DeserializeOwned>(
        &self,
        method: &str,
        params: P,
    ) -> Result<R> {
        let params_value = serde_json::to_value(params)?;
        let response = self
            .connection
            .send_message(&self.guid, method, params_value)
            .await?;
        serde_json::from_value(response).map_err(Into::into)
    }

    /// Sends a method call with no parameters.
    pub async fn send_no_params<R: DeserializeOwned>(&self, method: &str) -> Result<R> {
        self.send(method, Value::Null).await
    }

    /// Sends a method call that returns no result (void).
    pub async fn send_no_result<P: Serialize>(&self, method: &str, params: P) -> Result<()> {
        let _: Value = self.send(method, params).await?;
        Ok(())
    }

    /// Returns the GUID this channel represents.
    pub fn guid(&self) -> &str {
        &self.guid
    }
}
