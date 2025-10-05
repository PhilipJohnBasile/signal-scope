//! Data ingestion and normalisation layer.

pub mod faers;
pub mod normalize;
pub mod pubmed;
#[cfg(feature = "duckdb")]
pub mod store;
