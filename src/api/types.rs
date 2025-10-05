//! Shared DTOs for JSON responses.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SignalDto {
    pub drug_id: String,
    pub event_id: String,
    pub year_quarter: String,
    pub recent_ror: f64,
    pub ci_low: f64,
    pub ci_high: f64,
    pub lit_support: i64,
    pub trend_z: f64,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventDto {
    pub drug_id: String,
    pub event_id: String,
    pub year_quarter: String,
    pub recent_ror: f64,
    pub ci_low: f64,
    pub ci_high: f64,
    pub trend_z: f64,
}
