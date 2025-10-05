//! Runtime configuration utilities for rwe-assistant.

use std::{
    env,
    path::{Path, PathBuf},
};

use anyhow::Context;
use serde::Deserialize;

/// Application configuration resolved from `.env` and defaults.
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    /// Contact email for PubMed E-utilities courtesy policy.
    pub pubmed_email: String,
    /// Tool name sent with PubMed requests.
    pub pubmed_tool: String,
    /// Maximum abstracts fetched per drug.
    pub max_pubmed_per_drug: usize,
    /// Root folder for cached data artefacts.
    pub data_dir: PathBuf,
    /// Root folder for analytic outputs.
    pub outputs_dir: PathBuf,
}

impl Settings {
    /// Load configuration from environment with reasonable defaults.
    pub fn load() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();
        let pubmed_email =
            env::var("PUBMED_EMAIL").unwrap_or_else(|_| "research@example.com".to_string());
        let pubmed_tool = env::var("PUBMED_TOOL").unwrap_or_else(|_| "rwe_assistant".to_string());
        let max_pubmed_per_drug = env::var("MAX_PUBMED_PER_DRUG")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(150);
        let data_dir = env::var("DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./data"));
        let outputs_dir = env::var("OUTPUTS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./outputs"));

        std::fs::create_dir_all(&data_dir).context("creating data dir")?;
        std::fs::create_dir_all(&outputs_dir).context("creating outputs dir")?;

        Ok(Self {
            pubmed_email,
            pubmed_tool,
            max_pubmed_per_drug,
            data_dir,
            outputs_dir,
        })
    }

    /// Convenience helper for derived path segments.
    pub fn join_data<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.data_dir.join(path)
    }

    /// Convenience helper for derived output path segments.
    pub fn join_output<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.outputs_dir.join(path)
    }
}
