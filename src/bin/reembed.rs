use anyhow::Result;
use chrono::Utc;
use surreal_mind::bge_embedder::BGEEmbedder;
use surreal_mind::embeddings::Embedder;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment from .env file
    if let Err(e) = dotenvy::dotenv() {
        eprintln!("Warning: Could not load .env file: {}", e);
    }

    println!("🚀 Starting thought re-embedding process with BGE-small...");
    println!("📊 Converting to local BGE embeddings (384 dimensions)...");

    // Create BGE embedder
    let embedder = BGEEmbedder::new()?;
    let embed_dims = embedder.dimensions();
    println!("✅ BGE embedder initialized with {} dimensions", embed_dims);

    // Connect to SurrealDB
    let db = Surreal::new::<Ws>("127.0.0.1:8000").await?;
    db.signin(Root {
        username: "root",
        password: "root",
    })
    .await?;
    db.use_ns("surreal_mind").use_db("consciousness").await?;

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

    println!("\n🔄 Re-embedding thoughts with BGE-small...");
    for (i, thought) in thoughts.iter().enumerate() {
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
        let thought_id = thought["id"].as_str().unwrap_or("unknown");
        let content = thought["content"].as_str().unwrap_or("");
        let existing_emb_len = thought["emb_len"].as_u64().unwrap_or(0) as usize;
        let existing_model = thought["embedding_model"].as_str().unwrap_or("");

        // Skip if already embedded with BGE-small
        if existing_emb_len == embed_dims
            && (existing_model == "BAAI/bge-small-en-v1.5" || existing_model == "bge-small-en-v1.5")
        {
            skip_count += 1;
            continue;
        }

        // Generate new embedding
        match embedder.embed(content).await {
            Ok(new_embedding) => {
                // Update thought with new embedding and metadata
                let timestamp = Utc::now().to_rfc3339();
                let query = format!(
                    "UPDATE thoughts:`{}` SET \
                        embedding = $embedding, \
                        embedding_provider = $provider, \
                        embedding_model = $model, \
                        embedding_dim = $dims, \
                        embedded_at = $timestamp",
                    thought_id
                );

                match db
                    .query(&query)
                    .bind(("embedding", new_embedding))
                    .bind(("provider", "candle"))
                    .bind(("model", "BAAI/bge-small-en-v1.5"))
                    .bind(("dims", embed_dims as i64))
                    .bind(("timestamp", timestamp))
                    .await
                {
                    Ok(_) => success_count += 1,
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
        "⏭️  Skipped (already BGE-{}-dim): {} thoughts",
        embed_dims, skip_count
    );
    println!("❌ Errors: {} thoughts", error_count);
    println!("🎯 New embedding model: BGE-small-en-v1.5");
    println!("🎯 New embedding dimensions: {}", embed_dims);
    println!("{}", "=".repeat(50));

    Ok(())
}
