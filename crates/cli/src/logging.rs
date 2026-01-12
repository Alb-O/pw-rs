use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::MakeWriterExt;

pub fn init_logging(verbosity: u8) {
    // 0 = silent (suppress pw-core protocol noise entirely)
    // 1 (-v) = info for pw-cli, warn for pw-core
    // 2+ (-vv) = debug/trace for everything
    let filter = match verbosity {
        0 => "error,pw_core=off,pw=off",
        1 => "info,pw_core=warn",
        _ => "debug",
    };

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter));

    let stderr = std::io::stderr.with_max_level(tracing::Level::TRACE);

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(stderr)
        .with_target(true)
        .with_level(true)
        .compact()
        .init();
}
