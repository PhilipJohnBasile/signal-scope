use rwe_assistant::nlp::embeddings::cluster_preview;

#[cfg(feature = "embeddings")]
#[test]
fn near_duplicates_cluster_together() {
    let embeddings = vec![
        vec![1.0, 0.0, 0.0],
        vec![0.99, 0.01, 0.0],
        vec![0.0, 1.0, 0.0],
    ];
    let clusters = cluster_preview(&embeddings, 0.85);
    assert_eq!(clusters[0], clusters[1]);
    assert_ne!(clusters[0], clusters[2]);
}

#[cfg(not(feature = "embeddings"))]
#[test]
fn embeddings_stub_returns_identity() {
    let embeddings = vec![vec![1.0], vec![1.0]];
    let clusters = cluster_preview(&embeddings, 0.85);
    assert_eq!(clusters, vec![0, 1]);
}
