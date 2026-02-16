use std::path::PathBuf;

use clap::Parser;

use super::*;

#[test]
fn parse_exec_with_input() {
	let cli = Cli::try_parse_from(["pw", "exec", "page.text", "--input", r#"{"selector":"h1"}"#]).unwrap();
	match cli.command {
		Commands::Exec(args) => {
			assert_eq!(args.op.as_deref(), Some("page.text"));
			assert_eq!(args.input.as_deref(), Some(r#"{"selector":"h1"}"#));
			assert_eq!(args.profile, "default");
		}
		_ => panic!("expected exec"),
	}
}

#[test]
fn parse_exec_with_file() {
	let cli = Cli::try_parse_from(["pw", "exec", "--file", "request.json", "--profile", "agent-a"]).unwrap();
	match cli.command {
		Commands::Exec(args) => {
			assert!(args.op.is_none());
			assert_eq!(args.file, Some(PathBuf::from("request.json")));
			assert_eq!(args.profile, "agent-a");
		}
		_ => panic!("expected exec"),
	}
}

#[test]
fn parse_batch() {
	let cli = Cli::try_parse_from(["pw", "batch", "--profile", "ci"]).unwrap();
	match cli.command {
		Commands::Batch(args) => assert_eq!(args.profile, "ci"),
		_ => panic!("expected batch"),
	}
}

#[test]
fn parse_profile_set() {
	let cli = Cli::try_parse_from(["pw", "profile", "set", "default", "--file", "cfg.json"]).unwrap();
	match cli.command {
		Commands::Profile(ProfileArgs {
			action: ProfileAction::Set { name, file },
		}) => {
			assert_eq!(name, "default");
			assert_eq!(file, PathBuf::from("cfg.json"));
		}
		_ => panic!("expected profile set"),
	}
}

#[test]
fn parse_daemon_start_foreground() {
	let cli = Cli::try_parse_from(["pw", "daemon", "start", "--foreground"]).unwrap();
	match cli.command {
		Commands::Daemon(DaemonArgs {
			action: DaemonAction::Start { foreground },
		}) => assert!(foreground),
		_ => panic!("expected daemon start"),
	}
}

#[test]
fn invalid_command_fails() {
	assert!(Cli::try_parse_from(["pw", "navigate", "https://example.com"]).is_err());
}
