//! CLI entry-point for generating optional local summaries.

use anyhow::Result;
use clap::Args as ClapArgs;
use tracing::{info, instrument};

use crate::{config::Settings, nlp};

/// Args for the `summarize` command.
#[derive(Debug, Clone, ClapArgs)]
pub struct Args {
    /// Canonical drug name.
    #[arg(long)]
    pub drug: String,
    /// Canonical adverse event term.
    #[arg(long)]
    pub event: String,
    /// Top relations to include in the prompt.
    #[arg(long, default_value_t = 5)]
    pub topk: usize,
}

#[instrument(skip(settings))]
pub async fn run(args: Args, settings: Settings) -> Result<()> {
    let summary = nlp::summarize(&settings, &args.drug, &args.event, args.topk).await?;
    info!(%summary, "generated summary");
    println!("{summary}");
    Ok(())
}
