use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::output::OutputFormat;
use crate::styles::cli_styles;
use crate::types::BrowserKind;

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
    pub format: CliOutputFormat,

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

    #[command(subcommand)]
    pub command: Commands,
}

/// CLI output format (clap-compatible enum)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum CliOutputFormat {
    /// TOON output (default, token-efficient for LLMs)
    #[default]
    Toon,
    /// JSON output
    Json,
    /// Newline-delimited JSON (streaming)
    Ndjson,
    /// Human-readable text
    Text,
}

impl From<CliOutputFormat> for OutputFormat {
    fn from(f: CliOutputFormat) -> Self {
        match f {
            CliOutputFormat::Toon => OutputFormat::Toon,
            CliOutputFormat::Json => OutputFormat::Json,
            CliOutputFormat::Ndjson => OutputFormat::Ndjson,
            CliOutputFormat::Text => OutputFormat::Text,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Navigate to URL and check for console errors
    #[command(alias = "nav")]
    Navigate {
        /// Target URL (positional, uses context when omitted)
        url: Option<String>,
        /// Target URL (named alternative)
        #[arg(long = "url", short = 'u', value_name = "URL")]
        url_flag: Option<String>,
    },

    /// Capture console messages and errors
    #[command(alias = "con")]
    Console {
        /// Target URL (positional, uses context when omitted)
        url: Option<String>,
        /// Time to wait for console messages (ms)
        #[arg(default_value = "3000")]
        timeout_ms: u64,
        /// Target URL (named alternative)
        #[arg(long = "url", short = 'u', value_name = "URL")]
        url_flag: Option<String>,
    },

    /// Evaluate JavaScript and return result
    ///
    /// The expression can be provided positionally, via --expr/-e, or read from a file.
    /// When using named flags, the order doesn't matter.
    Eval {
        /// JavaScript expression (positional). Required unless --expr or --file is used.
        expression: Option<String>,
        /// Target URL (positional, uses context when omitted)
        url: Option<String>,
        /// JavaScript expression (named alternative to positional)
        #[arg(long = "expr", short = 'e', value_name = "EXPRESSION")]
        expression_flag: Option<String>,
        /// Read JavaScript expression from file (avoids shell argument limits for large scripts)
        #[arg(long = "file", short = 'F', value_name = "FILE")]
        file: Option<PathBuf>,
        /// Target URL (named alternative to positional)
        #[arg(long = "url", short = 'u', value_name = "URL")]
        url_flag: Option<String>,
    },

    /// Get HTML content (full page or specific selector)
    Html {
        /// Target URL (positional, uses context when omitted)
        url: Option<String>,
        /// CSS selector (positional, uses last selector or defaults to html)
        selector: Option<String>,
        /// Target URL (named alternative)
        #[arg(long = "url", short = 'u', value_name = "URL")]
        url_flag: Option<String>,
        /// CSS selector (named alternative)
        #[arg(long = "selector", short = 's', value_name = "SELECTOR")]
        selector_flag: Option<String>,
    },

    /// Get coordinates for first matching element
    Coords {
        /// Target URL (positional)
        url: Option<String>,
        /// CSS selector (positional)
        selector: Option<String>,
        /// Target URL (named alternative)
        #[arg(long = "url", short = 'u', value_name = "URL")]
        url_flag: Option<String>,
        /// CSS selector (named alternative)
        #[arg(long = "selector", short = 's', value_name = "SELECTOR")]
        selector_flag: Option<String>,
    },

    /// Get coordinates and info for all matching elements
    CoordsAll {
        /// Target URL (positional)
        url: Option<String>,
        /// CSS selector (positional)
        selector: Option<String>,
        /// Target URL (named alternative)
        #[arg(long = "url", short = 'u', value_name = "URL")]
        url_flag: Option<String>,
        /// CSS selector (named alternative)
        #[arg(long = "selector", short = 's', value_name = "SELECTOR")]
        selector_flag: Option<String>,
    },

    /// Take screenshot
    #[command(alias = "ss")]
    Screenshot {
        /// Target URL (positional, uses context when omitted)
        url: Option<String>,
        /// Output file path (uses context or defaults when omitted)
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        /// Capture the full scrollable page instead of just the viewport
        #[arg(long)]
        full_page: bool,
        /// Target URL (named alternative)
        #[arg(long = "url", short = 'u', value_name = "URL")]
        url_flag: Option<String>,
    },

    /// Click element and show resulting URL
    Click {
        /// Target URL (positional)
        url: Option<String>,
        /// CSS selector (positional)
        selector: Option<String>,
        /// Target URL (named alternative)
        #[arg(long = "url", short = 'u', value_name = "URL")]
        url_flag: Option<String>,
        /// CSS selector (named alternative)
        #[arg(long = "selector", short = 's', value_name = "SELECTOR")]
        selector_flag: Option<String>,
        /// Time to wait for navigation after click (milliseconds)
        #[arg(long, default_value = "500")]
        wait_ms: u64,
    },

    /// Get text content of element
    Text {
        /// Target URL (positional)
        url: Option<String>,
        /// CSS selector (positional)
        selector: Option<String>,
        /// Target URL (named alternative)
        #[arg(long = "url", short = 'u', value_name = "URL")]
        url_flag: Option<String>,
        /// CSS selector (named alternative)
        #[arg(long = "selector", short = 's', value_name = "SELECTOR")]
        selector_flag: Option<String>,
    },

    /// Fill text into an input field (works with React and other frameworks)
    Fill {
        /// Text to fill into the input
        text: String,
        /// CSS selector for the input element
        #[arg(long = "selector", short = 's', value_name = "SELECTOR")]
        selector: Option<String>,
        /// Target URL (named alternative)
        #[arg(long = "url", short = 'u', value_name = "URL")]
        url: Option<String>,
    },

    /// Extract readable content from a web page
    ///
    /// Removes clutter (ads, navigation, sidebars) and extracts the main article content.
    /// Useful for reading articles, blog posts, and documentation.
    Read {
        /// Target URL (positional)
        url: Option<String>,
        /// Target URL (named alternative)
        #[arg(long = "url", short = 'u', value_name = "URL")]
        url_flag: Option<String>,
        /// Output format: markdown (default), text, or html
        #[arg(long, short = 'o', default_value = "markdown", value_enum)]
        output_format: ReadOutputFormat,
        /// Include metadata (title, author, etc.) in output
        #[arg(long, short = 'm')]
        metadata: bool,
    },

    /// List interactive elements (buttons, links, inputs, selects)
    #[command(alias = "els")]
    Elements {
        /// Target URL (positional)
        url: Option<String>,
        /// Wait for elements with polling (useful for dynamic pages)
        #[arg(long)]
        wait: bool,
        /// Timeout in milliseconds for --wait mode (default: 10000)
        #[arg(long, default_value = "10000")]
        timeout_ms: u64,
        /// Target URL (named alternative)
        #[arg(long = "url", short = 'u', value_name = "URL")]
        url_flag: Option<String>,
    },

    /// Wait for condition (selector, timeout, or load state)
    Wait {
        /// Target URL (positional)
        url: Option<String>,
        /// Condition to wait for (selector, timeout ms, or load state)
        #[arg(default_value = "networkidle")]
        condition: String,
        /// Target URL (named alternative)
        #[arg(long = "url", short = 'u', value_name = "URL")]
        url_flag: Option<String>,
    },

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

    /// Set or show the CDP endpoint for connecting to a running browser
    ///
    /// Once set, all pw commands will use this endpoint instead of launching a new browser.
    /// Use `pw connect --clear` to remove the saved endpoint.
    Connect {
        /// CDP WebSocket endpoint URL (e.g., ws://127.0.0.1:9222/devtools/browser/...)
        endpoint: Option<String>,
        /// Clear the saved CDP endpoint
        #[arg(long)]
        clear: bool,
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum ReadOutputFormat {
    /// Plain text
    Text,
    /// Cleaned HTML
    Html,
    /// Markdown (default)
    #[default]
    Markdown,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_screenshot_command() {
        let args = vec![
            "pw",
            "screenshot",
            "https://example.com",
            "-o",
            "/tmp/test.png",
        ];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Screenshot {
                url,
                output,
                full_page,
                ..
            } => {
                assert_eq!(url.as_deref(), Some("https://example.com"));
                assert_eq!(output, Some(PathBuf::from("/tmp/test.png")));
                assert!(!full_page);
            }
            _ => panic!("Expected Screenshot command"),
        }
    }

    #[test]
    fn parse_screenshot_default_output() {
        let args = vec!["pw", "screenshot", "https://example.com"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Screenshot {
                url,
                output,
                full_page,
                ..
            } => {
                assert_eq!(url.as_deref(), Some("https://example.com"));
                assert_eq!(output, None);
                assert!(!full_page);
            }
            _ => panic!("Expected Screenshot command"),
        }
    }

    #[test]
    fn parse_html_command() {
        let args = vec!["pw", "html", "https://example.com", "div.content"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Html { url, selector, .. } => {
                assert_eq!(url.as_deref(), Some("https://example.com"));
                assert_eq!(selector.as_deref(), Some("div.content"));
            }
            _ => panic!("Expected Html command"),
        }
    }

    #[test]
    fn parse_wait_command() {
        let args = vec!["pw", "wait", "https://example.com", "networkidle"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Wait { url, condition, .. } => {
                assert_eq!(url.as_deref(), Some("https://example.com"));
                assert_eq!(condition, "networkidle");
            }
            _ => panic!("Expected Wait command"),
        }
    }

    #[test]
    fn verbose_flag_short_and_long() {
        let short_args = vec!["pw", "-v", "screenshot", "https://example.com"];
        let short_cli = Cli::try_parse_from(short_args).unwrap();
        assert_eq!(short_cli.verbose, 1);

        let long_args = vec!["pw", "--verbose", "screenshot", "https://example.com"];
        let long_cli = Cli::try_parse_from(long_args).unwrap();
        assert_eq!(long_cli.verbose, 1);

        let double_v = vec!["pw", "-vv", "screenshot", "https://example.com"];
        let double_cli = Cli::try_parse_from(double_v).unwrap();
        assert_eq!(double_cli.verbose, 2);
    }

    #[test]
    fn parse_cdp_endpoint_flag() {
        let args = vec![
            "pw",
            "--cdp-endpoint",
            "ws://localhost:19988/cdp",
            "navigate",
            "https://example.com",
        ];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(
            cli.cdp_endpoint.as_deref(),
            Some("ws://localhost:19988/cdp")
        );
    }

    #[test]
    fn parse_relay_command() {
        let args = vec!["pw", "relay", "--host", "0.0.0.0", "--port", "3000"];
        let cli = Cli::try_parse_from(args).unwrap();
        match cli.command {
            Commands::Relay { host, port } => {
                assert_eq!(host, "0.0.0.0");
                assert_eq!(port, 3000);
            }
            _ => panic!("Expected Relay command"),
        }
    }

    #[test]
    fn invalid_command_fails() {
        let args = vec!["pw", "unknown-command", "https://example.com"];
        assert!(Cli::try_parse_from(args).is_err());
    }

    #[test]
    fn parse_click_with_named_flags() {
        // Test using named flags instead of positional args
        let args = vec![
            "pw",
            "click",
            "--url",
            "https://example.com",
            "--selector",
            "button.submit",
        ];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Click {
                url,
                selector,
                url_flag,
                selector_flag,
                ..
            } => {
                // Positional args should be None
                assert!(url.is_none());
                assert!(selector.is_none());
                // Named flags should have values
                assert_eq!(url_flag.as_deref(), Some("https://example.com"));
                assert_eq!(selector_flag.as_deref(), Some("button.submit"));
            }
            _ => panic!("Expected Click command"),
        }
    }

    #[test]
    fn parse_eval_with_named_flags() {
        // Test eval with --expr and --url flags (order-independent)
        let args = vec![
            "pw",
            "eval",
            "--url",
            "https://example.com",
            "--expr",
            "document.title",
        ];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Eval {
                expression,
                url,
                expression_flag,
                url_flag,
            } => {
                assert!(expression.is_none());
                assert!(url.is_none());
                assert_eq!(expression_flag.as_deref(), Some("document.title"));
                assert_eq!(url_flag.as_deref(), Some("https://example.com"));
            }
            _ => panic!("Expected Eval command"),
        }
    }
}
