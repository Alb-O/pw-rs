use clap::Parser;
use pw_cli::{cli::Cli, commands, logging};
use tracing::error;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    logging::init_logging(cli.verbose);

    let auth_file = cli.auth.as_deref();
    if let Err(err) = commands::dispatch(cli.command, auth_file).await {
        error!(target = "pw", error = %err, "command failed");
        std::process::exit(1);
    }
}
