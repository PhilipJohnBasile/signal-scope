//! Structured logging bootstrap using `tracing`.

use anyhow::Result;
use tracing::Level;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Install a global tracing subscriber with sensible defaults.
pub fn init_tracing() -> Result<()> {
    if tracing::dispatcher::has_been_set() {
        return Ok(());
    }

    let env_filter = EnvFilter::try_from_default_env().or_else(|_| EnvFilter::try_new("info"))?;

    let timer = fmt::time::UtcTime::rfc_3339();

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_timer(timer)
        .with_level(true)
        .with_line_number(true)
        .with_file(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_filter(env_filter);

    let registry = tracing_subscriber::registry().with(fmt_layer);
    registry.init();

    tracing::debug!(level = ?Level::INFO, "tracing initialised");
    Ok(())
}
