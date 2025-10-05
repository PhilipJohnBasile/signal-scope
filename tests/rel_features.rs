use rwe_assistant::nlp::features::{featurise, SentenceContext};

#[test]
fn feature_vector_has_expected_shape() {
    let ctx = SentenceContext {
        pmid: "123".into(),
        sent_idx: 0,
        drug: "imatinib".into(),
        event: "hepatotoxicity".into(),
        text: "Imatinib is associated with hepatotoxicity in rare cases.".into(),
    };
    let features = featurise(&[ctx]);
    assert_eq!(features.len(), 1);
    let feature = &features[0];
    assert!(feature.has_cue_word >= 1.0);
    assert_eq!(feature.pmid, "123");
}
