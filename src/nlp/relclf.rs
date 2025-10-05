//! Weak supervision and relation classification routines.

use std::{collections::HashMap, fs::File, path::PathBuf};

use anyhow::Result;
use linfa::{
    dataset::DatasetBase,
    prelude::{Fit, Predict},
};
use linfa_logistic::LogisticRegression;
use ndarray::{Array1, Array2};
use polars::prelude::{DataFrame, NamedFrom, ParquetReader, ParquetWriter, SerReader, Series};
use serde::Serialize;
use tracing::{info, warn};

use crate::{
    cli::ExtractMode,
    config::Settings,
    data::pubmed::PubRecord,
    nlp::features::{self, FeatureVector, SentenceContext},
    nlp::ner::Ner,
};

const EVENT_DICTIONARY: &[&str] = &[
    "hepatotoxicity",
    "rash",
    "diarrhoea",
    "neutropenia",
    "fatigue",
    "nausea",
    "fever",
    "cardiotoxicity",
    "anemia",
    "thrombocytopenia",
    "headache",
];

#[derive(Debug, Clone, Serialize)]
struct RelationRow {
    drug_id: String,
    event_id: String,
    pmid: String,
    sent_idx: i64,
    confidence: f64,
}

/// Load PubMed JSONL cache and generate candidate sentences.
pub async fn hydrate_sentences(settings: &Settings) -> Result<Vec<SentenceContext>> {
    let mut contexts = Vec::new();
    let root = settings.join_data("raw/pubmed");
    if !root.exists() {
        return Ok(contexts);
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if entry.path().extension().and_then(|s| s.to_str()) != Some("jsonl") {
            continue;
        }
        let drug = entry
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_lowercase();
        let file = std::fs::read_to_string(entry.path())?;
        for line in file.lines() {
            let record: PubRecord = serde_json::from_str(line)?;
            for (sent_idx, sentence) in features::split_sentences(&record.abstract_text)
                .into_iter()
                .enumerate()
            {
                let sentence_lower = sentence.to_lowercase();
                if !sentence_lower.contains(&drug) {
                    continue;
                }
                if let Some(event) = EVENT_DICTIONARY
                    .iter()
                    .find(|term| sentence_lower.contains(*term))
                {
                    contexts.push(SentenceContext {
                        pmid: record.pmid.clone(),
                        sent_idx,
                        drug: drug.clone(),
                        event: (*event).to_string(),
                        text: sentence,
                    });
                }
            }
        }
    }
    info!(count = contexts.len(), "built sentence contexts");
    Ok(contexts)
}

/// Train a logistic classifier (optionally) and persist predictions.
pub async fn train_and_predict(
    settings: &Settings,
    _ner: &dyn Ner,
    features: Vec<FeatureVector>,
    mode: ExtractMode,
) -> Result<()> {
    if features.is_empty() {
        warn!("no features generated; skipping relation extraction");
        return Ok(());
    }

    let labels: Vec<i32> = features
        .iter()
        .map(|f| {
            if f.has_cue_word > 0.5 && f.negation_flag < 0.5 {
                1
            } else {
                0
            }
        })
        .collect();
    let matrix: Vec<f64> = features
        .iter()
        .flat_map(|f| {
            [
                f.token_distance as f64,
                f.has_cue_word as f64,
                f.negation_flag as f64,
                f.co_mention_count as f64,
                f.tfidf_like as f64,
            ]
        })
        .collect();
    let rows = features.len();
    let x = Array2::from_shape_vec((rows, 5), matrix)?;
    let y = Array1::from(labels.clone());
    let dataset: DatasetBase<_, _> = DatasetBase::new(x.clone(), y.clone());

    let confidences: Vec<f64> = if mode.is_training() {
        let model = LogisticRegression::default().max_iterations(150);
        let fitted = model.fit(&dataset)?;
        fitted
            .predict(&dataset)
            .into_iter()
            .map(|value| value as f64)
            .collect()
    } else {
        labels.into_iter().map(|value| value as f64).collect()
    };

    persist_relations(settings, &features, confidences)?;
    Ok(())
}

fn persist_relations(
    settings: &Settings,
    features: &[FeatureVector],
    confidences: Vec<f64>,
) -> Result<()> {
    let drug_lookup = parquet_lookup(
        settings.join_data("clean/drugs.parquet"),
        "name_canonical",
        "drug_id",
    )?;
    let event_lookup = parquet_lookup(
        settings.join_data("clean/events.parquet"),
        "term_canonical",
        "event_id",
    )?;

    let mut rows = Vec::new();
    for (feat, conf) in features.iter().zip(confidences) {
        let drug_key = feat.drug.to_lowercase();
        let event_key = feat.event.to_lowercase();
        let Some(drug_id) = drug_lookup.get(&drug_key) else {
            continue;
        };
        let Some(event_id) = event_lookup.get(&event_key) else {
            continue;
        };
        rows.push(RelationRow {
            drug_id: drug_id.clone(),
            event_id: event_id.clone(),
            pmid: feat.pmid.clone(),
            sent_idx: feat.sent_idx as i64,
            confidence: conf,
        });
    }

    if rows.is_empty() {
        warn!("no relation rows satisfied lookup; skipping parquet write");
        return Ok(());
    }

    let drug_ids: Vec<String> = rows.iter().map(|r| r.drug_id.clone()).collect();
    let event_ids: Vec<String> = rows.iter().map(|r| r.event_id.clone()).collect();
    let pmids: Vec<String> = rows.iter().map(|r| r.pmid.clone()).collect();
    let sent_idx: Vec<i64> = rows.iter().map(|r| r.sent_idx).collect();
    let confidences: Vec<f64> = rows.iter().map(|r| r.confidence).collect();

    let mut df = DataFrame::new(vec![
        Series::new("drug_id".into(), drug_ids),
        Series::new("event_id".into(), event_ids),
        Series::new("pmid".into(), pmids),
        Series::new("sent_idx".into(), sent_idx),
        Series::new("confidence".into(), confidences),
    ])?;
    let path = settings.join_data("clean/relations.parquet");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(&path)?;
    ParquetWriter::new(file).finish(&mut df)?;
    info!(path = %path.display(), rows = rows.len(), "wrote relations parquet");
    Ok(())
}

fn parquet_lookup(path: PathBuf, key: &str, value: &str) -> Result<HashMap<String, String>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let df = ParquetReader::new(File::open(&path)?).finish()?;
    let key_col = df.column(key)?;
    let val_col = df.column(value)?;
    let mut map = HashMap::new();
    for (k, v) in key_col
        .str()?
        .into_no_null_iter()
        .zip(val_col.str()?.into_no_null_iter())
    {
        map.insert(k.to_string(), v.to_string());
    }
    Ok(map)
}
