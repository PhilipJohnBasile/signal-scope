//! Simple quarterly trend scores based on rolling z-statistics.

/// Compute a rolling z-score using all historical quarters up to the latest value.
pub fn rolling_z(values: &[(i32, u8, f64)]) -> f64 {
    if values.len() < 3 {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by_key(|(year, quarter, _)| (*year, *quarter));
    let mut rors = Vec::new();
    for (_, _, value) in &sorted {
        rors.push(*value);
    }
    if rors.len() < 2 {
        return 0.0;
    }
    let mean = rors[..rors.len() - 1].iter().sum::<f64>() / (rors.len() - 1) as f64;
    let variance = rors[..rors.len() - 1]
        .iter()
        .map(|v| {
            let diff = v - mean;
            diff * diff
        })
        .sum::<f64>()
        / (rors.len() - 1).max(1) as f64;
    if variance <= 1e-9 {
        return 0.0;
    }
    let latest = rors.last().copied().unwrap_or(0.0);
    (latest - mean) / variance.sqrt()
}

/// Convert a quarter string like 2024Q1 into sortable tuple.
pub fn parse_quarter(quarter: &str) -> Option<(i32, u8)> {
    if quarter.len() != 6 {
        return None;
    }
    let year: i32 = quarter[0..4].parse().ok()?;
    let q: u8 = quarter[5..6].parse().ok()?;
    Some((year, q))
}
