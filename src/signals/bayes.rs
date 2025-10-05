//! Empirical Bayes shrinkage for reporting odds ratios.

/// Prior parameters estimated from the corpus.
#[derive(Debug, Clone, Copy)]
pub struct Prior {
    pub mean: f64,
    pub var: f64,
}

impl Prior {
    pub fn default() -> Self {
        Self {
            mean: 0.0,
            var: 0.25,
        }
    }
}

/// Estimate a Gaussian prior from observed log RORs.
pub fn estimate_prior(samples: &[f64]) -> Prior {
    if samples.is_empty() {
        return Prior::default();
    }
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let var = samples
        .iter()
        .map(|value| {
            let centered = value - mean;
            centered * centered
        })
        .sum::<f64>()
        / samples.len().max(1) as f64;
    Prior {
        mean,
        var: var.max(1e-6),
    }
}

/// Apply shrinkage to a single log ROR.
pub fn shrink(log_ror: f64, variance: f64, prior: Prior) -> (f64, f64, f64) {
    let weight = prior.var / (prior.var + variance);
    let shrunk_log = weight * log_ror + (1.0 - weight) * prior.mean;
    let shrunk_var = (variance * prior.var) / (variance + prior.var);
    let se = shrunk_var.sqrt();
    let ci_low = (shrunk_log - 1.96 * se).exp();
    let ci_high = (shrunk_log + 1.96 * se).exp();
    (shrunk_log.exp(), ci_low, ci_high)
}
