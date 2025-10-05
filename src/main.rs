//! Entry point wiring CLI dispatch to pipeline modules.

mod api;
mod cli;
mod config;
mod data;
mod logging;
mod nlp;
mod signals;
mod ui;

use anyhow::Result;
use cli::Cli;
use config::Settings;
use tracing::{info, instrument};

#[tokio::main]
#[instrument]
async fn main() -> Result<()> {
    logging::init_tracing()?;
    let settings = Settings::load()?;
    let cli = Cli::parse();

    info!(?cli, "starting command");
    cli.dispatch(settings).await
}
