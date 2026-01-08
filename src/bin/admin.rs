//! Admin utility for SurrealMind database inspection and maintenance.
//!
//! This consolidates several debug/admin binaries into a single CLI with subcommands.
//!
//! Usage:
//!   cargo run --bin admin -- inspect
//!   cargo run --bin admin -- sanity-cosine
//!   cargo run --bin admin -- db-check
//!   cargo run --bin admin -- check-contents
//!   cargo run --bin admin -- simple-test
//!   cargo run --bin admin -- fix-dims

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "admin")]
#[command(about = "SurrealMind admin utilities", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Inspect KG entities and observations for embedding metadata
    Inspect,
    /// Run sanity check comparing freshly computed vs stored embeddings
    SanityCosine,
    /// Comprehensive database connectivity and content check
    DbCheck,
    /// Quick check of database contents and counts
    CheckContents,
    /// Simple database connectivity test
    SimpleTest,
    /// Fix embedding dimension mismatches by re-embedding
    FixDims,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Inspect => inspect().await,
        Commands::SanityCosine => sanity_cosine().await,
        Commands::DbCheck => db_check().await,
        Commands::CheckContents => check_contents().await,
        Commands::SimpleTest => simple_test().await,
        Commands::FixDims => fix_dims().await,
    }
}

/// Inspect KG entities and observations for embedding metadata
async fn inspect() -> Result<()> {
    use serde_json::Value;
    use surreal_mind::embeddings::create_embedder;
    use surrealdb::Surreal;
    use surrealdb::engine::remote::ws::Ws;
    use surrealdb::opt::auth::Root;

    let _ = dotenvy::dotenv();

    // Load configuration
    let config = surreal_mind::config::Config::load().map_err(|e| {
        eprintln!("Failed to load configuration: {}", e);
        e
    })?;

    // Embedder (same policy as core)
    let embedder = create_embedder(&config).await?;
    let dims = embedder.dimensions();
    println!("✅ Embedder ready (dims={})", dims);

    // DB connection
    let url = config.system.database_url.clone();
    let user = config.runtime.database_user.clone();
    let pass = config.runtime.database_pass.clone();
    let ns = config.system.database_ns.clone();
    let dbname = config.system.database_db.clone();
    let db = Surreal::new::<Ws>(&url).await?;
    db.signin(Root {
        username: &user,
        password: &pass,
    })
    .await?;
    db.use_ns(&ns).use_db(&dbname).await?;

    // Inspect the seeded records
    let seeded_records = vec![
        ("kg_entities", "0ujxrmhj3powwi300bz3"),
        ("kg_observations", "kbwqbrkedqw0ux2zsysy"),
    ];

    for (table, id) in seeded_records {
        println!("\n🔍 Inspecting {}:{}", table, id);

        let sql = format!(
            "SELECT meta::id(id) as id, name, data, embedding, embedding_dim, embedding_provider, embedding_model, (IF type::is::array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len, created_at FROM {} WHERE meta::id(id) = '{}'",
            table, id
        );

        let rows: Vec<Value> = db.query(sql).await?.take(0)?;

        if rows.is_empty() {
            println!("  ❌ Record not found");
            continue;
        }

        let r = &rows[0];
        let name = r.get("name").and_then(|v| v.as_str()).unwrap_or("N/A");
        let emb_len = r.get("emb_len").and_then(|v| v.as_u64()).unwrap_or(0);
        let dim = r.get("embedding_dim").and_then(|v| v.as_u64()).unwrap_or(0);
        let provider = r
            .get("embedding_provider")
            .and_then(|v| v.as_str())
            .unwrap_or("N/A");
        let model = r
            .get("embedding_model")
            .and_then(|v| v.as_str())
            .unwrap_or("N/A");
        let has_emb = r.get("embedding").is_some();
        let created_at = r
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or("N/A");

        println!("  📝 Name: {}", name);
        println!("  📊 Embedding length: {}", emb_len);
        println!("  🎯 Recorded dim: {}", dim);
        println!("  🏢 Provider: {}", provider);
        println!("  🤖 Model: {}", model);
        println!("  ✅ Has embedding: {}", has_emb);
        println!("  🕒 Created: {}", created_at);

        // Check if embedding matches expected dimensions
        if emb_len > 0 && emb_len as usize != dims {
            println!(
                "  ⚠️  Embedding dim mismatch! Expected {}, got {}",
                dims, emb_len
            );
        }
    }

    Ok(())
}

