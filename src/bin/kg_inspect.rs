use anyhow::Result;
use serde_json::Value;
use surreal_mind::embeddings::create_embedder;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;

#[tokio::main]
async fn main() -> Result<()> {
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
