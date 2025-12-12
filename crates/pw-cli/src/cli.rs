use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "pw")]
#[command(about = "Playwright CLI - Browser automation from the command line")]
#[command(version)]
pub struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Load authentication state from file (cookies, localStorage)
    #[arg(long, global = true, value_name = "FILE")]
    pub auth: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Navigate to URL and check for console errors
    #[command(alias = "nav")]
    Navigate { url: String },

    /// Capture console messages and errors
    #[command(alias = "con")]
    Console {
        url: String,
        /// Time to wait for console messages (ms)
        #[arg(default_value = "3000")]
        timeout_ms: u64,
    },

    /// Evaluate JavaScript and return result
    Eval { url: String, expression: String },

    /// Get HTML content (full page or specific selector)
    Html {
        url: String,
        /// CSS selector (defaults to html)
        #[arg(default_value = "html")]
        selector: String,
    },

    /// Get coordinates for first matching element
    Coords { url: String, selector: String },

    /// Get coordinates and info for all matching elements
    CoordsAll { url: String, selector: String },

    /// Take full-page screenshot
    #[command(alias = "ss")]
    Screenshot {
        url: String,
        /// Output file path
        #[arg(default_value = "screenshot.png")]
        output: PathBuf,
    },

    /// Click element and show resulting URL
    Click { url: String, selector: String },

    /// Get text content of element
    Text { url: String, selector: String },

    /// Wait for condition (selector, timeout, or load state)
    Wait { url: String, condition: String },

    /// Authentication and session management
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum AuthAction {
    /// Interactive login - opens browser for manual login, then saves session
    Login {
        /// URL to navigate to for login
        url: String,
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
        url: String,
        /// Output format: json or table
        #[arg(short, long, default_value = "table")]
        format: String,
    },

    /// Show current storage state (cookies + localStorage)
    Show {
        /// Auth file to display
        #[arg(default_value = "auth.json")]
        file: PathBuf,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_screenshot_command() {
        let args = vec!["pw", "screenshot", "https://example.com", "/tmp/test.png"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Screenshot { url, output } => {
                assert_eq!(url, "https://example.com");
                assert_eq!(output, PathBuf::from("/tmp/test.png"));
            }
            _ => panic!("Expected Screenshot command"),
        }
    }

    #[test]
    fn parse_screenshot_default_output() {
        let args = vec!["pw", "screenshot", "https://example.com"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Screenshot { url, output } => {
                assert_eq!(url, "https://example.com");
                assert_eq!(output, PathBuf::from("screenshot.png"));
            }
            _ => panic!("Expected Screenshot command"),
        }
    }

    #[test]
    fn parse_html_command() {
        let args = vec!["pw", "html", "https://example.com", "div.content"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Html { url, selector } => {
                assert_eq!(url, "https://example.com");
                assert_eq!(selector, "div.content");
            }
            _ => panic!("Expected Html command"),
        }
    }

    #[test]
    fn parse_wait_command() {
        let args = vec!["pw", "wait", "https://example.com", "networkidle"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Wait { url, condition } => {
                assert_eq!(url, "https://example.com");
                assert_eq!(condition, "networkidle");
            }
            _ => panic!("Expected Wait command"),
        }
    }

    #[test]
    fn verbose_flag_short_and_long() {
        let short_args = vec!["pw", "-v", "screenshot", "https://example.com"];
        let short_cli = Cli::try_parse_from(short_args).unwrap();
        assert!(short_cli.verbose);

        let long_args = vec!["pw", "--verbose", "screenshot", "https://example.com"];
        let long_cli = Cli::try_parse_from(long_args).unwrap();
        assert!(long_cli.verbose);
    }

    #[test]
    fn invalid_command_fails() {
        let args = vec!["pw", "unknown-command", "https://example.com"];
        assert!(Cli::try_parse_from(args).is_err());
    }
}