/// Run sanity check comparing freshly computed vs stored embeddings
async fn sanity_cosine() -> Result<()> {
    use surreal_mind::embeddings::create_embedder;
    use surrealdb::Surreal;
    use surrealdb::engine::remote::ws::Ws;
    use surrealdb::opt::auth::Root;

    fn cosine(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let mut dot = 0.0;
        let mut na = 0.0;
        let mut nb = 0.0;
        for i in 0..a.len() {
            dot += a[i] * b[i];
            na += a[i] * a[i];
            nb += b[i] * b[i];
        }
        if na == 0.0 || nb == 0.0 {
            0.0
        } else {
            dot / (na.sqrt() * nb.sqrt())
        }
    }

    dotenvy::dotenv().ok();
    // Load config to get embedder settings
    let config = surreal_mind::config::Config::load()?;

    let db =
        Surreal::new::<Ws>(std::env::var("SURR_DB_URL").unwrap_or("127.0.0.1:8000".into())).await?;
    db.signin(Root {
        username: &std::env::var("SURR_DB_USER").unwrap_or("root".into()),
        password: &std::env::var("SURR_DB_PASS").unwrap_or("root".into()),
    })
    .await?;
    db.use_ns(std::env::var("SURR_DB_NS").unwrap_or("surreal_mind".into()))
        .use_db(std::env::var("SURR_DB_DB").unwrap_or("consciousness".into()))
        .await?;

    let rows: Vec<serde_json::Value> = db
        .query("SELECT meta::id(id) as id, content FROM thoughts LIMIT 1")
        .await?
        .take(0)?;
    let (id, content) = {
        let r = rows.first().expect("no thoughts");
        (
            r.get("id").and_then(|v| v.as_str()).unwrap().to_string(),
            r.get("content")
                .and_then(|v| v.as_str())
                .unwrap()
                .to_string(),
        )
    };
    println!(
        "Using thought {}: {}",
        id,
        content.chars().take(60).collect::<String>()
    );

    let embedder = create_embedder(&config).await?;
    let q: Vec<f32> = embedder.embed(&content).await?;
    let stored: Vec<serde_json::Value> = db
        .query("SELECT embedding FROM thoughts WHERE id = type::thing('thoughts', $id) LIMIT 1")
        .bind(("id", id.clone()))
        .await?
        .take(0)?;
    let emb = stored[0]["embedding"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap() as f32)
        .collect::<Vec<_>>();
    println!("dims: query={}, stored={}", q.len(), emb.len());
    println!("cosine: {:.4}", cosine(&q, &emb));
    Ok(())
}

