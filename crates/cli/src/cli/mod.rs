#[cfg(test)]
mod tests;

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::output::OutputFormat;
use crate::styles::cli_styles;
use crate::types::BrowserKind;

/// HAR content policy (CLI wrapper for pw_rs::HarContentPolicy)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CliHarContentPolicy {
	/// Include content inline (base64)
	Embed,
	/// Store content in separate files
	#[default]
	Attach,
	/// Omit content entirely
	Omit,
}

impl From<CliHarContentPolicy> for pw_rs::HarContentPolicy {
	fn from(policy: CliHarContentPolicy) -> Self {
		match policy {
			CliHarContentPolicy::Embed => pw_rs::HarContentPolicy::Embed,
			CliHarContentPolicy::Attach => pw_rs::HarContentPolicy::Attach,
			CliHarContentPolicy::Omit => pw_rs::HarContentPolicy::Omit,
		}
	}
}

/// HAR recording mode (CLI wrapper for pw_rs::HarMode)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CliHarMode {
	/// Store all content
	#[default]
	Full,
	/// Store only essential content for replay
	Minimal,
}

impl From<CliHarMode> for pw_rs::HarMode {
	fn from(mode: CliHarMode) -> Self {
		match mode {
			CliHarMode::Full => pw_rs::HarMode::Full,
			CliHarMode::Minimal => pw_rs::HarMode::Minimal,
		}
	}
}

pub use crate::commands::graph::{AuthAction, Commands, DaemonAction, HarAction, PageAction, ProtectAction, SessionAction, TabsAction};
// Re-export OutputFormat for backwards compatibility
pub use crate::output::OutputFormat as CliOutputFormat;

#[derive(Parser, Debug)]
#[command(name = "pw")]
#[command(about = "Playwright CLI - Browser automation from the command line")]
#[command(version)]
#[command(styles = cli_styles())]
pub struct Cli {
	/// Increase verbosity (-v info, -vv debug)
	#[arg(short, long, global = true, action = clap::ArgAction::Count)]
	pub verbose: u8,

	/// Output format: toon (default), json, ndjson, or text
	#[arg(short = 'f', long, global = true, value_enum, default_value = "toon")]
	pub format: OutputFormat,

	/// Load authentication state from file (cookies, localStorage)
	#[arg(long, global = true, value_name = "FILE")]
	pub auth: Option<PathBuf>,

	/// Browser to use for automation
	#[arg(short, long, global = true, value_enum, default_value = "chromium")]
	pub browser: BrowserKind,

	/// Connect to an existing CDP endpoint instead of launching a browser
	#[arg(long, global = true, value_name = "URL")]
	pub cdp_endpoint: Option<String>,

	/// Launch a reusable local browser server and persist its endpoint
	#[arg(long, global = true)]
	pub launch_server: bool,

	/// Disable daemon usage for this invocation
	#[arg(long, global = true)]
	pub no_daemon: bool,

	/// Disable project detection (use current directory paths)
	#[arg(long, global = true)]
	pub no_project: bool,

	/// Workspace root path for state/session isolation (or "auto")
	#[arg(long, global = true, value_name = "PATH|auto")]
	pub workspace: Option<String>,

	/// Namespace inside the workspace for strict session isolation
	#[arg(long, global = true, value_name = "NAME", default_value = "default")]
	pub namespace: String,

	/// Disable contextual inference/caching for this invocation
	#[arg(long, global = true)]
	pub no_context: bool,

	/// Do not persist command results back to context store
	#[arg(long, global = true)]
	pub no_save_context: bool,

	/// Clear cached context data before running
	#[arg(long, global = true)]
	pub refresh_context: bool,

	/// Base URL used when URL argument is relative or omitted
	#[arg(long, global = true, value_name = "URL")]
	pub base_url: Option<String>,

	/// Directory to save artifacts (screenshot, HTML) on command failure
	#[arg(long, global = true, value_name = "DIR")]
	pub artifacts_dir: Option<std::path::PathBuf>,

	/// Block requests matching URL pattern (glob, can be used multiple times)
	#[arg(long, global = true, value_name = "PATTERN", action = clap::ArgAction::Append)]
	pub block: Vec<String>,

	/// Load request blocking patterns from file (one pattern per line)
	#[arg(long, global = true, value_name = "FILE")]
	pub block_file: Option<PathBuf>,

	/// Directory to save downloaded files (enables download tracking)
	#[arg(long, global = true, value_name = "DIR")]
	pub downloads_dir: Option<PathBuf>,

	/// Timeout for navigation and wait operations in milliseconds
	#[arg(long, global = true, value_name = "MS")]
	pub timeout: Option<u64>,

	#[command(subcommand)]
	pub command: Commands,
}

/// Project template type for init command
#[derive(Clone, Debug, ValueEnum, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InitTemplate {
	/// Full structure: tests/, scripts/, results/, reports/, screenshots/
	#[default]
	Standard,
	/// Minimal structure: tests/ only
	Minimal,
}

/// Output format for the read command
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReadOutputFormat {
	/// Plain text
	Text,
	/// Cleaned HTML
	Html,
	/// Markdown (default)
	#[default]
	Markdown,
}
