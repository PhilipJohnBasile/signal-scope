//! Signal computation and ranking layer.

pub mod bayes;
pub mod ror;
pub mod trend;

use std::{collections::HashMap, fs::File};

use anyhow::Result;
use polars::prelude::{
    CsvWriter, DataFrame, NamedFrom, ParquetReader, ParquetWriter, SerReader, SerWriter, Series,
};
use tracing::{info, warn};

use crate::config::Settings;

#[derive(Debug, Clone)]
struct MetricRow {
    drug_id: String,
    event_id: String,
    year_quarter: String,
    ror: f64,
    ci_low: f64,
    ci_high: f64,
    variance: f64,
    log_ror: f64,
    ror_shrunk: f64,
    shrunk_ci_low: f64,
    shrunk_ci_high: f64,
    trend_z: f64,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FaersRow {
    drug_id: String,
    event_id: String,
    year_quarter: String,
    a: i64,
    b: i64,
    c: i64,
    d: i64,
}

pub async fn compute(settings: &Settings) -> Result<()> {
    let path = settings.join_data("clean/faers_norm.parquet");
    if !path.exists() {
        warn!("faers_norm.parquet missing; run normalize first");
        return Ok(());
    }
    let df = ParquetReader::new(File::open(&path)?).finish()?;
    let drug_col = df.column("drug_id")?.str()?;
    let event_col = df.column("event_id")?.str()?;
    let quarter_col = df.column("year_quarter")?.str()?;
    let a_col = df.column("a")?.i64()?;
    let b_col = df.column("b")?.i64()?;
    let c_col = df.column("c")?.i64()?;
    let d_col = df.column("d")?.i64()?;
    let mut rows = Vec::new();
    for idx in 0..df.height() {
        if let (Some(drug), Some(event), Some(quarter), Some(a), Some(b), Some(c), Some(d)) = (
            drug_col.get(idx),
            event_col.get(idx),
            quarter_col.get(idx),
            a_col.get(idx),
            b_col.get(idx),
            c_col.get(idx),
            d_col.get(idx),
        ) {
            rows.push(FaersRow {
                drug_id: drug.to_string(),
                event_id: event.to_string(),
                year_quarter: quarter.to_string(),
                a,
                b,
                c,
                d,
            });
        }
    }
    if rows.is_empty() {
        warn!("no FAERS rows available for signal computation");
        return Ok(());
    }

    let mut metrics = Vec::new();
    let mut log_rors = Vec::new();
    for row in &rows {
        let (ror_value, ci_low, ci_high, variance) =
            ror::ror_with_ci(row.a as f64, row.b as f64, row.c as f64, row.d as f64);
        let log_ror = ror_value.ln();
        log_rors.push(log_ror);
        metrics.push(MetricRow {
            drug_id: row.drug_id.clone(),
            event_id: row.event_id.clone(),
            year_quarter: row.year_quarter.clone(),
            ror: ror_value,
            ci_low,
            ci_high,
            variance,
            log_ror,
            ror_shrunk: ror_value,
            shrunk_ci_low: ci_low,
            shrunk_ci_high: ci_high,
            trend_z: 0.0,
        });
    }

    let prior = bayes::estimate_prior(&log_rors);
    for metric in &mut metrics {
        let (shrunk, low, high) = bayes::shrink(metric.log_ror, metric.variance, prior);
        metric.ror_shrunk = shrunk;
        metric.shrunk_ci_low = low;
        metric.shrunk_ci_high = high;
    }

    apply_trend_scores(&mut metrics);
    persist_metrics(settings, &metrics)?;
    Ok(())
}

pub async fn rank(settings: &Settings) -> Result<()> {
    let metrics_path = settings.join_data("clean/signal_metrics.parquet");
    if !metrics_path.exists() {
        warn!("signal metrics parquet missing; run signal first");
        return Ok(());
    }
    let df = ParquetReader::new(File::open(&metrics_path)?).finish()?;
    let drug_col = df.column("drug_id")?.str()?;
    let event_col = df.column("event_id")?.str()?;
    let quarter_col = df.column("year_quarter")?.str()?;
    let log_col = df.column("log_ror")?.f64()?;
    let var_col = df.column("variance")?.f64()?;
    let shrunk_col = df.column("ror_shrunk")?.f64()?;
    let lo_col = df.column("shrunk_ci_low")?.f64()?;
    let hi_col = df.column("shrunk_ci_high")?.f64()?;
    let trend_col = df.column("trend_z")?.f64()?;
    let mut rows = Vec::new();
    for i in 0..df.height() {
        if let (
            Some(drug),
            Some(event),
            Some(quarter),
            Some(log_ror),
            Some(variance),
            Some(shrunk),
            Some(ci_low),
            Some(ci_high),
            Some(trend_z),
        ) = (
            drug_col.get(i),
            event_col.get(i),
            quarter_col.get(i),
            log_col.get(i),
            var_col.get(i),
            shrunk_col.get(i),
            lo_col.get(i),
            hi_col.get(i),
            trend_col.get(i),
        ) {
            rows.push((
                drug.to_string(),
                event.to_string(),
                quarter.to_string(),
                log_ror,
                variance,
                shrunk,
                ci_low,
                ci_high,
                trend_z,
            ));
        }
    }

    let mut latest: HashMap<(String, String), (String, f64, f64, f64, f64, f64, f64)> =
        HashMap::new();
    for row in rows {
        let key = (row.0.clone(), row.1.clone());
        let order = trend::parse_quarter(&row.2).unwrap_or((0, 0));
        let entry = latest
            .entry(key)
            .or_insert_with(|| (String::new(), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
        let current_order = trend::parse_quarter(&entry.0).unwrap_or((0, 0));
        if order >= current_order {
            *entry = (row.2.clone(), row.3, row.4, row.5, row.6, row.7, row.8);
        }
    }

    let lit_counts = literature_support(settings)?;

    let mut out_rows = Vec::new();
    for (key, value) in latest {
        let (quarter, log_ror, variance, shrunk, ci_low, ci_high, trend_z) = value;
        let z_recent = ror::z_score(log_ror, variance);
        let lit_support = lit_counts.get(&key).cloned().unwrap_or(0);
        let score = z_recent + 0.3 * ((lit_support + 1) as f64).ln() + 0.2 * trend_z;
        out_rows.push((
            key.0,
            key.1,
            quarter,
            shrunk,
            ci_low,
            ci_high,
            lit_support,
            trend_z,
            score,
        ));
    }

    if out_rows.is_empty() {
        warn!("no ranked rows to persist");
        return Ok(());
    }

    let mut df = DataFrame::new(vec![
        Series::new(
            "drug_id".into(),
            out_rows.iter().map(|r| r.0.clone()).collect::<Vec<_>>(),
        ),
        Series::new(
            "event_id".into(),
            out_rows.iter().map(|r| r.1.clone()).collect::<Vec<_>>(),
        ),
        Series::new(
            "year_quarter".into(),
            out_rows.iter().map(|r| r.2.clone()).collect::<Vec<_>>(),
        ),
        Series::new(
            "recent_ror".into(),
            out_rows.iter().map(|r| r.3).collect::<Vec<_>>(),
        ),
        Series::new(
            "ci_low".into(),
            out_rows.iter().map(|r| r.4).collect::<Vec<_>>(),
        ),
        Series::new(
            "ci_high".into(),
            out_rows.iter().map(|r| r.5).collect::<Vec<_>>(),
        ),
        Series::new(
            "lit_support".into(),
            out_rows.iter().map(|r| r.6 as i64).collect::<Vec<_>>(),
        ),
        Series::new(
            "trend_z".into(),
            out_rows.iter().map(|r| r.7).collect::<Vec<_>>(),
        ),
        Series::new(
            "score".into(),
            out_rows.iter().map(|r| r.8).collect::<Vec<_>>(),
        ),
    ])?;
    let out_path = settings.join_output("signals.csv");
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = File::create(&out_path)?;
    CsvWriter::new(&mut file).finish(&mut df)?;
    info!(path = %out_path.display(), rows = df.height(), "wrote ranked signals");
    Ok(())
}

fn apply_trend_scores(metrics: &mut [MetricRow]) {
    let mut grouped: HashMap<(String, String), Vec<usize>> = HashMap::new();
    for (idx, metric) in metrics.iter().enumerate() {
        grouped
            .entry((metric.drug_id.clone(), metric.event_id.clone()))
            .or_default()
            .push(idx);
    }

    for ((_drug, _event), mut indices) in grouped {
        indices
            .sort_by_key(|idx| trend::parse_quarter(&metrics[*idx].year_quarter).unwrap_or((0, 0)));
        let mut history = Vec::new();
        for idx in indices {
            let parsed = trend::parse_quarter(&metrics[idx].year_quarter).unwrap_or((0, 0));
            history.push((parsed.0, parsed.1, metrics[idx].ror_shrunk));
            metrics[idx].trend_z = trend::rolling_z(&history);
        }
    }
}

fn persist_metrics(settings: &Settings, metrics: &[MetricRow]) -> Result<()> {
    let mut df = DataFrame::new(vec![
        Series::new(
            "drug_id".into(),
            metrics
                .iter()
                .map(|m| m.drug_id.clone())
                .collect::<Vec<_>>(),
        ),
        Series::new(
            "event_id".into(),
            metrics
                .iter()
                .map(|m| m.event_id.clone())
                .collect::<Vec<_>>(),
        ),
        Series::new(
            "year_quarter".into(),
            metrics
                .iter()
                .map(|m| m.year_quarter.clone())
                .collect::<Vec<_>>(),
        ),
        Series::new(
            "ror".into(),
            metrics.iter().map(|m| m.ror).collect::<Vec<_>>(),
        ),
        Series::new(
            "ci_low".into(),
            metrics.iter().map(|m| m.ci_low).collect::<Vec<_>>(),
        ),
        Series::new(
            "ci_high".into(),
            metrics.iter().map(|m| m.ci_high).collect::<Vec<_>>(),
        ),
        Series::new(
            "variance".into(),
            metrics.iter().map(|m| m.variance).collect::<Vec<_>>(),
        ),
        Series::new(
            "log_ror".into(),
            metrics.iter().map(|m| m.log_ror).collect::<Vec<_>>(),
        ),
        Series::new(
            "ror_shrunk".into(),
            metrics.iter().map(|m| m.ror_shrunk).collect::<Vec<_>>(),
        ),
        Series::new(
            "shrunk_ci_low".into(),
            metrics.iter().map(|m| m.shrunk_ci_low).collect::<Vec<_>>(),
        ),
        Series::new(
            "shrunk_ci_high".into(),
            metrics.iter().map(|m| m.shrunk_ci_high).collect::<Vec<_>>(),
        ),
        Series::new(
            "trend_z".into(),
            metrics.iter().map(|m| m.trend_z).collect::<Vec<_>>(),
        ),
    ])?;
    let out_path = settings.join_data("clean/signal_metrics.parquet");
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(&out_path)?;
    ParquetWriter::new(file).finish(&mut df)?;
    info!(path = %out_path.display(), rows = df.height(), "wrote signal metrics");
    Ok(())
}

fn literature_support(settings: &Settings) -> Result<HashMap<(String, String), i64>> {
    let path = settings.join_data("clean/relations.parquet");
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let df = ParquetReader::new(File::open(&path)?).finish()?;
    let mut counts: HashMap<(String, String), i64> = HashMap::new();
    let drug_col = df.column("drug_id")?.str()?;
    let event_col = df.column("event_id")?.str()?;
    for (drug, event) in drug_col
        .into_no_null_iter()
        .zip(event_col.into_no_null_iter())
    {
        *counts
            .entry((drug.to_string(), event.to_string()))
            .or_insert(0) += 1;
    }
    Ok(counts)
}