/// Comprehensive database connectivity and content check
async fn db_check() -> Result<()> {
    use surrealdb::Surreal;
    use surrealdb::engine::remote::ws::Ws;
    use surrealdb::opt::auth::Root;

    // Load environment variables
    dotenvy::dotenv().ok();

    let url = std::env::var("SURR_DB_URL").unwrap_or_else(|_| "127.0.0.1:8000".to_string());
    let user = std::env::var("SURR_DB_USER").unwrap_or_else(|_| "root".to_string());
    let pass = std::env::var("SURR_DB_PASS").unwrap_or_else(|_| "root".to_string());
    let ns = std::env::var("SURR_DB_NS").unwrap_or_else(|_| "surreal_mind".to_string());
    let dbname = std::env::var("SURR_DB_DB").unwrap_or_else(|_| "consciousness".to_string());

    println!("Connecting to SurrealDB at {}...", url);

    // Connect to the running SurrealDB service
    let db = Surreal::new::<Ws>(url).await?;

    // Authenticate
    db.signin(Root {
        username: &user,
        password: &pass,
    })
    .await?;

    // Select namespace and database
    db.use_ns(&ns).use_db(&dbname).await?;

    println!(
        "Connected successfully to namespace '{}' and database '{}'",
        ns, dbname
    );

    // Check if thoughts table exists and count records
    let count_result: Vec<serde_json::Value> = db
        .query("SELECT count() FROM thoughts GROUP ALL")
        .await?
        .take(0)?;

    println!("Thoughts count: {:?}", count_result);

    // Check if thoughts have embeddings by counting those with non-empty embeddings
    let thoughts_with_embeddings: Vec<serde_json::Value> = db
        .query("SELECT count() FROM thoughts WHERE array::len(embedding) > 0 GROUP ALL")
        .await?
        .take(0)?;

    println!("Thoughts with embeddings: {:?}", thoughts_with_embeddings);

    // Check thoughts without embeddings
    let thoughts_without_embeddings: Vec<serde_json::Value> = db
        .query("SELECT count() FROM thoughts WHERE array::len(embedding) = 0 GROUP ALL")
        .await?
        .take(0)?;

    println!(
        "Thoughts without embeddings: {:?}",
        thoughts_without_embeddings
    );

    // Simple query to check thought IDs only
    let thought_ids: Vec<serde_json::Value> =
        db.query("SELECT id FROM thoughts LIMIT 5").await?.take(0)?;

    println!("Sample thought IDs:");
    for thought in thought_ids {
        if let Some(id) = thought.get("id").and_then(|v| v.as_str()) {
            println!("ID: {}", id);
        }
    }

    // Check if recalls table exists
    let recalls_count_result: Vec<serde_json::Value> = db
        .query("SELECT count() FROM recalls GROUP ALL")
        .await?
        .take(0)?;

    if let Some(recalls_count) = recalls_count_result.first() {
        println!("Recalls count: {:?}", recalls_count);
    }

    // Check chain_id usage in thoughts
    let thoughts_with_chain_id: Vec<serde_json::Value> = db
        .query("SELECT count() FROM thoughts WHERE chain_id IS NOT NULL GROUP ALL")
        .await?
        .take(0)?;

    println!("Thoughts with chain_id: {:?}", thoughts_with_chain_id);

    let chain_id_samples: Vec<serde_json::Value> = db
        .query("SELECT chain_id FROM thoughts WHERE chain_id IS NOT NULL LIMIT 10")
        .await?
        .take(0)?;

    println!("Sample chain_id values: {:?}", chain_id_samples);

    // Check source_thought_id in memories
    let entities_with_source: Vec<serde_json::Value> = db
        .query("SELECT count() FROM kg_entities WHERE data.source_thought_id IS NOT NULL GROUP ALL")
        .await?
        .take(0)?;

    println!(
        "Entities with source_thought_id: {:?}",
        entities_with_source
    );

    let observations_with_source: Vec<serde_json::Value> = db
        .query("SELECT count() FROM kg_observations WHERE source_thought_id IS NOT NULL GROUP ALL")
        .await?
        .take(0)?;

    println!(
        "Observations with source_thought_id: {:?}",
        observations_with_source
    );

    let edges_with_source: Vec<serde_json::Value> = db
        .query("SELECT count() FROM kg_edges WHERE data.source_thought_id IS NOT NULL GROUP ALL")
        .await?
        .take(0)?;

    println!(
        "Relationships with source_thought_id: {:?}",
        edges_with_source
    );

    Ok(())
}

