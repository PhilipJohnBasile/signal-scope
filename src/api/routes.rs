//! HTTP route handlers for Axum.

use std::cmp::Ordering;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use csv::ReaderBuilder;
use serde::Deserialize;
use tracing::warn;

use crate::{
    api::types::{EventDto, SignalDto},
    config::Settings,
};

use super::AppState;

type ApiResult<T> = Result<Json<T>, (StatusCode, String)>;

#[derive(Debug, Deserialize)]
pub struct SignalQuery {
    pub drug: Option<String>,
}

pub async fn list_signals(
    states: State<AppState>,
    Query(query): Query<SignalQuery>,
) -> ApiResult<Vec<SignalDto>> {
    let mut signals = load_signals(&states.settings)?;
    if let Some(drug) = query.drug {
        let drug_norm = drug.to_ascii_uppercase();
        signals.retain(|s| s.drug_id.to_ascii_uppercase() == drug_norm);
    }
    signals.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
    signals.truncate(100);
    Ok(Json(signals))
}

pub async fn list_events(
    Path(drug_id): Path<String>,
    states: State<AppState>,
) -> ApiResult<Vec<EventDto>> {
    let signals = load_signals(&states.settings)?;
    let drug_norm = drug_id.to_ascii_uppercase();
    let mut events: Vec<EventDto> = signals
        .into_iter()
        .filter(|s| s.drug_id.to_ascii_uppercase() == drug_norm)
        .map(|s| EventDto {
            drug_id: s.drug_id,
            event_id: s.event_id,
            year_quarter: s.year_quarter,
            recent_ror: s.recent_ror,
            ci_low: s.ci_low,
            ci_high: s.ci_high,
            trend_z: s.trend_z,
        })
        .collect();
    events.sort_by(|a, b| {
        b.recent_ror
            .partial_cmp(&a.recent_ror)
            .unwrap_or(Ordering::Equal)
    });
    events.truncate(200);
    Ok(Json(events))
}

fn load_signals(settings: &Settings) -> Result<Vec<SignalDto>, (StatusCode, String)> {
    let path = settings.join_output("signals.csv");
    if !path.exists() {
        warn!("signals.csv missing; run rank first");
        return Ok(Vec::new());
    }
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(&path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut out = Vec::new();
    for result in reader.deserialize::<RawSignal>() {
        match result {
            Ok(raw) => out.push(raw.into()),
            Err(err) => return Err((StatusCode::INTERNAL_SERVER_ERROR, err.to_string())),
        }
    }
    Ok(out)
}

#[derive(Debug, Deserialize)]
struct RawSignal {
    drug_id: String,
    event_id: String,
    year_quarter: String,
    recent_ror: f64,
    ci_low: f64,
    ci_high: f64,
    lit_support: i64,
    trend_z: f64,
    score: f64,
}

impl From<RawSignal> for SignalDto {
    fn from(value: RawSignal) -> Self {
        SignalDto {
            drug_id: value.drug_id,
            event_id: value.event_id,
            year_quarter: value.year_quarter,
            recent_ror: value.recent_ror,
            ci_low: value.ci_low,
            ci_high: value.ci_high,
            lit_support: value.lit_support,
            trend_z: value.trend_z,
            score: value.score,
        }
    }
}
