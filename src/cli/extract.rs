//! CLI entry-point for relation extraction.

use anyhow::Result;
use clap::Args as ClapArgs;
use tracing::instrument;

use crate::{cli::ExtractMode, config::Settings, nlp};

/// Args for the `extract` command.
#[derive(Debug, Clone, ClapArgs)]
pub struct Args {
    /// Extraction strategy.
    #[arg(long, default_value = "weakly-supervised", value_enum)]
    pub mode: ExtractMode,
}

#[instrument(skip(settings))]
pub async fn run(args: Args, settings: Settings) -> Result<()> {
    nlp::extract_relations(&settings, args.mode).await
}
