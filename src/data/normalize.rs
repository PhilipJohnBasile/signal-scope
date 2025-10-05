//! Terminology normalisation and contingency table construction.

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    path::PathBuf,
};

use anyhow::Result;
use indexmap::IndexMap;
use polars::prelude::{DataFrame, NamedFrom, ParquetWriter, Series};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use strsim::jaro_winkler;
use tracing::info;

use crate::config::Settings;

const DRUG_SEED_MAP: &[(&str, &str)] = &[
    ("GLEEVEC", "imatinib"),
    ("IMATINIB", "imatinib"),
    ("SPRYCEL", "dasatinib"),
    ("DASATINIB", "dasatinib"),
    ("TASIGNA", "nilotinib"),
    ("NILOTINIB", "nilotinib"),
    ("OPDIVO", "nivolumab"),
    ("NIVOLUMAB", "nivolumab"),
    ("KEYTRUDA", "pembrolizumab"),
    ("PEMBROLIZUMAB", "pembrolizumab"),
    ("YERVOY", "ipilimumab"),
    ("IPILIMUMAB", "ipilimumab"),
];

const SIDER_TERMS: &[&str] = &[
    "hepatotoxicity",
    "rash",
    "diarrhoea",
    "neutropenia",
    "fatigue",
    "nausea",
    "fever",
    "cardiotoxicity",
    "anemia",
    "thrombocytopenia",
    "headache",
];

/// Public helper for integration tests to assert seed mappings.
pub fn seed_lookup(name: &str) -> Option<&'static str> {
    let key = name.trim().to_ascii_uppercase();
    DRUG_SEED_MAP
        .iter()
        .find(|(raw, _)| raw.trim().eq_ignore_ascii_case(key.as_str()))
        .map(|(_, canon)| *canon)
}

#[derive(Debug, Deserialize)]
struct FaersRawRow {
    #[serde(rename = "CASEID")]
    caseid: String,
    #[serde(rename = "DRUGNAME")]
    drugname: String,
    #[serde(rename = "PT")]
    event: String,
    #[serde(rename = "YEAR_QUARTER")]
    quarter: String,
}

#[derive(Debug, Serialize)]
struct DrugRow {
    drug_id: String,
    name_canonical: String,
}

#[derive(Debug, Serialize)]
struct EventRow {
    event_id: String,
    term_canonical: String,
}

#[derive(Debug, Serialize)]
struct FaersNormRow {
    drug_id: String,
    event_id: String,
    year_quarter: String,
    a: i64,
    b: i64,
    c: i64,
    d: i64,
}

pub async fn canonicalise(settings: &Settings) -> Result<()> {
    let raw_rows = load_faers_rows(settings)?;
    if raw_rows.is_empty() {
        info!("no FAERS rows found; normalization is a no-op");
        return Ok(());
    }

    let client = Client::builder().user_agent("rwe-assistant/0.1").build()?;

    let unique_drugs = collect_unique(raw_rows.iter().map(|r| r.drugname.clone()));
    let unique_events = collect_unique(raw_rows.iter().map(|r| r.event.clone()));

    let drug_map = build_drug_map(&unique_drugs, &client).await;
    let event_map = build_event_map(&unique_events);

    let (drug_rows, drug_lookup) = materialise_drugs(&drug_map);
    let (event_rows, event_lookup) = materialise_events(&event_map);

    write_drugs(&drug_rows, settings.join_data("clean/drugs.parquet"))?;
    write_events(&event_rows, settings.join_data("clean/events.parquet"))?;

    let norm_rows = build_contingency(&raw_rows, &drug_lookup, &event_lookup);
    write_norm(&norm_rows, settings.join_data("clean/faers_norm.parquet"))?;
    Ok(())
}

fn load_faers_rows(settings: &Settings) -> Result<Vec<FaersRawRow>> {
    let mut rows = Vec::new();
    let root = settings.join_data("raw/faers");
    if !root.exists() {
        return Ok(rows);
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if entry.path().extension().and_then(|s| s.to_str()) != Some("csv") {
            continue;
        }
        let mut reader = csv::Reader::from_path(entry.path())?;
        for result in reader.deserialize() {
            let row: FaersRawRow = result?;
            rows.push(row);
        }
    }
    info!(rows = rows.len(), "loaded faers raw rows");
    Ok(rows)
}

