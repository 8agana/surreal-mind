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

    println!("🚀 Starting thought re-embedding process...");
    println!("📊 Target dimensions: 768 (from 1536)");

    // Create embedder (will use SURR_EMBED_DIM=768 from env)
    let embedder = create_embedder().await?;
    let embed_dims = embedder.dimensions();
    println!("✅ Embedder initialized with {} dimensions", embed_dims);

    // Connect to SurrealDB
    let db = Surreal::new::<Ws>("127.0.0.1:8000").await?;
    db.signin(Root {
        username: "root",
        password: "root",
    })
    .await?;
    db.use_ns("surreal_mind").use_db("consciousness").await?;

    // Get all thoughts using a raw query
    println!("\n📚 Fetching all thoughts from database...");
    let result = db.query("SELECT * FROM thoughts").await?;
    let mut response = result.check()?;
    let thoughts: Vec<serde_json::Value> = response.take(0)?;
    println!("✅ Found {} thoughts to process", thoughts.len());

    let mut success_count = 0;
    let mut skip_count = 0;
    let mut error_count = 0;

    println!("\n🔄 Re-embedding thoughts...");
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
        let existing_embedding = thought["embedding"].as_array();

        // Check if already correct dimensions
        if let Some(emb) = existing_embedding {
            if emb.len() == embed_dims {
                skip_count += 1;
                continue;
            }
        }

        // Generate new embedding
        match embedder.embed(content).await {
            Ok(new_embedding) => {
                // Update thought with new embedding
                let query = format!("UPDATE {} SET embedding = $embedding", thought_id);

                match db.query(&query).bind(("embedding", new_embedding)).await {
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
        "⏭️  Skipped (already {}-dim): {} thoughts",
        embed_dims, skip_count
    );
    println!("❌ Errors: {} thoughts", error_count);
    println!("🎯 New embedding dimensions: {}", embed_dims);
    println!("{}", "=".repeat(50));

    Ok(())
}
