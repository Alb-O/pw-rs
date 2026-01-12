// Copyright 2024 Paul Adamson
// Licensed under the Apache License, Version 2.0
//
// Object Factory - Creates protocol objects from type names
//
// Architecture Reference:
// - Python: playwright-python/playwright/_impl/_connection.py (_create_remote_object)
// - Java: playwright-java/.../impl/Connection.java (createRemoteObject)
// - JavaScript: playwright/.../client/connection.ts (_createRemoteObject)
//
// The object factory maps protocol type names (strings) to Rust constructors.
// When the server sends a `__create__` message, the factory instantiates
// the appropriate Rust object based on the type name.

use crate::{
    Browser, BrowserContext, BrowserType, Dialog, Frame, Page, Playwright, Request, ResponseObject,
    Route, Tracing, Video, artifact::Artifact,
};
use pw_runtime::channel_owner::{ChannelOwner, ChannelOwnerImpl, ParentOrConnection};
use pw_runtime::{Error, Result};
use serde_json::Value;
use std::sync::Arc;

/// Creates a protocol object from a `__create__` message.
///
/// This function is the central object factory for the Playwright protocol.
/// It maps type names from the server to Rust struct constructors.
///
/// # Arguments
///
/// * `parent` - Either a parent ChannelOwner or the root Connection
/// * `type_name` - Protocol type name (e.g., "Playwright", "BrowserType")
/// * `guid` - Unique GUID assigned by the server
/// * `initializer` - JSON object with initial state
///
/// # Returns
///
/// An `Arc<dyn ChannelOwner>` pointing to the newly created object.
///
/// # Errors
///
/// Returns `Error::ProtocolError` if the type name is unknown or if
/// object construction fails.
///
/// # Example
///
/// ```ignore
/// # use pw::server::object_factory::create_object;
/// # use pw::server::channel_owner::ParentOrConnection;
/// # use pw::server::connection::ConnectionLike;
/// # use std::sync::Arc;
/// # use serde_json::json;
/// # async fn example(connection: Arc<dyn ConnectionLike>) -> Result<(), Box<dyn std::error::Error>> {
/// let playwright_obj = create_object(
///     ParentOrConnection::Connection(connection),
///     "Playwright".to_string(),
///     Arc::from("playwright@1"),
///     json!({
///         "chromium": { "guid": "browserType@chromium" },
///         "firefox": { "guid": "browserType@firefox" },
///         "webkit": { "guid": "browserType@webkit" }
///     })
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn create_object(
    parent: ParentOrConnection,
    type_name: String,
    guid: Arc<str>,
    initializer: Value,
) -> Result<Arc<dyn ChannelOwner>> {
    // Match on type name and call appropriate constructor
    let object: Arc<dyn ChannelOwner> = match type_name.as_str() {
        "Playwright" => {
            // Playwright is the root object, so parent must be Connection
            let connection = match parent {
                ParentOrConnection::Connection(conn) => conn,
                ParentOrConnection::Parent(_) => {
                    return Err(Error::ProtocolError(
                        "Playwright must have Connection as parent".to_string(),
                    ));
                }
            };

            Arc::new(Playwright::new(connection, type_name, guid, initializer).await?)
        }

        "BrowserType" => {
            // BrowserType has Playwright as parent
            let parent_owner = match parent {
                ParentOrConnection::Parent(p) => p,
                ParentOrConnection::Connection(_) => {
                    return Err(Error::ProtocolError(
                        "BrowserType must have Playwright as parent".to_string(),
                    ));
                }
            };

            Arc::new(BrowserType::new(
                parent_owner,
                type_name,
                guid,
                initializer,
            )?)
        }

        "Browser" => {
            // Browser has BrowserType as parent
            let parent_owner = match parent {
                ParentOrConnection::Parent(p) => p,
                ParentOrConnection::Connection(_) => {
                    return Err(Error::ProtocolError(
                        "Browser must have BrowserType as parent".to_string(),
                    ));
                }
            };

            Arc::new(Browser::new(parent_owner, type_name, guid, initializer)?)
        }

        "BrowserContext" => {
            // BrowserContext has Browser as parent
            let parent_owner = match parent {
                ParentOrConnection::Parent(p) => p,
                ParentOrConnection::Connection(_) => {
                    return Err(Error::ProtocolError(
                        "BrowserContext must have Browser as parent".to_string(),
                    ));
                }
            };

            Arc::new(BrowserContext::new(
                parent_owner,
                type_name,
                guid,
                initializer,
            )?)
        }

        "Page" => {
            // Page has BrowserContext as parent
            let parent_owner = match parent {
                ParentOrConnection::Parent(p) => p,
                ParentOrConnection::Connection(_) => {
                    return Err(Error::ProtocolError(
                        "Page must have BrowserContext as parent".to_string(),
                    ));
                }
            };

            Arc::new(Page::new(parent_owner, type_name, guid, initializer)?)
        }

        "Frame" => {
            // Frame has Page as parent
            let parent_owner = match parent {
                ParentOrConnection::Parent(p) => p,
                ParentOrConnection::Connection(_) => {
                    return Err(Error::ProtocolError(
                        "Frame must have Page as parent".to_string(),
                    ));
                }
            };

            Arc::new(Frame::new(parent_owner, type_name, guid, initializer)?)
        }

        "Request" => {
            // Request has Frame as parent
            let parent_owner = match parent {
                ParentOrConnection::Parent(p) => p,
                ParentOrConnection::Connection(_) => {
                    return Err(Error::ProtocolError(
                        "Request must have Frame as parent".to_string(),
                    ));
                }
            };

            Arc::new(Request::new(parent_owner, type_name, guid, initializer)?)
        }

        "Route" => {
            // Route has Frame as parent (created during network interception)
            let parent_owner = match parent {
                ParentOrConnection::Parent(p) => p,
                ParentOrConnection::Connection(_) => {
                    return Err(Error::ProtocolError(
                        "Route must have Frame as parent".to_string(),
                    ));
                }
            };

            Arc::new(Route::new(parent_owner, type_name, guid, initializer)?)
        }

        "Response" => {
            // Response has Request as parent (not Frame!)
            let parent_owner = match parent {
                ParentOrConnection::Parent(p) => p,
                ParentOrConnection::Connection(_) => {
                    return Err(Error::ProtocolError(
                        "Response must have Request as parent".to_string(),
                    ));
                }
            };

            Arc::new(ResponseObject::new(
                parent_owner,
                type_name,
                guid,
                initializer,
            )?)
        }

        "ElementHandle" => {
            // ElementHandle has Frame as parent
            let parent_owner = match parent {
                ParentOrConnection::Parent(p) => p,
                ParentOrConnection::Connection(_) => {
                    return Err(Error::ProtocolError(
                        "ElementHandle must have Frame as parent".to_string(),
                    ));
                }
            };

            Arc::new(crate::ElementHandle::new(
                parent_owner,
                type_name,
                guid,
                initializer,
            )?)
        }

        "Artifact" => {
            // Artifact has BrowserContext as parent
            let parent_owner = match parent {
                ParentOrConnection::Parent(p) => p,
                ParentOrConnection::Connection(_) => {
                    return Err(Error::ProtocolError(
                        "Artifact must have BrowserContext as parent".to_string(),
                    ));
                }
            };

            Arc::new(Artifact::new(parent_owner, type_name, guid, initializer)?)
        }

        "Dialog" => {
            // Dialog has Page as parent
            let parent_owner = match parent {
                ParentOrConnection::Parent(p) => p,
                ParentOrConnection::Connection(_) => {
                    return Err(Error::ProtocolError(
                        "Dialog must have Page as parent".to_string(),
                    ));
                }
            };

            Arc::new(Dialog::new(parent_owner, type_name, guid, initializer)?)
        }

        "Tracing" => {
            // Tracing has BrowserContext as parent
            let parent_owner = match parent {
                ParentOrConnection::Parent(p) => p,
                ParentOrConnection::Connection(_) => {
                    return Err(Error::ProtocolError(
                        "Tracing must have BrowserContext as parent".to_string(),
                    ));
                }
            };

            Arc::new(Tracing::new(parent_owner, type_name, guid, initializer)?)
        }

        "Video" => {
            // Video has Page as parent (created when video recording is enabled)
            let parent_owner = match parent {
                ParentOrConnection::Parent(p) => p,
                ParentOrConnection::Connection(_) => {
                    return Err(Error::ProtocolError(
                        "Video must have Page as parent".to_string(),
                    ));
                }
            };

            Arc::new(Video::new(parent_owner, type_name, guid, initializer)?)
        }

        _ => {
            // Unknown type - log at debug level and return inert object to stay forward-compatible
            tracing::debug!("Unknown protocol type (forward-compatible): {}", type_name);
            Arc::new(UnknownObject::new(parent, type_name, guid, initializer))
        }
    };

    Ok(object)
}

