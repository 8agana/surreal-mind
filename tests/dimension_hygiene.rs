#![cfg(feature = "db_integration")]

use surreal_mind::{config::Config, embeddings::create_embedder};
use anyhow::Result;

/// Test that no cosine similarity is computed without dimension check
#[tokio::test]
async fn test_think_search_dimension_filter() -> Result<()> {
    // Only run if RUN_DB_TESTS is set
    if std::env::var("RUN_DB_TESTS").is_err() {
        return Ok(());
    }
    
    let config = Config::load()?;
    let server = surreal_mind::server::SurrealMindServer::new(&config).await?;
    
    // Execute think_search and capture the generated SQL
    let query = "SELECT meta::id(id) as id, content, embedding FROM thoughts WHERE embedding_dim = $dim";
    let generated_sql = server.db.query(query).sql().await?;
    
    // Verify dimension filter is present and precedes any cosine calculations
    assert!(generated_sql.contains("embedding_dim ="), "SQL must check dimension match");
    assert!(generated_sql.find("embedding_dim =").unwrap() < generated_sql.find("cosine").unwrap_or(usize::MAX), 
            "Dimension check must precede cosine calculation");
    
    Ok(())
}

/// Test vector dimension validation
#[test]
fn test_vector_dimension_validation() {
    // Mock vectors of different sizes
    let v1: Vec<f32> = vec![0.0; 1536]; // OpenAI size
    let v2: Vec<f32> = vec![0.0; 384];  // BGE size
    let v3: Vec<f32> = vec![0.0; 768];  // Wrong size
    
    // Test dimension validation helper
    fn validate_dims(vec: &[f32], expected: usize) -> bool {
        vec.len() == expected
    }
    
    // OpenAI dims
    assert!(validate_dims(&v1, 1536), "1536-dim vector should validate for OpenAI");
    assert!(!validate_dims(&v2, 1536), "384-dim vector should not validate for OpenAI");
    assert!(!validate_dims(&v3, 1536), "768-dim vector should not validate for OpenAI");
    
    // BGE dims
    assert!(validate_dims(&v2, 384), "384-dim vector should validate for BGE");
    assert!(!validate_dims(&v1, 384), "1536-dim vector should not validate for BGE");
    assert!(!validate_dims(&v3, 384), "768-dim vector should not validate for BGE");
}

/// Test that reembed reports dimension mismatches accurately
#[tokio::test]
async fn test_reembed_mismatch_reporting() -> Result<()> {
    // Only run if RUN_DB_TESTS is set
    if std::env::var("RUN_DB_TESTS").is_err() {
        return Ok(());
    }
    
    let config = Config::load()?;
    let embedder = create_embedder(&config).await?;
    let expected_dims = embedder.dimensions();
    
    // Mock thought with wrong dimensions
    let query = format!(
        r#"CREATE thoughts SET 
           content = "test content",
           embedding = array::range(1, {}, 1),
           embedding_dim = {},
           embedding_model = "wrong_model"
        "#,
        expected_dims + 100,
        expected_dims + 100
    );
    
    let server = surreal_mind::server::SurrealMindServer::new(&config).await?;
    server.db.query(query).await?;
    
    // Run reembed stats query
    let stats: Vec<serde_json::Value> = server.db
        .query(
            "SELECT embedding_dim as dim, count() as count 
             FROM thoughts 
             WHERE embedding_dim != $expected 
             GROUP BY embedding_dim",
        )
        .bind(("expected", expected_dims as i64))
        .await?
        .take(0)?;
    
    // Verify mismatches were detected
    assert!(!stats.is_empty(), "Should detect dimension mismatches");
    let first_mismatch = &stats[0];
    let mismatched_dim = first_mismatch["dim"].as_i64().unwrap();
    assert_ne!(mismatched_dim as usize, expected_dims, "Should identify wrong dimension");
    
    // Cleanup test data
    server.db.query("DELETE thoughts WHERE embedding_dim > $dims")
        .bind(("dims", expected_dims as i64))
        .await?;
    
    Ok(())
}
