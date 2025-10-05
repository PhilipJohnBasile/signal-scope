//! CLI entry-point for terminology normalization.

use anyhow::Result;
use tracing::instrument;

use crate::{config::Settings, data};

#[instrument(skip(settings))]
pub async fn run(settings: Settings) -> Result<()> {
    data::normalize::canonicalise(&settings).await?;
    Ok(())
}
