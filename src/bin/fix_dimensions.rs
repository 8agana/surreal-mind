use anyhow::Result;
use surreal_mind::embeddings::create_embedder;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;

#[tokio::main]
async fn main() -> Result<()> {
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
