use anyhow::Result;
// use chrono::Utc;
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

    println!("🚀 Starting thought re-embedding process...");
    // Prefer OpenAI 1536; fallback to local BGE if unavailable
    let embedder = create_embedder(&config).await?;
    let embed_dims = embedder.dimensions();
    println!(
        "✅ Embedder initialized (expected primary dims = {}), provider/model from config",
        embed_dims
    );

    // Connect to SurrealDB using config
    let db = Surreal::new::<Ws>(&config.system.database_url).await?;
    db.signin(Root {
        username: &config.runtime.database_user,
        password: &config.runtime.database_pass,
    })
    .await?;
    db.use_ns(&config.system.database_ns)
        .use_db(&config.system.database_db)
        .await?;

    // Show current distribution by provider/model/dimension
    println!("\n📊 Current embedding distribution (before re-embed):");
    let dist_rows: Vec<serde_json::Value> = db
        .query(
            "SELECT embedding_provider as provider, embedding_model as model, embedding_dim as dim, count() as count FROM thoughts GROUP BY embedding_provider, embedding_model, embedding_dim ORDER BY count DESC"
        )
        .await?
        .take(0)?;
    if dist_rows.is_empty() {
        println!("  (no existing embeddings found)");
    } else {
        for r in &dist_rows {
            let prov = r.get("provider").and_then(|v| v.as_str()).unwrap_or("NONE");
            let model = r.get("model").and_then(|v| v.as_str()).unwrap_or("NONE");
            let dim = r.get("dim").and_then(|v| v.as_i64()).unwrap_or(0);
            let count = r.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
            println!(
                "  - {:>6} dims | {:<8} | {:<28} | {:>6} items",
                dim, prov, model, count
            );
        }
    }

    // Get all thoughts using a raw query with meta::id() to avoid Thing serialization
    println!("\n📚 Fetching all thoughts from database...");
    let result = db
        .query("SELECT meta::id(id) as id, content, array::len(embedding) as emb_len, embedding_model, embedding_provider, embedding_dim FROM thoughts")
        .await?;
    let mut response = result.check()?;
    let thoughts: Vec<serde_json::Value> = response.take(0)?;
    println!("✅ Found {} thoughts to process", thoughts.len());

    let mut success_count = 0;
    let mut skip_count = 0;
    let mut error_count = 0;

    println!("\n🔄 Re-embedding thoughts with configured embedder (OpenAI→BGE fallback)...");
    for i in 0..thoughts.len() {
        let thought = &thoughts[i];
        // Progress indicator
        if i % 10 == 0 && i > 0 {
            println!(
                "  Progress: {}/{} ({}%)",
                i,
                thoughts.len(),
                i * 100 / thoughts.len()
            );
        }

        // Extract fields
        let thought_id = thought["id"].as_str().unwrap_or("unknown").to_string();
        let content = thought["content"].as_str().unwrap_or("").to_string();
        let existing_emb_len = thought["emb_len"].as_u64().unwrap_or(0) as usize;
        let existing_model = thought["embedding_model"].as_str().unwrap_or("");

        // Skip if already embedded with the current embedder's dimensions AND model matches config model
        let target_model = config.system.embedding_model.clone();
        if existing_emb_len == embed_dims && existing_model == target_model {
            skip_count += 1;
            continue;
        }

        // Generate new embedding
        match embedder.embed(&content).await {
            Ok(new_embedding) => {
                // Update thought with new embedding and metadata
                let (provider, model) = (
                    config.system.embedding_provider.clone(),
                    config.system.embedding_model.clone(),
                );
                let query = "UPDATE type::thing('thoughts', $id) SET embedding = $embedding, embedding_provider = $provider, embedding_model = $model, embedding_dim = $dims, embedded_at = time::now() RETURN meta::id(id) as id";

                match db
                    .query(query)
                    .bind(("id", thought_id.clone()))
                    .bind(("embedding", new_embedding))
                    .bind(("provider", provider.clone()))
                    .bind(("model", model.clone()))
                    .bind(("dims", embed_dims as i64))
                    .await
                {
                    Ok(response) => {
                        success_count += 1;
                        if i < 3 {
                            eprintln!(
                                "  ✅ Updated {} with provider={}, model={}, dims={}",
                                thought_id, provider, model, embed_dims
                            );
                            eprintln!("     Response: {:?}", response);
                        }
                    }
                    Err(e) => {
                        error_count += 1;
                        eprintln!("  ⚠️  Failed to update {}: {}", thought_id, e);
                    }
                }
            }
            Err(e) => {
                error_count += 1;
                eprintln!("  ⚠️  Failed to embed content for {}: {}", thought_id, e);
            }
        }
    }

    // Final statistics
    println!("\n{}", "=".repeat(50));
    println!("📊 RE-EMBEDDING COMPLETE!");
    println!("✅ Successfully re-embedded: {} thoughts", success_count);
    println!(
        "⏭️  Skipped (already target dims={} & model): {} thoughts",
        embed_dims, skip_count
    );
    println!("❌ Errors: {} thoughts", error_count);
    println!("🎯 Target embedding dimensions: {}", embed_dims);
    println!("{}", "=".repeat(50));

    Ok(())
}
