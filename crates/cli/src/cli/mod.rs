#[cfg(test)]
mod tests;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::output::OutputFormat;
use crate::styles::cli_styles;
use crate::types::BrowserKind;

/// HAR content policy (CLI wrapper for pw::HarContentPolicy)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum CliHarContentPolicy {
	/// Include content inline (base64)
	Embed,
	/// Store content in separate files
	#[default]
	Attach,
	/// Omit content entirely
	Omit,
}

impl From<CliHarContentPolicy> for pw::HarContentPolicy {
	fn from(policy: CliHarContentPolicy) -> Self {
		match policy {
			CliHarContentPolicy::Embed => pw::HarContentPolicy::Embed,
			CliHarContentPolicy::Attach => pw::HarContentPolicy::Attach,
			CliHarContentPolicy::Omit => pw::HarContentPolicy::Omit,
		}
	}
}

/// HAR recording mode (CLI wrapper for pw::HarMode)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum CliHarMode {
	/// Store all content
	#[default]
	Full,
	/// Store only essential content for replay
	Minimal,
}

impl From<CliHarMode> for pw::HarMode {
	fn from(mode: CliHarMode) -> Self {
		match mode {
			CliHarMode::Full => pw::HarMode::Full,
			CliHarMode::Minimal => pw::HarMode::Minimal,
		}
	}
}

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

	/// Named context to load for this run
	#[arg(long, global = true, value_name = "NAME")]
	pub context: Option<String>,

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

	/// Record network activity to HAR (HTTP Archive) file
	#[arg(long, global = true, value_name = "FILE")]
	pub har: Option<PathBuf>,

	/// HAR content policy: embed (inline base64), attach (separate files), or omit
	#[arg(long, global = true, value_enum, default_value = "attach")]
	pub har_content: CliHarContentPolicy,

	/// HAR recording mode: full (all content) or minimal (essential for replay)
	#[arg(long, global = true, value_enum, default_value = "full")]
	pub har_mode: CliHarMode,

	/// Omit request/response content from HAR recording
	#[arg(long, global = true)]
	pub har_omit_content: bool,

	/// URL pattern filter for HAR recording (glob pattern)
	#[arg(long, global = true, value_name = "PATTERN")]
	pub har_url_filter: Option<String>,

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

