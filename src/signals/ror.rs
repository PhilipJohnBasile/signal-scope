//! Reporting odds ratio computations.

/// Compute the reporting odds ratio with 95% confidence interval.
pub fn ror_with_ci(a: f64, b: f64, c: f64, d: f64) -> (f64, f64, f64, f64) {
    let (a, b, c, d) = continuity_correct(a, b, c, d);
    let r1 = a / b;
    let r2 = c / d;
    let ror = r1 / r2;
    let log_ror = ror.ln();
    let variance = (1.0 / a) + (1.0 / b) + (1.0 / c) + (1.0 / d);
    let se = variance.sqrt();
    let ci_low = (log_ror - 1.96 * se).exp();
    let ci_high = (log_ror + 1.96 * se).exp();
    (ror, ci_low, ci_high, variance)
}

fn continuity_correct(a: f64, b: f64, c: f64, d: f64) -> (f64, f64, f64, f64) {
    if [a, b, c, d].iter().any(|&x| x == 0.0) {
        (a + 0.5, b + 0.5, c + 0.5, d + 0.5)
    } else {
        (a, b, c, d)
    }
}

/// Convert log ROR and variance to a z-score.
pub fn z_score(log_ror: f64, variance: f64) -> f64 {
    if variance <= 0.0 {
        0.0
    } else {
        log_ror / variance.sqrt()
    }
}
