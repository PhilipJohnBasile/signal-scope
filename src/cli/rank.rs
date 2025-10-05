//! CLI entry-point for ranking signal outputs.

use anyhow::Result;
use tracing::instrument;

use crate::{config::Settings, signals};

#[instrument(skip(settings))]
pub async fn run(settings: Settings) -> Result<()> {
    signals::rank(&settings).await
}
