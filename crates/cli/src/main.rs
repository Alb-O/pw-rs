use clap::Parser;
use pw_cli::cli::Cli;
use pw_cli::error::PwError;
use pw_cli::output::{self, OutputFormat, ResultBuilder};
use pw_cli::{commands, logging};

#[tokio::main]
async fn main() {
	let cli = Cli::parse();
	logging::init_logging(cli.verbose);

	let format = cli.format;

	if let Err(err) = commands::dispatch(cli, format).await {
		// If output was already printed (e.g., with artifacts), just exit
		if !err.is_output_already_printed() {
			handle_error(err, format);
		}
		std::process::exit(1);
	}
}

fn handle_error(err: PwError, format: OutputFormat) {
	// Convert error to structured output
	let cmd_error = err.to_command_error();

	// Always print to stderr for humans
	output::print_error_stderr(&cmd_error);

	// Also emit JSON envelope to stdout with ok=false (for agents)
	if format != OutputFormat::Text {
		let result: output::CommandResult<()> = ResultBuilder::new("unknown")
			.error(cmd_error.code, &cmd_error.message)
			.build();
		output::print_result(&result, format);
	}
}
