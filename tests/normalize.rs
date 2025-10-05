use rwe_assistant::data::normalize;

#[test]
fn gleevec_maps_to_imatinib() {
    let canonical = normalize::seed_lookup("Gleevec").unwrap();
    assert_eq!(canonical, "imatinib");
}
