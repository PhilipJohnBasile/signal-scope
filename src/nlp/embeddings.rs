//! Embedding and clustering utilities built on fastembed.

use std::fs::File;

use anyhow::Result;
use polars::prelude::{DataFrame, NamedFrom, ParquetReader, ParquetWriter, SerReader, Series};
use tracing::{info, warn};

#[cfg(feature = "embeddings")]
use fastembed::TextEmbedding;

use crate::config::Settings;

/// Compute MiniLM embeddings for canonical event terms and cluster near-duplicates.
pub async fn build_event_clusters(settings: &Settings) -> Result<()> {
    let events_path = settings.join_data("clean/events.parquet");
    if !events_path.exists() {
        warn!("event parquet missing; run normalize first");
        return Ok(());
    }
    let df = ParquetReader::new(File::open(&events_path)?).finish()?;
    let event_ids: Vec<String> = df
        .column("event_id")?
        .str()?
        .into_no_null_iter()
        .map(|s| s.to_string())
        .collect();
    let terms: Vec<String> = df
        .column("term_canonical")?
        .str()?
        .into_no_null_iter()
        .map(|s| s.to_string())
        .collect();
    if terms.is_empty() {
        return Ok(());
    }

    #[cfg(feature = "embeddings")]
    let clusters = {
        let embedder = TextEmbedding::try_new(Default::default())?;
        let documents: Vec<&str> = terms.iter().map(String::as_str).collect();
        let embeddings = embedder.embed(documents, None)?;
        cluster_embeddings(&embeddings, 0.85)
    };

    #[cfg(not(feature = "embeddings"))]
    let clusters = (0..terms.len()).collect::<Vec<_>>();
    let mut reps = std::collections::HashMap::new();
    for (idx, &cluster_id) in clusters.iter().enumerate() {
        reps.entry(cluster_id).or_insert_with(|| terms[idx].clone());
    }

    let cluster_ids: Vec<i64> = clusters.iter().map(|c| *c as i64).collect();
    let rep_terms: Vec<String> = clusters
        .iter()
        .map(|c| reps.get(c).cloned().unwrap_or_else(|| "unknown".into()))
        .collect();
    let mut df = DataFrame::new(vec![
        Series::new("event_id".into(), event_ids),
        Series::new("cluster_id".into(), cluster_ids),
        Series::new("rep_term".into(), rep_terms),
    ])?;
    let out_path = settings.join_data("clean/event_clusters.parquet");
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(&out_path)?;
    ParquetWriter::new(file).finish(&mut df)?;
    let unique_clusters = reps.len();
    info!(path = %out_path.display(), clusters = unique_clusters, "wrote event clusters");
    Ok(())
}

fn cluster_embeddings(embeddings: &[Vec<f32>], threshold: f32) -> Vec<usize> {
    let mut clusters: Vec<Vec<f32>> = Vec::new();
    let mut assignments = Vec::new();
    for vector in embeddings {
        if let Some((idx, _)) = clusters
            .iter()
            .enumerate()
            .find(|(_, centroid)| cosine(vector, centroid) >= threshold)
        {
            assignments.push(idx);
        } else {
            clusters.push(vector.clone());
            assignments.push(clusters.len() - 1);
        }
    }
    assignments
}

/// Expose clustering for integration tests.
pub fn cluster_preview(embeddings: &[Vec<f32>], _threshold: f32) -> Vec<usize> {
    if embeddings.is_empty() {
        return Vec::new();
    }
    #[cfg(feature = "embeddings")]
    {
        cluster_embeddings(embeddings, _threshold)
    }
    #[cfg(not(feature = "embeddings"))]
    {
        (0..embeddings.len()).collect()
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot = a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
    let norm_a = a.iter().map(|v| v * v).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

/// Summaries fallback when llama.cpp is not enabled.
#[cfg(not(feature = "summaries"))]
pub async fn summaries(
    _settings: &Settings,
    drug: &str,
    event: &str,
    topk: usize,
) -> Result<String> {
    Ok(format!(
        "Summary unavailable (LLM disabled). {drug} and {event} flagged in top {topk} supporting sentences. Cite relevant PMIDs from relations file."
    ))
}

/// Summaries using llama.cpp if feature enabled.
#[cfg(feature = "summaries")]
pub async fn summaries(
    settings: &Settings,
    drug: &str,
    event: &str,
    topk: usize,
) -> Result<String> {
    use llama_cpp_rs::{LLama, LLamaContextParams, LLamaModel, TokenId};

    let model_path = settings.join_data("models/llama-tiny.gguf");
    if !model_path.exists() {
        return Ok(format!(
            "Summary disabled – expected model {} not found.",
            model_path.display()
        ));
    }
    let model = LLamaModel::load_from_file(&model_path, Default::default())?;
    let ctx_params = LLamaContextParams::default();
    let ctx = LLama::new(model, ctx_params)?;
    let prompt = format!(
        "Summarise evidence for {drug} causing {event}. Include PMID references. Limit to {topk} sentences."
    );
    let tokens: Vec<TokenId> = ctx.model().tokenize(&prompt, true)?;
    let response = ctx.evaluate(&tokens, None, 256, None)?;
    Ok(response)
}
