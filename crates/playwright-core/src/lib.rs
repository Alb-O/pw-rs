// playwright-core: Internal implementation of Playwright protocol for Rust
//
// This crate is not part of the public API and should only be used by the
// `playwright` crate.

pub mod driver;
pub mod error;
pub mod server;

pub use error::{Error, Result};
pub use server::PlaywrightServer;