fn collect_unique<I>(iter: I) -> Vec<String>
where
    I: Iterator<Item = String>,
{
    let mut set = IndexMap::<String, ()>::new();
    for value in iter {
        let key = value.trim().to_ascii_uppercase();
        set.entry(key).or_insert(());
    }
    set.into_keys().collect()
}

async fn build_drug_map(names: &[String], client: &Client) -> HashMap<String, String> {
    let seed: HashMap<_, _> = DRUG_SEED_MAP
        .iter()
        .map(|(raw, canon)| ((*raw).to_string(), (*canon).to_string()))
        .collect();
    let mut mapping = HashMap::new();
    for name in names {
        let seed_key = name.trim().to_ascii_uppercase();
        if let Some(canon) = seed.get(&seed_key) {
            mapping.insert(name.clone(), canon.clone());
            continue;
        }
        if let Some(rx) = rxnorm_lookup(name, client).await {
            mapping.insert(name.clone(), rx);
        } else {
            mapping.insert(name.clone(), name.to_lowercase());
        }
    }
    mapping
}

async fn rxnorm_lookup(name: &str, client: &Client) -> Option<String> {
    let url = format!(
        "https://rxnav.nlm.nih.gov/REST/drugs.json?name={}",
        urlencoding::encode(name)
    );
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let payload: serde_json::Value = resp.json().await.ok()?;
    payload
        .pointer("/drugGroup/conceptGroup/0/conceptProperties/0/name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_lowercase())
}

fn build_event_map(names: &[String]) -> HashMap<String, String> {
    let dictionary: Vec<String> = SIDER_TERMS.iter().map(|s| s.to_string()).collect();
    let mut mapping = HashMap::new();
    for name in names {
        let target = name.trim().to_lowercase();
        let mut best = (0.0f64, target.clone());
        for candidate in &dictionary {
            let score = jaro_winkler(&target, candidate);
            if score > best.0 {
                best = (score, candidate.clone());
            }
        }
        let canonical = if best.0 > 0.82 { best.1 } else { target };
        mapping.insert(name.clone(), canonical);
    }
    mapping
}

fn materialise_drugs(map: &HashMap<String, String>) -> (Vec<DrugRow>, HashMap<String, String>) {
    let mut canonical = IndexMap::new();
    for value in map.values() {
        if !canonical.contains_key(value) {
            let id = format!("D{:04}", canonical.len() + 1);
            canonical.insert(value.clone(), id);
        }
    }
    let mut rows = Vec::new();
    for (name, id) in &canonical {
        rows.push(DrugRow {
            drug_id: id.clone(),
            name_canonical: name.clone(),
        });
    }
    let mut lookup = HashMap::new();
    for (raw, canon) in map {
        if let Some(id) = canonical.get(canon) {
            lookup.insert(raw.clone(), id.clone());
        }
    }
    (rows, lookup)
}

fn materialise_events(map: &HashMap<String, String>) -> (Vec<EventRow>, HashMap<String, String>) {
    let mut canonical = IndexMap::new();
    for value in map.values() {
        if !canonical.contains_key(value) {
            let id = format!("E{:04}", canonical.len() + 1);
            canonical.insert(value.clone(), id);
        }
    }
    let mut rows = Vec::new();
    for (name, id) in &canonical {
        rows.push(EventRow {
            event_id: id.clone(),
            term_canonical: name.clone(),
        });
    }
    let mut lookup = HashMap::new();
    for (raw, canon) in map {
        if let Some(id) = canonical.get(canon) {
            lookup.insert(raw.clone(), id.clone());
        }
    }
    (rows, lookup)
}

fn build_contingency(
    rows: &[FaersRawRow],
    drug_lookup: &HashMap<String, String>,
    event_lookup: &HashMap<String, String>,
) -> Vec<FaersNormRow> {
    #[derive(Default)]
    struct CaseSummary {
        drugs: HashSet<String>,
        events: HashSet<String>,
    }

    let mut quarters: HashMap<String, HashMap<String, CaseSummary>> = HashMap::new();

    for row in rows {
        let case_entry = quarters
            .entry(row.quarter.clone())
            .or_default()
            .entry(row.caseid.clone())
            .or_default();
        if let Some(drug_id) = drug_lookup.get(&row.drugname.trim().to_ascii_uppercase()) {
            case_entry.drugs.insert(drug_id.clone());
        }
        if let Some(event_id) = event_lookup.get(&row.event.trim().to_ascii_uppercase()) {
            case_entry.events.insert(event_id.clone());
        }
    }

    let mut results = Vec::new();
    for (quarter, cases) in quarters {
        let case_values: Vec<_> = cases.values().collect();
        let total_cases = case_values.len() as i64;
        let mut drug_totals: HashMap<String, i64> = HashMap::new();
        let mut event_totals: HashMap<String, i64> = HashMap::new();
        let mut co_counts: HashMap<(String, String), i64> = HashMap::new();

        for summary in &case_values {
            for drug in &summary.drugs {
                *drug_totals.entry(drug.clone()).or_insert(0) += 1;
            }
            for event in &summary.events {
                *event_totals.entry(event.clone()).or_insert(0) += 1;
            }
            for drug in &summary.drugs {
                for event in &summary.events {
                    *co_counts.entry((drug.clone(), event.clone())).or_insert(0) += 1;
                }
            }
        }

        for ((drug, event), a) in co_counts.iter() {
            let b = drug_totals.get(drug).cloned().unwrap_or(0) - a;
            let c = event_totals.get(event).cloned().unwrap_or(0) - a;
            let d = total_cases - (a + b + c);
            results.push(FaersNormRow {
                drug_id: drug.clone(),
                event_id: event.clone(),
                year_quarter: quarter.clone(),
                a: *a,
                b,
                c,
                d,
            });
        }
    }

    results
}

fn write_drugs(rows: &[DrugRow], path: PathBuf) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let ids: Vec<String> = rows.iter().map(|r| r.drug_id.clone()).collect();
    let names: Vec<String> = rows.iter().map(|r| r.name_canonical.clone()).collect();
    let mut df = DataFrame::new(vec![
        Series::new("drug_id".into(), ids),
        Series::new("name_canonical".into(), names),
    ])?;
    let file = File::create(&path)?;
    ParquetWriter::new(file).finish(&mut df)?;
    info!(path = %path.display(), rows = rows.len(), "wrote drugs parquet");
    Ok(())
}

