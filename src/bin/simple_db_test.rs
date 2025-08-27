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
