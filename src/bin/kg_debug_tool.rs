use anyhow::Result;
use serde_json::Value;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    // Load configuration using the library's config loader if accessible,
    // or just load from env directly to avoid visibility issues if Config isn't fully public.
    // Assuming surreal_mind::config::Config is public.
    let config = surreal_mind::config::Config::load()?;

    let url = config.system.database_url.clone();
    let user = config.runtime.database_user.clone();
    let pass = config.runtime.database_pass.clone();
    let ns = config.system.database_ns.clone();
    let dbname = config.system.database_db.clone();

    println!("Connecting to {} ({}/{})", url, ns, dbname);

    let db = Surreal::new::<Ws>(&url).await?;
    db.signin(Root {
        username: &user,
        password: &pass,
    })
    .await?;
    db.use_ns(&ns).use_db(&dbname).await?;

    // Check kg_entities
    println!("Checking kg_entities for anomalous embeddings...");
    let sql = "SELECT meta::id(id) as id, embedding, type::of(embedding) as type FROM kg_entities \
               WHERE NOT (type::is::array(embedding) AND array::len(embedding) > 0) \
               AND NOT (embedding IS NULL OR embedding IS NONE) \
               LIMIT 5";

    let rows: Vec<Value> = db.query(sql).await?.take(0)?;
    if rows.is_empty() {
        println!("No anomalous records found in kg_entities.");
    } else {
        for r in rows {
            println!("ANOMALY: {:?}", r);
        }
    }

    // Check kg_observations
    println!("Checking kg_observations for anomalous embeddings...");
    let sql = "SELECT meta::id(id) as id, embedding, type::of(embedding) as type FROM kg_observations \
               WHERE NOT (type::is::array(embedding) AND array::len(embedding) > 0) \
               AND NOT (embedding IS NULL OR embedding IS NONE) \
               LIMIT 5";

    let rows: Vec<Value> = db.query(sql).await?.take(0)?;
    if rows.is_empty() {
        println!("No anomalous records found in kg_observations.");
    } else {
        for r in rows {
            println!("ANOMALY: {:?}", r);
        }
    }

    // Check kg_edges
    println!("Checking kg_edges for anomalous embeddings...");
    let sql = "SELECT meta::id(id) as id, embedding, type::of(embedding) as type FROM kg_edges \
               WHERE NOT (type::is::array(embedding) AND array::len(embedding) > 0) \
               AND NOT (embedding IS NULL OR embedding IS NONE) \
               LIMIT 5";

    let rows: Vec<Value> = db.query(sql).await?.take(0)?;
    if rows.is_empty() {
        println!("No anomalous records found in kg_edges.");
    } else {
        for r in rows {
            println!("ANOMALY: {:?}", r);
        }
    }

    Ok(())
}
