#[cfg(test)]
mod tests;

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::output::OutputFormat;
use crate::styles::cli_styles;
use crate::types::BrowserKind;

/// Root CLI for pw v2.
#[derive(Parser, Debug)]
#[command(name = "pw")]
#[command(about = "Playwright CLI - protocol-first browser automation")]
#[command(version)]
#[command(styles = cli_styles())]
pub struct Cli {
	/// Increase verbosity (-v info, -vv debug)
	#[arg(short, long, global = true, action = clap::ArgAction::Count)]
	pub verbose: u8,

	/// Output format: toon (default), json, ndjson, or text
	#[arg(short = 'f', long, global = true, value_enum, default_value = "toon")]
	pub format: OutputFormat,

	#[command(subcommand)]
	pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
	/// Execute a single canonical operation.
	Exec(ExecArgs),
	/// Stream request envelopes over stdin/stdout (NDJSON).
	Batch(BatchArgs),
	/// Manage profile-scoped runtime configuration.
	Profile(ProfileArgs),
	/// Manage daemon lifecycle.
	Daemon(DaemonArgs),
}

#[derive(Args, Debug, Clone)]
pub struct ExecArgs {
	/// Canonical operation id (for example: page.text, navigate, click).
	#[arg(value_name = "OP")]
	pub op: Option<String>,

	/// JSON object for operation input.
	#[arg(long, value_name = "JSON", conflicts_with = "file")]
	pub input: Option<String>,

	/// Path to a full request envelope JSON file.
	#[arg(long, value_name = "FILE", conflicts_with = "input")]
	pub file: Option<PathBuf>,

	/// Runtime profile name.
	#[arg(long, value_name = "NAME", default_value = "default")]
	pub profile: String,

	/// Directory for failure artifacts.
	#[arg(long, value_name = "DIR")]
	pub artifacts_dir: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct BatchArgs {
	/// Runtime profile name.
	#[arg(long, value_name = "NAME", default_value = "default")]
	pub profile: String,
}

#[derive(Args, Debug, Clone)]
pub struct ProfileArgs {
	#[command(subcommand)]
	pub action: ProfileAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ProfileAction {
	/// List available profiles.
	List,
	/// Show profile configuration.
	Show {
		#[arg(value_name = "NAME")]
		name: String,
	},
	/// Replace profile configuration from JSON file.
	Set {
		#[arg(value_name = "NAME")]
		name: String,
		#[arg(long, value_name = "FILE")]
		file: PathBuf,
	},
	/// Delete a profile and its persisted state.
	Delete {
		#[arg(value_name = "NAME")]
		name: String,
	},
}

#[derive(Args, Debug, Clone)]
pub struct DaemonArgs {
	#[command(subcommand)]
	pub action: DaemonAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum DaemonAction {
	Start {
		#[arg(long)]
		foreground: bool,
	},
	Stop,
	Status,
}

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

/// Project template type for init command.
#[derive(Clone, Debug, ValueEnum, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InitTemplate {
	/// Full structure: tests/, scripts/, results/, reports/, screenshots/
	#[default]
	Standard,
	/// Minimal structure: tests/ only
	Minimal,
}

/// Output format for the read command.
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

/// Browser parsing helper retained for serde compatibility in command raw payloads.
#[allow(dead_code)]
fn _browser_kind_marker(_: BrowserKind) {}
