//! Lightweight sentence feature engineering for relation extraction.

use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::debug;

/// Representation of a sentence mentioning a drug and event.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SentenceContext {
    pub pmid: String,
    pub sent_idx: usize,
    pub drug: String,
    pub event: String,
    pub text: String,
}

/// Numerical features used by the logistic relation classifier.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeatureVector {
    pub pmid: String,
    pub sent_idx: usize,
    pub drug: String,
    pub event: String,
    pub token_distance: f32,
    pub has_cue_word: f32,
    pub negation_flag: f32,
    pub co_mention_count: f32,
    pub tfidf_like: f32,
}

/// Split abstract text into coarse sentences.
pub fn split_sentences(text: &str) -> Vec<String> {
    static PATTERN: once_cell::sync::Lazy<Regex> =
        once_cell::sync::Lazy::new(|| Regex::new(r"(?m)(?<=[.!?])\s+").expect("valid regex"));
    PATTERN
        .split(text)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Convert sentences into feature vectors.
pub fn featurise(sentences: &[SentenceContext]) -> Vec<FeatureVector> {
    sentences
        .iter()
        .map(|ctx| FeatureVector {
            pmid: ctx.pmid.clone(),
            sent_idx: ctx.sent_idx,
            drug: ctx.drug.clone(),
            event: ctx.event.clone(),
            token_distance: token_distance(ctx),
            has_cue_word: cue_word(ctx),
            negation_flag: negation(ctx),
            co_mention_count: co_mentions(ctx),
            tfidf_like: tfidf_like(ctx),
        })
        .collect()
}

fn token_distance(ctx: &SentenceContext) -> f32 {
    let tokens: Vec<&str> = ctx.text.split_whitespace().collect();
    let drug_idx = tokens
        .iter()
        .position(|token| token.eq_ignore_ascii_case(&ctx.drug));
    let event_idx = tokens
        .iter()
        .position(|token| token.eq_ignore_ascii_case(&ctx.event));
    match (drug_idx, event_idx) {
        (Some(d), Some(e)) => (d as f32 - e as f32).abs(),
        _ => tokens.len() as f32,
    }
}

fn cue_word(ctx: &SentenceContext) -> f32 {
    const CUE_WORDS: &[&str] = &["associated", "induced", "triggered", "linked"];
    let text = ctx.text.to_lowercase();
    if CUE_WORDS.iter().any(|cue| text.contains(cue)) {
        1.0
    } else {
        0.0
    }
}

fn negation(ctx: &SentenceContext) -> f32 {
    const NEGATIONS: &[&str] = &["no", "not", "without", "neither"];
    let text = ctx.text.to_lowercase();
    if NEGATIONS.iter().any(|cue| text.contains(cue)) {
        1.0
    } else {
        0.0
    }
}

fn co_mentions(ctx: &SentenceContext) -> f32 {
    let lower = ctx.text.to_lowercase();
    let event = ctx.event.to_lowercase();
    lower.matches(&event).count() as f32
}

fn tfidf_like(ctx: &SentenceContext) -> f32 {
    let token_count = ctx.text.split_whitespace().count() as f32 + 1e-6;
    (ctx.event.len() as f32 / token_count).min(5.0)
}

/// Convenience helper for instrumentation.
pub fn log_feature_preview(features: &[FeatureVector]) {
    debug!(count = features.len(), "generated features");
}
