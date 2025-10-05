//! FAERS ingestion utilities.

use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use reqwest::Client;
use tracing::{info, warn};
use zip::ZipArchive;

use crate::config::Settings;

const BASE_URLS: &[&str] = &[
    "https://download-001.fda.gov/faers",
    "https://download-002.fda.gov/faers",
    "https://download-003.fda.gov/faers",
];

/// Download and cache FAERS quarterly archives, returning filtered CSV paths.
pub async fn fetch_faers_quarters(
    quarters: &[String],
    settings: &Settings,
) -> Result<Vec<PathBuf>> {
    let client = Client::builder()
        .user_agent(format!("rwe-assistant/0.1 (+{})", settings.pubmed_email))
        .gzip(true)
        .build()?;

    let dest_root = settings.join_data("raw/faers");
    std::fs::create_dir_all(&dest_root)?;

    let mut outputs = Vec::new();
    for quarter in quarters {
        let archive_name = format!("FAERS_ASCII_{quarter}.zip");
        let archive_path = dest_root.join(&archive_name);
        if !archive_path.exists() {
            download_archive(&client, quarter, &archive_path).await?;
        } else {
            info!(%quarter, "using cached faers archive");
        }

        let filtered_path = dest_root.join(format!("faers_{quarter}.csv"));
        if !filtered_path.exists() {
            info!(%quarter, "filtering faers archive");
            filter_archive(&archive_path, quarter, &filtered_path)?;
        }
        outputs.push(filtered_path);
    }

    Ok(outputs)
}

async fn download_archive(client: &Client, quarter: &str, dest: &Path) -> Result<()> {
    for base in BASE_URLS {
        let url = format!("{base}/FAERS_ASCII_{quarter}.zip");
        info!(%url, "attempting FAERS download");
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let bytes = resp.bytes().await?;
                let mut file = File::create(dest).with_context(|| format!("create {dest:?}"))?;
                file.write_all(&bytes)?;
                info!(?dest, size = bytes.len(), "downloaded faers archive");
                return Ok(());
            }
            Ok(resp) => {
                warn!(status = %resp.status(), "failed url, trying next mirror");
            }
            Err(err) => warn!(%err, "download error, next mirror"),
        }
    }
    Err(anyhow!("unable to download FAERS archive for {quarter}"))
}

fn filter_archive(archive_path: &Path, quarter: &str, dest_csv: &Path) -> Result<()> {
    let file =
        File::open(archive_path).with_context(|| format!("open archive {archive_path:?}"))?;
    let mut archive = ZipArchive::new(file)?;

    let mut drug_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut reaction_map: HashMap<String, Vec<String>> = HashMap::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_ascii_uppercase();
        if !name.ends_with(".TXT") {
            continue;
        }
        if name.contains("DRUG") {
            info!(file = %entry.name(), "processing drug file");
            let text = read_to_string(&mut entry)?;
            parse_table(&text, "CASEID", "DRUGNAME", &mut drug_map)?;
        } else if name.contains("REAC") {
            info!(file = %entry.name(), "processing reaction file");
            let text = read_to_string(&mut entry)?;
            parse_table(&text, "CASEID", "PT", &mut reaction_map)?;
        }
    }

    let mut writer = csv::Writer::from_path(dest_csv)?;
    writer.write_record(["CASEID", "DRUGNAME", "PT", "YEAR_QUARTER"])?;

    let mut count = 0u64;
    for (case, drugs) in &drug_map {
        if let Some(events) = reaction_map.get(case) {
            for drug in drugs {
                for event in events {
                    writer.write_record([case, drug, event, quarter])?;
                    count += 1;
                }
            }
        }
    }
    writer.flush()?;
    info!(rows = count, path = %dest_csv.display(), "wrote filtered FAERS file");
    Ok(())
}

fn read_to_string(entry: &mut zip::read::ZipFile<'_>) -> Result<String> {
    let mut buf = String::new();
    entry.read_to_string(&mut buf)?;
    Ok(buf)
}

fn parse_table(
    text: &str,
    case_header: &str,
    value_header: &str,
    sink: &mut HashMap<String, Vec<String>>,
) -> Result<()> {
    let mut lines = text.lines();
    let header_line = lines.next().ok_or_else(|| anyhow!("missing header"))?;
    let delimiter = if header_line.contains('|') {
        '|'
    } else if header_line.contains('$') {
        '$'
    } else if header_line.contains('\t') {
        '\t'
    } else {
        ','
    };
    let headers: Vec<&str> = header_line.split(delimiter).collect();
    let case_idx = headers
        .iter()
        .position(|h| h.eq_ignore_ascii_case(case_header))
        .ok_or_else(|| anyhow!("missing {case_header}"))?;
    let value_idx = headers
        .iter()
        .position(|h| h.eq_ignore_ascii_case(value_header))
        .ok_or_else(|| anyhow!("missing {value_header}"))?;

    for line in lines {
        let cols: Vec<&str> = line.split(delimiter).collect();
        if cols.len() <= case_idx || cols.len() <= value_idx {
            continue;
        }
        let case = cols[case_idx].trim();
        let value = cols[value_idx].trim();
        if case.is_empty() || value.is_empty() {
            continue;
        }
        sink.entry(case.to_string())
            .or_default()
            .push(value.to_string());
    }

    Ok(())
}

/// Helper to stamp the data refresh time.
pub fn utc_timestamp_string() -> String {
    Utc::now().to_rfc3339()
}
