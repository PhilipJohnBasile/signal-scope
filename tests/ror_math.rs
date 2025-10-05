use rwe_assistant::signals::ror;

#[test]
fn ror_matches_reference() {
    let (ror_value, ci_low, ci_high, variance) = ror::ror_with_ci(12.0, 30.0, 8.0, 90.0);
    assert!((ror_value - 4.5).abs() < 0.5);
    assert!(ci_low < ror_value);
    assert!(ci_high > ror_value);
    assert!(variance > 0.0);
}