#[derive(Subcommand, Debug)]
pub enum Commands {
	/// Navigate to URL and show page snapshot (text, elements, metadata)
	#[command(alias = "nav")]
	Navigate(#[command(flatten)] crate::commands::navigate::NavigateRaw),

	/// Take screenshot
	#[command(alias = "ss")]
	Screenshot(#[command(flatten)] crate::commands::screenshot::ScreenshotRaw),

	/// Click element and show resulting URL
	Click(#[command(flatten)] crate::commands::click::ClickRaw),

	/// Fill text into an input field (works with React and other frameworks)
	Fill(#[command(flatten)] crate::commands::fill::FillRaw),

	/// Wait for condition (selector, timeout, or load state)
	Wait(#[command(flatten)] crate::commands::wait::WaitRaw),

	/// Page content extraction commands (text, html, snapshot, elements, etc.)
	#[command(subcommand)]
	Page(PageAction),

	/// Authentication and session management
	Auth {
		#[command(subcommand)]
		action: AuthAction,
	},

	/// Session lifecycle and inspection
	Session {
		#[command(subcommand)]
		action: SessionAction,
	},

	/// Manage the pw daemon for persistent browser sessions
	Daemon {
		#[command(subcommand)]
		action: DaemonAction,
	},

	/// Initialize a new playwright project structure
	Init {
		/// Project directory (defaults to current directory)
		#[arg(default_value = ".")]
		path: PathBuf,

		/// Template type: standard (full structure) or minimal (tests only)
		#[arg(long, short, default_value = "standard", value_enum)]
		template: InitTemplate,

		/// Skip creating playwright.config.js
		#[arg(long)]
		no_config: bool,

		/// Skip creating example test file
		#[arg(long)]
		no_example: bool,

		/// Use TypeScript for config and tests
		#[arg(long)]
		typescript: bool,

		/// Force overwrite existing files
		#[arg(long, short)]
		force: bool,

		/// Generate Nix browser setup script (for NixOS/Nix users)
		#[arg(long)]
		nix: bool,
	},

	/// Run the CDP relay server for the browser extension bridge
	Relay {
		/// Host to bind
		#[arg(long, default_value = "127.0.0.1")]
		host: String,
		/// Port to bind
		#[arg(long, default_value_t = 19988)]
		port: u16,
	},

	/// Connect to or launch a browser with remote debugging
	///
	/// Once connected, all pw commands will use this browser instead of launching a new one.
	/// This is the recommended way to bypass bot detection - use your real browser.
	///
	/// Examples:
	///   pw connect --launch          # Launch Chrome with debugging enabled
	///   pw connect --discover        # Find existing Chrome with debugging
	///   pw connect ws://...          # Set endpoint manually
	///   pw connect --clear           # Disconnect
	Connect {
		/// CDP WebSocket endpoint URL (e.g., ws://127.0.0.1:9222/devtools/browser/...)
		endpoint: Option<String>,
		/// Clear the saved CDP endpoint
		#[arg(long)]
		clear: bool,
		/// Launch Chrome with remote debugging enabled
		#[arg(long)]
		launch: bool,
		/// Discover and connect to existing Chrome with debugging enabled
		#[arg(long)]
		discover: bool,
		/// Kill Chrome process on the debugging port
		#[arg(long)]
		kill: bool,
		/// Remote debugging port (default: 9222)
		#[arg(long, short, default_value = "9222")]
		port: u16,
		/// Browser user data directory (for --launch)
		#[arg(long)]
		user_data_dir: Option<PathBuf>,
	},

	/// Manage browser tabs
	#[command(subcommand)]
	Tabs(TabsAction),

	/// Manage protected URL patterns (tabs the CLI won't touch)
	///
	/// Protected tabs are excluded from page selection and tab operations.
	/// Use this to prevent the CLI from accidentally navigating your PWAs or
	/// important tabs like Discord, Slack, etc.
	#[command(subcommand)]
	Protect(ProtectAction),

	/// Run commands from stdin in batch mode (for AI agents)
	///
	/// Reads NDJSON commands from stdin and streams responses to stdout.
	/// Each line should be a JSON object with "id", "command", and "args" fields.
	///
	/// Example input:
	///   {"id":"1","command":"navigate","args":{"url":"https://example.com"}}
	///   {"id":"2","command":"screenshot","args":{"output":"page.png"}}
	///
	/// Use Ctrl+D (EOF) to exit batch mode.
	Run,

	/// Run Playwright tests
	///
	/// Invokes the bundled Playwright test runner without requiring npm.
	/// All arguments after -- are passed directly to the Playwright test CLI.
	///
	/// Examples:
	///   pw test                          # Run all tests
	///   pw test -- --headed              # Show browser
	///   pw test -- --browser=firefox     # Use Firefox
	///   pw test -- -g "login"            # Filter by name
	///   pw test -- --debug               # Debug mode
	#[command(alias = "t")]
	Test {
		/// Arguments passed to playwright test CLI
		#[arg(trailing_var_arg = true, allow_hyphen_values = true)]
		args: Vec<String>,
	},
}

/// Project template type for init command
#[derive(Clone, Debug, ValueEnum, Default)]
pub enum InitTemplate {
	/// Full structure: tests/, scripts/, results/, reports/, screenshots/
	#[default]
	Standard,
	/// Minimal structure: tests/ only
	Minimal,
}

/// Output format for the read command
#[derive(
	Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum, serde::Serialize, serde::Deserialize,
)]
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

/// Page content extraction commands
#[derive(Subcommand, Debug)]
pub enum PageAction {
	/// Capture console messages and errors
	#[command(alias = "con")]
	Console(#[command(flatten)] crate::commands::page::console::ConsoleRaw),

	/// Evaluate JavaScript and return result
	Eval(#[command(flatten)] crate::commands::page::eval::EvalRaw),

	/// Get HTML content (full page or specific selector)
	Html(#[command(flatten)] crate::commands::page::html::HtmlRaw),

	/// Get coordinates for first matching element
	Coords(#[command(flatten)] crate::commands::page::coords::CoordsRaw),

	/// Get coordinates and info for all matching elements
	CoordsAll(#[command(flatten)] crate::commands::page::coords::CoordsRaw),

	/// Get text content of element
	Text(#[command(flatten)] crate::commands::page::text::TextRaw),

	/// Extract readable content from a web page
	Read(#[command(flatten)] crate::commands::page::read::ReadRaw),

	/// List interactive elements (buttons, links, inputs, selects)
	#[command(alias = "els")]
	Elements(#[command(flatten)] crate::commands::page::elements::ElementsRaw),

	/// Get a comprehensive page model (URL, title, elements, text) in one call
	#[command(alias = "snap")]
	Snapshot(#[command(flatten)] crate::commands::page::snapshot::SnapshotRaw),
}

#[derive(Subcommand, Debug)]
pub enum AuthAction {
	/// Interactive login - opens browser for manual login, then saves session
	Login {
		/// URL to navigate to for login (uses context when omitted)
		url: Option<String>,
		/// File to save authentication state to
		#[arg(short, long, default_value = "auth.json")]
		output: PathBuf,
		/// Wait time in seconds for manual login (default: 60)
		#[arg(short, long, default_value = "60")]
		timeout: u64,
	},

	/// Show cookies for a URL (uses saved auth if --auth provided)
	Cookies {
		/// URL to get cookies for
		url: Option<String>,
		/// Output format: json or table
		#[arg(short, long, default_value = "table")]
		format: String,
	},

	/// Show contents of a saved auth file
	Show {
		/// File to read authentication state from
		file: PathBuf,
	},

	/// Listen for cookies from browser extension
	///
	/// Starts a WebSocket server that receives cookies from the pw browser extension.
	/// A one-time token is displayed for authentication.
	Listen {
		/// Host to bind
		#[arg(long, default_value = "127.0.0.1")]
		host: String,
		/// Port to bind
		#[arg(long, default_value_t = 9271)]
		port: u16,
	},
}

#[derive(Subcommand, Debug)]
pub enum DaemonAction {
	/// Start the daemon (use --foreground to run in terminal)
	Start {
		#[arg(long)]
		foreground: bool,
	},
	/// Stop the running daemon
	Stop,
	/// Show daemon status
	Status,
}

#[derive(Subcommand, Debug)]
pub enum SessionAction {
	/// Show session descriptor status for the active context
	Status,
	/// Remove stored session descriptor for the active context
	Clear,
	/// Start a reusable local browser session and persist its endpoint
	Start {
		/// Run with a visible (headful) browser window
		#[arg(long)]
		headful: bool,
	},
	/// Stop the reusable local browser session and remove descriptor
	Stop,
}

#[derive(Subcommand, Debug)]
pub enum TabsAction {
	/// List all open tabs
	List,
	/// Switch to a tab by index or URL pattern
	Switch {
		/// Tab index (0-based) or URL/title pattern to match
		target: String,
	},
	/// Close a tab by index or URL pattern
	Close {
		/// Tab index (0-based) or URL/title pattern to match
		target: String,
	},
	/// Open a new tab, optionally with a URL
	New {
		/// URL to open in the new tab
		url: Option<String>,
	},
}

#[derive(Subcommand, Debug)]
pub enum ProtectAction {
	/// Add a URL pattern to protect (e.g., "discord.com", "slack.com")
	Add {
		/// URL pattern to protect (substring match, case-insensitive)
		pattern: String,
	},
	/// Remove a URL pattern from protection
	Remove {
		/// URL pattern to remove
		pattern: String,
	},
	/// List all protected URL patterns
	List,
}

impl Commands {
	/// Returns registry [`CommandId`] and JSON args for registry-backed commands.
	///
	/// [`CommandId`]: crate::commands::registry::CommandId
	pub fn into_registry_args(
		&self,
	) -> Option<(crate::commands::registry::CommandId, serde_json::Value)> {
		use crate::commands::registry::CommandId as Id;

		Some(match self {
			Commands::Navigate(args) => (Id::Navigate, serde_json::to_value(args).ok()?),
			Commands::Screenshot(args) => (Id::Screenshot, serde_json::to_value(args).ok()?),
			Commands::Click(args) => (Id::Click, serde_json::to_value(args).ok()?),
			Commands::Fill(args) => (Id::Fill, serde_json::to_value(args).ok()?),
			Commands::Wait(args) => (Id::Wait, serde_json::to_value(args).ok()?),
			Commands::Page(action) => return action.into_registry_args(),
			_ => return None,
		})
	}
}

impl PageAction {
	/// Returns registry [`CommandId`] and JSON args.
	///
	/// [`CommandId`]: crate::commands::registry::CommandId
	pub fn into_registry_args(
		&self,
	) -> Option<(crate::commands::registry::CommandId, serde_json::Value)> {
		use crate::commands::registry::CommandId as Id;

		Some(match self {
			PageAction::Console(args) => (Id::PageConsole, serde_json::to_value(args).ok()?),
			PageAction::Eval(args) => (Id::PageEval, serde_json::to_value(args).ok()?),
			PageAction::Html(args) => (Id::PageHtml, serde_json::to_value(args).ok()?),
			PageAction::Coords(args) => (Id::PageCoords, serde_json::to_value(args).ok()?),
			PageAction::CoordsAll(args) => (Id::PageCoordsAll, serde_json::to_value(args).ok()?),
			PageAction::Text(args) => (Id::PageText, serde_json::to_value(args).ok()?),
			PageAction::Read(args) => (Id::PageRead, serde_json::to_value(args).ok()?),
			PageAction::Elements(args) => (Id::PageElements, serde_json::to_value(args).ok()?),
			PageAction::Snapshot(args) => (Id::PageSnapshot, serde_json::to_value(args).ok()?),
		})
	}
}
