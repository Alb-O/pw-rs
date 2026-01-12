//! Wire types for the Playwright protocol.
//!
//! This crate contains the serde-serializable types used for communication
//! with the Playwright server over JSON-RPC. These types represent the
//! "protocol layer" - the shapes of data as they appear on the wire.
//!
//! # Design Philosophy
//!
//! Types in this crate are:
//! - **Pure data**: No behavior beyond serialization/deserialization
//! - **1:1 with protocol**: Match Playwright's protocol.yml schema
//! - **Stable**: Changes only when the wire protocol changes
//!
//! Higher-level ergonomic APIs are built on top of these types in `pw-api`.

pub mod auth_exchange;
pub mod cookie;
pub mod options;
pub mod types;

pub use auth_exchange::*;
pub use cookie::*;
pub use options::*;
pub use types::*;
