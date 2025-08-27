use anyhow::Result;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;

#[tokio::main]
async fn main() -> Result<()> {
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
