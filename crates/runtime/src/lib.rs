//! Playwright Runtime - Driver lifecycle, connection, and registry
//!
//! This crate provides the low-level runtime infrastructure for communicating
//! with the Playwright Node.js server:
//!
//! - **Driver management**: Locating and launching the Playwright driver
//! - **Transport**: Bidirectional communication over stdio pipes or WebSocket
//! - **Connection**: JSON-RPC request/response correlation and event dispatch
//! - **Object registry**: Managing protocol objects by GUID
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐
//! │   pw-api    │  Protocol objects (Browser, Page, etc.)
//! └──────┬──────┘
//!        │ implements ObjectFactory
//! ┌──────▼──────┐
//! │  pw-runtime │  This crate
//! │  ┌────────┐ │
//! │  │ Conn   │ │  JSON-RPC correlation
//! │  └────────┘ │
//! │  ┌────────┐ │
//! │  │ Trans  │ │  Pipe/WebSocket transport
//! │  └────────┘ │
//! │  ┌────────┐ │
//! │  │ Driver │ │  Process management
//! │  └────────┘ │
//! └─────────────┘
//! ```
//!
//! # Decoupling via ObjectFactory
//!
//! The `Connection` uses an `ObjectFactory` trait to create protocol objects
//! without depending on their concrete types. This allows pw-runtime to be
//! independent of pw-api, breaking the circular dependency.

pub mod channel;
pub mod channel_owner;
pub mod connection;
pub mod driver;
pub mod error;
pub mod playwright_server;
pub mod transport;

// Re-export key types at crate root
pub use channel::Channel;
pub use channel_owner::{ChannelOwner, ChannelOwnerImpl, DisposeReason, ParentOrConnection};
pub use connection::{
    AsyncChannelOwnerResult, Connection, ConnectionLike, Event, Message, Metadata, ObjectFactory,
    Request, Response,
};
pub use driver::get_driver_executable;
pub use error::{Error, Result};
pub use playwright_server::PlaywrightServer;
pub use transport::{
    PipeTransport, PipeTransportReceiver, PipeTransportSender, Transport, TransportParts,
    TransportReceiver, WebSocketTransport, WebSocketTransportReceiver, WebSocketTransportSender,
};
