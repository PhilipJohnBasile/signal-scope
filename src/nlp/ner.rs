//! Lightweight dictionary-based NER fallback. Swap with rust-bert when enabled.

use std::sync::Arc;

use anyhow::Result;
use once_cell::sync::Lazy;

use crate::config::Settings;

/// Extracted entity span with offsets relative to the source text.
#[derive(Debug, Clone)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub label: String,
    pub text: String,
    pub score: f64,
}

/// Trait for NER implementations.
pub trait Ner: Send + Sync {
    fn extract(&self, text: &str) -> Vec<Span>;
}

static DRUG_TERMS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        "imatinib",
        "gleevec",
        "dasatinib",
        "sprycel",
        "nilotinib",
        "tasigna",
        "nivolumab",
        "opdivo",
        "pembrolizumab",
        "keytruda",
        "ipilimumab",
        "yervoy",
    ]
});

static EVENT_TERMS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
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
    ]
});

struct DictionaryNer;

impl Ner for DictionaryNer {
    fn extract(&self, text: &str) -> Vec<Span> {
        let mut spans = Vec::new();
        spans.extend(find_terms(text, &DRUG_TERMS, "DRUG"));
        spans.extend(find_terms(text, &EVENT_TERMS, "EVENT"));
        spans
    }
}

fn find_terms(text: &str, terms: &[&str], label: &str) -> Vec<Span> {
    let lower = text.to_lowercase();
    let mut spans = Vec::new();
    for term in terms {
        let term_lower = term.to_lowercase();
        let mut start_pos = 0;
        while let Some(pos) = lower[start_pos..].find(&term_lower) {
            let start = start_pos + pos;
            let end = start + term_lower.len();
            spans.push(Span {
                start,
                end,
                label: label.to_string(),
                text: text[start..end].to_string(),
                score: 0.8,
            });
            start_pos = end;
        }
    }
    spans
}

/// Load a dictionary-backed NER implementation.
pub async fn load_model(_settings: &Settings) -> Result<Arc<dyn Ner>> {
    Ok(Arc::new(DictionaryNer) as Arc<dyn Ner>)
}