/// Quick check of database contents and counts
async fn check_contents() -> Result<()> {
    use surrealdb::Surreal;
    use surrealdb::engine::remote::ws::Ws;
    use surrealdb::opt::auth::Root;

    // Load environment
    dotenvy::dotenv().ok();

    let url = std::env::var("SURR_DB_URL").unwrap_or_else(|_| "127.0.0.1:8000".to_string());
    let user = std::env::var("SURR_DB_USER").unwrap_or_else(|_| "root".to_string());
    let pass = std::env::var("SURR_DB_PASS").unwrap_or_else(|_| "root".to_string());
    let ns = std::env::var("SURR_DB_NS").unwrap_or_else(|_| "surreal_mind".to_string());
    let dbname = std::env::var("SURR_DB_DB").unwrap_or_else(|_| "consciousness".to_string());

    println!("Connecting to SurrealDB at {}...", url);

    let db = Surreal::new::<Ws>(url).await?;
    db.signin(Root {
        username: &user,
        password: &pass,
    })
    .await?;
    db.use_ns(&ns).use_db(&dbname).await?;

    println!("Connected to namespace '{}' and database '{}'", ns, dbname);

    // Check thoughts count
    let count_result: Vec<serde_json::Value> = db
        .query("SELECT count() FROM thoughts GROUP ALL")
        .await?
        .take(0)?;

    println!("Thoughts count: {:?}", count_result);

    // Check thoughts with embeddings
    let with_embeddings: Vec<serde_json::Value> = db
        .query("SELECT count() FROM thoughts WHERE array::len(embedding) > 0 GROUP ALL")
        .await?
        .take(0)?;

    println!("Thoughts with embeddings: {:?}", with_embeddings);

    // Check thoughts without embeddings
    let without_embeddings: Vec<serde_json::Value> = db
        .query("SELECT count() FROM thoughts WHERE array::len(embedding) = 0 GROUP ALL")
        .await?
        .take(0)?;

    println!("Thoughts without embeddings: {:?}", without_embeddings);

    // Get sample thought IDs using simple query
    let thought_ids: Vec<serde_json::Value> =
        db.query("SELECT id FROM thoughts LIMIT 5").await?.take(0)?;

    println!("Sample thought IDs:");
    for thought in thought_ids {
        if let Some(id) = thought.get("id").and_then(|v| v.as_str()) {
            println!("  - {}", id);
        } else {
            println!("  - ID format: {:?}", thought.get("id"));
        }
    }

    Ok(())
}

