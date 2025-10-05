//! CLI entry-point for serving the HTTP API and static UI.

use anyhow::Result;
use clap::Args as ClapArgs;
use tracing::instrument;

use crate::{api, config::Settings};

/// Run the Axum server.
#[derive(Debug, Clone, ClapArgs)]
pub struct Args {
    /// Port to bind (default 8080).
    #[arg(long, default_value_t = 8080)]
    pub port: u16,
    /// Host address, defaults to localhost.
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
}

#[instrument(skip(settings))]
pub async fn run(args: Args, settings: Settings) -> Result<()> {
    api::serve(settings, args.host, args.port).await
}
