use crate::cli::GctlLogLevel;
use tracing::Level;
use tracing_subscriber::{self, EnvFilter};

pub fn setup_logging(gctl_level: GctlLogLevel) {
    let level = match gctl_level {
        GctlLogLevel::Verbose => Level::DEBUG,
        GctlLogLevel::VeryVerbose => Level::TRACE,
        GctlLogLevel::Normal => Level::ERROR,
    };

    let filter = EnvFilter::from_default_env().add_directive(level.into());
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .with_target(false)
        .with_level(true)
        .with_ansi(true)
        .init();
}
