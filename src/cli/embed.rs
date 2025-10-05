//! CLI entry-point for embedding and clustering event terminology.

use anyhow::Result;
use tracing::instrument;

use crate::{config::Settings, nlp};

#[instrument(skip(settings))]
pub async fn run(settings: Settings) -> Result<()> {
    nlp::build_embeddings(&settings).await
}
