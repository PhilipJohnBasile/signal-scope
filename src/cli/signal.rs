//! CLI entry-point for computing signal metrics.

use anyhow::Result;
use tracing::instrument;

use crate::{config::Settings, signals};

#[instrument(skip(settings))]
pub async fn run(settings: Settings) -> Result<()> {
    signals::compute(&settings).await
}
