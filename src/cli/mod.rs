//! Command-line interface wiring for rwe-assistant.

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

use crate::config::Settings;

pub mod embed;
pub mod extract;
pub mod fetch;
pub mod normalize;
pub mod rank;
pub mod serve;
pub mod signal;
pub mod summarize;

/// Top-level CLI definition.
#[derive(Debug, Parser)]
#[command(author, version, about = "Real-world evidence assistant", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

impl Cli {
    /// Parse CLI arguments from the environment.
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }

    /// Dispatch the selected sub-command.
    pub async fn dispatch(self, settings: Settings) -> Result<()> {
        match self.command {
            Commands::Fetch(args) => fetch::run(args, settings).await,
            Commands::Normalize => normalize::run(settings).await,
            Commands::Extract(args) => extract::run(args, settings).await,
            Commands::Embed => embed::run(settings).await,
            Commands::Signal => signal::run(settings).await,
            Commands::Rank => rank::run(settings).await,
            Commands::Serve(args) => serve::run(args, settings).await,
            Commands::Summarize(args) => summarize::run(args, settings).await,
        }
    }
}

/// Supported sub-commands.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Download FAERS and PubMed artefacts.
    Fetch(fetch::Args),
    /// Canonicalise terminology and aggregate counts.
    Normalize,
    /// Run relation extraction over PubMed abstracts.
    Extract(extract::Args),
    /// Build embeddings for deduplication.
    Embed,
    /// Compute disproportionality and trend metrics.
    Signal,
    /// Rank safety signals.
    Rank,
    /// Serve the JSON API and static UI.
    Serve(serve::Args),
    /// Produce optional local summaries.
    Summarize(summarize::Args),
}

/// Operation mode for extraction.
#[derive(Clone, Debug, ValueEnum)]
pub enum ExtractMode {
    /// Weak supervision uses auto-labelled heuristics and logistic regression.
    WeaklySupervised,
    /// Skip training and use pattern-only predictions.
    PatternsOnly,
}

impl ExtractMode {
    pub fn is_training(&self) -> bool {
        matches!(self, Self::WeaklySupervised)
    }
}