struct UnknownObject {
    base: ChannelOwnerImpl,
}

impl UnknownObject {
    fn new(
        parent: ParentOrConnection,
        type_name: String,
        guid: Arc<str>,
        initializer: Value,
    ) -> Self {
        let base = ChannelOwnerImpl::new(parent, type_name, guid, initializer);
        Self { base }
    }
}

impl pw_runtime::channel_owner::private::Sealed for UnknownObject {}

impl ChannelOwner for UnknownObject {
    fn guid(&self) -> &str {
        self.base.guid()
    }

    fn type_name(&self) -> &str {
        self.base.type_name()
    }

    fn parent(&self) -> Option<Arc<dyn ChannelOwner>> {
        self.base.parent()
    }

    fn connection(&self) -> Arc<dyn pw_runtime::connection::ConnectionLike> {
        self.base.connection()
    }

    fn initializer(&self) -> &Value {
        self.base.initializer()
    }

    fn channel(&self) -> &pw_runtime::channel::Channel {
        self.base.channel()
    }

    fn dispose(&self, reason: pw_runtime::channel_owner::DisposeReason) {
        self.base.dispose(reason)
    }

    fn adopt(&self, child: Arc<dyn ChannelOwner>) {
        self.base.adopt(child)
    }

    fn add_child(&self, guid: Arc<str>, child: Arc<dyn ChannelOwner>) {
        self.base.add_child(guid, child)
    }

    fn remove_child(&self, guid: &str) {
        self.base.remove_child(guid)
    }

    fn on_event(&self, _method: &str, _params: Value) {}

    fn was_collected(&self) -> bool {
        self.base.was_collected()
    }
}

// Note: Object factory testing is done via integration tests since it requires:
// - Real Connection with object registry
// - Protocol messages from the server
// See: crates/playwright-core/tests/connection_integration.rs