fn write_events(rows: &[EventRow], path: PathBuf) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let ids: Vec<String> = rows.iter().map(|r| r.event_id.clone()).collect();
    let names: Vec<String> = rows.iter().map(|r| r.term_canonical.clone()).collect();
    let mut df = DataFrame::new(vec![
        Series::new("event_id".into(), ids),
        Series::new("term_canonical".into(), names),
    ])?;
    let file = File::create(&path)?;
    ParquetWriter::new(file).finish(&mut df)?;
    info!(path = %path.display(), rows = rows.len(), "wrote events parquet");
    Ok(())
}

fn write_norm(rows: &[FaersNormRow], path: PathBuf) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let drug_ids: Vec<String> = rows.iter().map(|r| r.drug_id.clone()).collect();
    let event_ids: Vec<String> = rows.iter().map(|r| r.event_id.clone()).collect();
    let quarters: Vec<String> = rows.iter().map(|r| r.year_quarter.clone()).collect();
    let a: Vec<i64> = rows.iter().map(|r| r.a).collect();
    let b: Vec<i64> = rows.iter().map(|r| r.b).collect();
    let c: Vec<i64> = rows.iter().map(|r| r.c).collect();
    let d: Vec<i64> = rows.iter().map(|r| r.d).collect();
    let mut df = DataFrame::new(vec![
        Series::new("drug_id".into(), drug_ids),
        Series::new("event_id".into(), event_ids),
        Series::new("year_quarter".into(), quarters),
        Series::new("a".into(), a),
        Series::new("b".into(), b),
        Series::new("c".into(), c),
        Series::new("d".into(), d),
    ])?;
    let file = File::create(&path)?;
    ParquetWriter::new(file).finish(&mut df)?;
    info!(path = %path.display(), rows = rows.len(), "wrote faers_norm parquet");
    Ok(())
}
