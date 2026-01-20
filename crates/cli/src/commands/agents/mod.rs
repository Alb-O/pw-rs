//! Embedded markdown documentation for AI agents.

use crate::error::Result;

const DOC_PW: &str = include_str!("AGENTS.md");
const DOC_AUTH: &str = include_str!("../auth/AGENTS.md");
const DOC_CONNECT: &str = include_str!("../connect/AGENTS.md");
const DOC_DAEMON: &str = include_str!("../daemon/AGENTS.md");
const DOC_PAGE: &str = include_str!("../page/AGENTS.md");
const DOC_PROTECT: &str = include_str!("../protect/AGENTS.md");
const DOC_RUN: &str = include_str!("../run/AGENTS.md");
const DOC_TEST: &str = include_str!("../test/AGENTS.md");

/// Prints the main pw documentation for agents.
pub fn show_main() -> Result<()> {
    println!("{DOC_PW}");
    Ok(())
}

/// Prints documentation for the `auth` subcommand.
pub fn show_auth() -> Result<()> {
    println!("{DOC_AUTH}");
    Ok(())
}

/// Prints documentation for the `connect` subcommand.
pub fn show_connect() -> Result<()> {
    println!("{DOC_CONNECT}");
    Ok(())
}

/// Prints documentation for the `daemon` subcommand.
pub fn show_daemon() -> Result<()> {
    println!("{DOC_DAEMON}");
    Ok(())
}

/// Prints documentation for the `page` subcommand.
pub fn show_page() -> Result<()> {
    println!("{DOC_PAGE}");
    Ok(())
}

/// Prints documentation for the `protect` subcommand.
pub fn show_protect() -> Result<()> {
    println!("{DOC_PROTECT}");
    Ok(())
}

/// Prints documentation for the `run` subcommand.
pub fn show_run() -> Result<()> {
    println!("{DOC_RUN}");
    Ok(())
}

/// Prints documentation for the `test` subcommand.
pub fn show_test() -> Result<()> {
    println!("{DOC_TEST}");
    Ok(())
}
