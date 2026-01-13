//! Embedded markdown documentation for AI agents.

use crate::error::Result;

const DOC_PW: &str = include_str!("agents_pw.md");
const DOC_AUTH: &str = include_str!("agents_auth.md");
const DOC_CONNECT: &str = include_str!("agents_connect.md");
const DOC_DAEMON: &str = include_str!("agents_daemon.md");
const DOC_PAGE: &str = include_str!("agents_page.md");
const DOC_PROTECT: &str = include_str!("agents_protect.md");
const DOC_RUN: &str = include_str!("agents_run.md");

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