/// Simple database connectivity test
async fn simple_test() -> Result<()> {
    use surrealdb::Surreal;
    use surrealdb::engine::remote::ws::Ws;
    use surrealdb::opt::auth::Root;

    // Load environment variables
    dotenvy::dotenv().ok();

    let url = std::env::var("SURR_DB_URL").unwrap_or_else(|_| "127.0.0.1:8000".to_string());
    let user = std::env::var("SURR_DB_USER").unwrap_or_else(|_| "root".to_string());
    let pass = std::env::var("SURR_DB_PASS").unwrap_or_else(|_| "root".to_string());
    let ns = std::env::var("SURR_DB_NS").unwrap_or_else(|_| "surreal_mind".to_string());
    let dbname = std::env::var("SURR_DB_DB").unwrap_or_else(|_| "consciousness".to_string());

    println!("Connecting to SurrealDB at {}...", url);

    // Connect to the running SurrealDB service
    let db = Surreal::new::<Ws>(url).await?;

    // Authenticate
    db.signin(Root {
        username: &user,
        password: &pass,
    })
    .await?;

    // Select namespace and database
    db.use_ns(&ns).use_db(&dbname).await?;

    println!(
        "Connected successfully to namespace '{}' and database '{}'",
        ns, dbname
    );

    // Test 1: Count thoughts
    println!("\n1. Counting thoughts...");
    let count_result: Vec<serde_json::Value> = db
        .query("SELECT count() FROM thoughts GROUP ALL")
        .await?
        .take(0)?;

    println!("Thoughts count: {:?}", count_result);

    // Test 2: Check thoughts with embeddings
    println!("\n2. Checking thoughts with embeddings...");
    let with_embeddings: Vec<serde_json::Value> = db
        .query("SELECT count() FROM thoughts WHERE array::len(embedding) > 0 GROUP ALL")
        .await?
        .take(0)?;

    println!("Thoughts with embeddings: {:?}", with_embeddings);

    // Test 3: Check thoughts without embeddings
    println!("\n3. Checking thoughts without embeddings...");
    let without_embeddings: Vec<serde_json::Value> = db
        .query("SELECT count() FROM thoughts WHERE array::len(embedding) = 0 GROUP ALL")
        .await?
        .take(0)?;

    println!("Thoughts without embeddings: {:?}", without_embeddings);

    // Test 4: Get sample thought IDs
    println!("\n4. Getting sample thought IDs...");
    let thought_ids: Vec<serde_json::Value> =
        db.query("SELECT id FROM thoughts LIMIT 5").await?.take(0)?;

    println!("Sample thought IDs:");
    for thought in thought_ids {
        if let Some(id) = thought.get("id").and_then(|v| v.as_str()) {
            println!("  - {}", id);
        }
    }

    // Test 5: Try the exact query from search_thoughts
    println!("\n5. Testing search_thoughts query...");
    let search_query = "SELECT id, content, created_at, embedding, significance, access_count, last_accessed, submode FROM thoughts ORDER BY created_at DESC LIMIT 50";

    let search_result: Result<Vec<serde_json::Value>, _> = db.query(search_query).await?.take(0);

    match search_result {
        Ok(rows) => {
            println!("Search query successful! Found {} rows", rows.len());
            if !rows.is_empty() {
                println!(
                    "First row keys: {:?}",
                    rows[0].as_object().map(|o| o.keys().collect::<Vec<_>>())
                );

                if let Some(embedding) = rows[0].get("embedding") {
                    println!("Embedding type: {:?}", embedding);
                    if let Some(arr) = embedding.as_array() {
                        println!("Embedding array length: {}", arr.len());
                    }
                }
            }
        }
        Err(e) => {
            println!("Search query failed: {}", e);
        }
    }

    println!("\n=== Test Complete ===");
    Ok(())
}

