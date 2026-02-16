use clap::Parser;
use pw_cli::cli::Cli;
use pw_cli::error::PwError;
use pw_cli::{commands, logging};

#[tokio::main]
async fn main() {
	let cli = Cli::parse();
	logging::init_logging(cli.verbose);
	if let Err(err) = commands::dispatch(cli).await {
		handle_error(err);
		std::process::exit(1);
	}
}

fn handle_error(err: PwError) {
	eprintln!("Error: {err}");
}
