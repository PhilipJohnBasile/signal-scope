//! CLI entry-point for fetching FAERS and PubMed artefacts.

use std::time::Duration;

use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use futures::stream::{self, StreamExt};
use tokio::time::sleep;
use tracing::{info, instrument};

use crate::{
    config::Settings,
    data::{self, pubmed::PubRecord},
};

/// Args for the `fetch` sub-command.
#[derive(Debug, Clone, ClapArgs)]
pub struct Args {
    /// Comma separated list of canonical drug names.
    #[arg(long, value_delimiter = ',')]
    pub drugs: Vec<String>,
    /// FAERS quarters to download (e.g., 2024Q1).
    #[arg(long, value_delimiter = ',', default_value = "2024Q1,2024Q2")]
    pub quarters: Vec<String>,
    /// Override maximum PubMed abstracts per drug.
    #[arg(long)]
    pub max_pubmed_per_drug: Option<usize>,
}

#[instrument(skip(settings))]
pub async fn run(args: Args, settings: Settings) -> Result<()> {
    let max_pubmed = args
        .max_pubmed_per_drug
        .unwrap_or(settings.max_pubmed_per_drug);

    info!(quarters = ?args.quarters, "fetching FAERS quarters");
    let _faers_paths = data::faers::fetch_faers_quarters(&args.quarters, &settings).await?;

    let concurrency = 2usize;
    stream::iter(args.drugs.clone())
        .map(|drug| {
            let settings = settings.clone();
            async move {
                info!(%drug, "searching pubmed");
                let pmids = data::pubmed::search_pubmed(&drug, max_pubmed, &settings)
                    .await
                    .with_context(|| format!("search pubmed for {drug}"))?;
                sleep(Duration::from_millis(350)).await; // be nice to E-utilities
                let records: Vec<PubRecord> =
                    data::pubmed::fetch_pubmed(&pmids, &settings)
                        .await
                        .with_context(|| format!("fetch pubmed abstracts for {drug}"))?;
                data::pubmed::persist_records(&drug, &records, &settings)
                    .with_context(|| format!("save pubmed records for {drug}"))?;
                Ok::<_, anyhow::Error>(())
            }
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    Ok(())
}
