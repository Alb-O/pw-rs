//! Session lifecycle and browser-connection subsystem.
//!
//! This module centralizes session descriptor persistence, acquisition
//! strategy decisions, and shared connect/discover orchestration.

/// Browser connect/discover helpers shared across commands.
pub mod connector;
/// Persisted session descriptor schema and helpers.
pub mod descriptor;
/// Session request/manager/handle types and orchestration.
pub mod manager;
/// Pure strategy selection for session acquisition.
pub mod strategy;

/// Persisted session descriptor metadata.
pub use descriptor::SessionDescriptor;
/// Session handle, manager, and request types.
pub use manager::{SessionHandle, SessionManager, SessionRequest};
