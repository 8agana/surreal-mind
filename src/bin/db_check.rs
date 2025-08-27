use anyhow::Result;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;

#[tokio::main]
async fn main() -> Result<()> {
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

    Ok(())
}
