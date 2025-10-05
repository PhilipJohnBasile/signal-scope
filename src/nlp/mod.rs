//! Natural language processing orchestration layer.

pub mod embeddings;
pub mod features;
pub mod ner;
pub mod relclf;

use anyhow::Result;
use tracing::info;

use crate::{cli::ExtractMode, config::Settings};

/// Run the end-to-end relation extraction pipeline.
pub async fn extract_relations(settings: &Settings, mode: ExtractMode) -> Result<()> {
    info!(?mode, "starting relation extraction");
    let ner = ner::load_model(settings).await?;
    let sentences = relclf::hydrate_sentences(settings).await?;
    let features = features::featurise(&sentences);
    relclf::train_and_predict(settings, ner.as_ref(), features, mode).await
}

/// Build embeddings for event deduplication.
pub async fn build_embeddings(settings: &Settings) -> Result<()> {
    embeddings::build_event_clusters(settings).await
}

/// Produce optional local summary text.
pub async fn summarize(
    settings: &Settings,
    drug: &str,
    event: &str,
    topk: usize,
) -> Result<String> {
    embeddings::summaries(settings, drug, event, topk).await
}
