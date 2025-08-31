use surreal_mind::kg_extractor::HeuristicExtractor;

#[tokio::test]
async fn extractor_filters_common_stopwords() {
    let extractor = HeuristicExtractor::new();
    let text = "These are notes. Let's plan quickly.".to_string();
    let result = extractor.extract(&[text]).await.unwrap();
    let names: Vec<String> = result.entities.into_iter().map(|e| e.name).collect();
    assert!(
        !names.iter().any(|n| n == "These" || n == "Let"),
        "Stopwords should not be extracted as entities: {:?}",
        names
    );
}

#[tokio::test]
async fn extractor_keeps_clear_entities() {
    let extractor = HeuristicExtractor::new();
    let text = "SurrealDB integrates with Rust.".to_string();
    let result = extractor.extract(&[text]).await.unwrap();
    let names: Vec<String> = result.entities.into_iter().map(|e| e.name).collect();
    assert!(names.iter().any(|n| n == "SurrealDB"));
    assert!(names.iter().any(|n| n == "Rust"));
}

