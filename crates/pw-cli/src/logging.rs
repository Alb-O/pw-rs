use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::EnvFilter;

pub fn init_logging(verbose: bool) {
    // Allow RUST_LOG overrides, fall back to cli flag-controlled level
    let default_level = if verbose { "debug" } else { "info" };
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_level));

    // Log to stderr; keep formatting compact
    let stderr = std::io::stderr.with_max_level(tracing::Level::TRACE);

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(stderr)
        .with_target(true)
        .with_level(true)
        .compact()
        .init();
}