/// Fix embedding dimension mismatches by re-embedding
async fn fix_dims() -> Result<()> {
    use surreal_mind::embeddings::create_embedder;
    use surrealdb::Surreal;
    use surrealdb::engine::remote::ws::Ws;
    use surrealdb::opt::auth::Root;

    // Load environment from .env file
    if let Err(e) = dotenvy::dotenv() {
        eprintln!("Warning: Could not load .env file: {}", e);
    }

    // Load configuration
    let config = surreal_mind::config::Config::load().map_err(|e| {
        eprintln!("Failed to load configuration: {}", e);
        e
    })?;

    println!("🔧 Starting dimension correction process...");

    // Create embedder with current config (OpenAI primary)
    let embedder = create_embedder(&config).await?;
    let target_dims = embedder.dimensions();
    println!(
        "✅ Embedder ready: provider={}, model={}, dims={}",
        config.system.embedding_provider, config.system.embedding_model, target_dims
    );

    // Connect to SurrealDB
    let db = Surreal::new::<Ws>(&config.system.database_url).await?;
    db.signin(Root {
        username: &config.runtime.database_user,
        password: &config.runtime.database_pass,
    })
    .await?;
    db.use_ns(&config.system.database_ns)
        .use_db(&config.system.database_db)
        .await?;

    // Query thoughts with wrong dimensions
    println!("\n📊 Finding thoughts with incorrect embedding dimensions...");
    let result = db
        .query(
            "SELECT meta::id(id) as id, content, array::len(embedding) as emb_len, embedding_model, embedding_provider, embedding_dim FROM thoughts WHERE embedding_dim != $target_dims"
        )
        .bind(("target_dims", target_dims as i64))
        .await?;
    let mut response = result.check()?;
    let mismatched_thoughts: Vec<serde_json::Value> = response.take(0)?;
    println!(
        "✅ Found {} thoughts with wrong dimensions",
        mismatched_thoughts.len()
    );

    if mismatched_thoughts.is_empty() {
        println!("🎉 No dimension corrections needed!");
        return Ok(());
    }

    // Show distribution of wrong dimensions before fix
    println!("\n📈 Current wrong dimension distribution:");
    let dist_rows: Vec<serde_json::Value> = db
        .query(
            "SELECT embedding_dim as wrong_dim, count() as count FROM thoughts WHERE embedding_dim != $target_dims GROUP BY embedding_dim ORDER BY count DESC"
        )
        .bind(("target_dims", target_dims as i64))
        .await?
        .take(0)?;
    for r in &dist_rows {
        let wrong_dim = r.get("wrong_dim").and_then(|v| v.as_i64()).unwrap_or(0);
        let count = r.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
        println!("  - {:>6} dims | {:>6} items", wrong_dim, count);
    }

    // Process each mismatched thought
    println!("\n🔄 Correcting embedding dimensions to {}...", target_dims);
    let mut success_count = 0;
    let mut error_count = 0;

    for i in 0..mismatched_thoughts.len() {
        let thought = &mismatched_thoughts[i];

        // Progress indicator
        if i % 5 == 0 && i > 0 {
            println!(
                "  Progress: {}/{} ({}%) - {} successful, {} errors",
                i,
                mismatched_thoughts.len(),
                i * 100 / mismatched_thoughts.len(),
                success_count,
                error_count
            );
        }

        // Extract fields
        let thought_id = thought["id"].as_str().unwrap_or("unknown").to_string();
        let content = thought["content"].as_str().unwrap_or("").to_string();
        let old_dims = thought["embedding_dim"].as_u64().unwrap_or(0) as usize;

        // Generate new embedding
        match embedder.embed(&content).await {
            Ok(new_embedding) => {
                // Update thought with corrected embedding and metadata
                let provider = config.system.embedding_provider.clone();
                let model = config.system.embedding_model.clone();
                let query = "UPDATE type::thing('thoughts', $id) SET embedding = $embedding, embedding_provider = $provider, embedding_model = $model, embedding_dim = $dims, embedded_at = time::now() RETURN meta::id(id) as id";

                match db
                    .query(query)
                    .bind(("id", thought_id.clone()))
                    .bind(("embedding", new_embedding))
                    .bind(("provider", provider.clone()))
                    .bind(("model", model.clone()))
                    .bind(("dims", target_dims as i64))
                    .await
                {
                    Ok(_update_response) => {
                        success_count += 1;
                        println!(
                            "  ✅ Fixed {} ({} → {} dims)",
                            thought_id, old_dims, target_dims
                        );
                    }
                    Err(e) => {
                        error_count += 1;
                        eprintln!("  ⚠️  Failed to update {}: {}", thought_id, e);
                    }
                }
            }
            Err(e) => {
                error_count += 1;
                eprintln!("  ⚠️  Failed to re-embed content for {}: {}", thought_id, e);
            }
        }
    }

    // Verify the fixes
    println!("\n🔍 Verifying dimension corrections...");
    let final_check: Vec<serde_json::Value> = db
        .query(
            "SELECT count() as remaining_wrong FROM thoughts WHERE embedding_dim != $target_dims",
        )
        .bind(("target_dims", target_dims as i64))
        .await?
        .take(0)?;
    let remaining_wrong = final_check
        .first()
        .and_then(|r| r.get("remaining_wrong"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    // Final summary
    println!("\n📋 Dimension Correction Summary:");
    println!("  ✅ Successfully corrected: {}", success_count);
    println!("  ⚠️  Errors encountered: {}", error_count);
    println!("  🔧 Target dimensions: {}", target_dims);
    println!("  📊 Remaining wrong: {}", remaining_wrong);

    if remaining_wrong == 0 && error_count == 0 {
        println!("🎉 All dimensions corrected successfully!");
    } else if remaining_wrong > 0 {
        println!(
            "⚠️  {} thoughts still have wrong dimensions (may need manual review)",
            remaining_wrong
        );
    }

    Ok(())
}
