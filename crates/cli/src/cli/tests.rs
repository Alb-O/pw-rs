use std::path::PathBuf;

use clap::Parser;

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
		Commands::Screenshot(args) => {
			assert_eq!(args.url.as_deref(), Some("https://example.com"));
			assert_eq!(args.output, Some(PathBuf::from("/tmp/test.png")));
			assert_eq!(args.full_page, None);
		}
		_ => panic!("Expected Screenshot command"),
	}
}

#[test]
fn parse_screenshot_default_output() {
	let args = vec!["pw", "screenshot", "https://example.com"];
	let cli = Cli::try_parse_from(args).unwrap();

	match cli.command {
		Commands::Screenshot(args) => {
			assert_eq!(args.url.as_deref(), Some("https://example.com"));
			assert_eq!(args.output, None);
			assert_eq!(args.full_page, None);
		}
		_ => panic!("Expected Screenshot command"),
	}
}

#[test]
fn parse_page_html_command() {
	let args = vec!["pw", "page", "html", "https://example.com", "div.content"];
	let cli = Cli::try_parse_from(args).unwrap();

	match cli.command {
		Commands::Page(PageAction::Html(args)) => {
			assert_eq!(args.url.as_deref(), Some("https://example.com"));
			assert_eq!(args.selector.as_deref(), Some("div.content"));
		}
		_ => panic!("Expected Page Html command"),
	}
}

#[test]
fn parse_wait_command() {
	let args = vec!["pw", "wait", "https://example.com", "networkidle"];
	let cli = Cli::try_parse_from(args).unwrap();

	match cli.command {
		Commands::Wait(args) => {
			assert_eq!(args.url.as_deref(), Some("https://example.com"));
			assert_eq!(args.condition.as_deref(), Some("networkidle"));
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
		Commands::Click(args) => {
			// Positional args should be None
			assert!(args.url.is_none());
			assert!(args.selector.is_none());
			// Named flags should have values
			assert_eq!(args.url_flag.as_deref(), Some("https://example.com"));
			assert_eq!(args.selector_flag.as_deref(), Some("button.submit"));
		}
		_ => panic!("Expected Click command"),
	}
}

#[test]
fn parse_page_eval_with_named_flags() {
	// Test eval with --expr and --url flags (order-independent)
	let args = vec![
		"pw",
		"page",
		"eval",
		"--url",
		"https://example.com",
		"--expr",
		"document.title",
	];
	let cli = Cli::try_parse_from(args).unwrap();

	match cli.command {
		Commands::Page(PageAction::Eval(args)) => {
			assert!(args.expression.is_none());
			assert!(args.url.is_none());
			assert_eq!(args.expression_flag.as_deref(), Some("document.title"));
			assert_eq!(args.url_flag.as_deref(), Some("https://example.com"));
		}
		_ => panic!("Expected Page Eval command"),
	}
}

#[test]
fn parse_har_flags() {
	let args = vec![
		"pw",
		"--har",
		"network.har",
		"--har-content",
		"embed",
		"--har-mode",
		"minimal",
		"--har-omit-content",
		"--har-url-filter",
		"*.api.example.com",
		"navigate",
		"https://example.com",
	];
	let cli = Cli::try_parse_from(args).unwrap();

	assert_eq!(
		cli.har.as_deref(),
		Some(std::path::Path::new("network.har"))
	);
	assert_eq!(cli.har_content, CliHarContentPolicy::Embed);
	assert_eq!(cli.har_mode, CliHarMode::Minimal);
	assert!(cli.har_omit_content);
	assert_eq!(cli.har_url_filter.as_deref(), Some("*.api.example.com"));
}

#[test]
fn parse_har_defaults() {
	let args = vec!["pw", "navigate", "https://example.com"];
	let cli = Cli::try_parse_from(args).unwrap();

	assert!(cli.har.is_none());
	assert_eq!(cli.har_content, CliHarContentPolicy::Attach);
	assert_eq!(cli.har_mode, CliHarMode::Full);
	assert!(!cli.har_omit_content);
	assert!(cli.har_url_filter.is_none());
}
