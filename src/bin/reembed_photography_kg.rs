use anyhow::Result;
use chrono::Utc;
use clap::Parser;
use serde_json::Value;
use surreal_mind::embeddings::create_embedder;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;

/// Normalize WebSocket URL for SurrealDB connection (expects host:port, no scheme)
fn normalize_ws_url(s: &str) -> String {
    s.strip_prefix("ws://")
        .or_else(|| s.strip_prefix("wss://"))
        .or_else(|| s.strip_prefix("http://"))
        .or_else(|| s.strip_prefix("https://"))
        .unwrap_or(s)
        .to_string()
}

/// Photography KG re-embedding tool
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Dry run: do not write to DB
    #[arg(long)]
    dry_run: bool,

    /// Limit number of items to process (0 for no limit)
    #[arg(long, default_value = "0")]
    limit: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    let args = Args::parse();

    let dry_run = args.dry_run;
    let limit = if args.limit == 0 {
        None
    } else {
        Some(args.limit)
    };

    println!("📸 Photography KG re-embed starting (entities + observations)");
    if dry_run {
        println!("🔎 Dry run: no writes to DB");
    }

    // Load configuration
    let config = surreal_mind::config::Config::load().map_err(|e| {
        eprintln!("Failed to load configuration: {}", e);
        e
    })?;

    // Embedder (same policy as core): OpenAI primary (1536) if key; else Candle BGE (384)
    let embedder = create_embedder(&config).await?;
    let dims = embedder.dimensions();
    let prov = config.system.embedding_provider.clone();
    let model = config.system.embedding_model.clone();
    println!(
        "✅ Embedder ready (provider={}, model={}, dims={})",
        prov, model, dims
    );

    // Photography DB connection - reuse env vars for consistency
    let url = std::env::var("SURR_PHOTO_URL")
        .or_else(|_| std::env::var("SURR_DB_URL"))
        .unwrap_or_else(|_| "ws://127.0.0.1:8000".to_string());
    let user = std::env::var("SURR_PHOTO_USER")
        .or_else(|_| std::env::var("SURR_DB_USER"))
        .unwrap_or_else(|_| "root".to_string());
    let pass = std::env::var("SURR_PHOTO_PASS")
        .or_else(|_| std::env::var("SURR_DB_PASS"))
        .unwrap_or_else(|_| "root".to_string());
    let ns = std::env::var("SURR_PHOTO_NS").unwrap_or_else(|_| "photography".to_string());
    let dbname = std::env::var("SURR_PHOTO_DB").unwrap_or_else(|_| "work".to_string());
    let normalized_url = normalize_ws_url(&url);
    let db = Surreal::new::<Ws>(&normalized_url).await?;
    db.signin(Root {
        username: &user,
        password: &pass,
    })
    .await?;
    db.use_ns(&ns).use_db(&dbname).await?;
    println!("✅ Photography DB connected: {} / {}", ns, dbname);

    let mut updated_entities = 0usize;
    let mut skipped_entities = 0usize;
    let mut mismatched_entities = 0usize;
    let mut missing_entities = 0usize;
    let mut updated_obs = 0usize;
    let mut skipped_obs = 0usize;
    let mut mismatched_obs = 0usize;
    let mut missing_obs = 0usize;

    // Entities
    {
        let sql = match limit {
            Some(l) => format!("SELECT meta::id(id) as id, name, data, entity_type, (IF type::is::array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len, embedding_model FROM kg_entities LIMIT {}", l),
            None => "SELECT meta::id(id) as id, name, data, entity_type, (IF type::is::array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len, embedding_model FROM kg_entities".to_string(),
        };
        let rows: Vec<Value> = db.query(sql).await?.take(0)?;
        println!("📚 Photography entities to check: {}", rows.len());
        for i in 0..rows.len() {
            let r = &rows[i];
            if i % 50 == 0 && i > 0 {
                println!("  Photography entities progress: {}/{}", i, rows.len());
            }
            let id = r
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = r
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let emb_len = r.get("emb_len").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let model = r
                .get("embedding_model")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let etype = r
                .get("entity_type")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    r.get("data")
                        .and_then(|d| d.get("entity_type"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_default();
            // Hygiene counts
            if emb_len == 0 {
                missing_entities += 1;
            }
            if emb_len != dims
                || !(model == "text-embedding-3-small"
                    || model == "BAAI/bge-small-en-v1.5"
                    || model == "bge-small-en-v1.5")
            {
                mismatched_entities += 1;
            }

            if emb_len == dims
                && (model == "text-embedding-3-small"
                    || model == "BAAI/bge-small-en-v1.5"
                    || model == "bge-small-en-v1.5")
            {
                skipped_entities += 1;
                continue;
            }
            let text = if etype.is_empty() {
                name.clone()
            } else {
                format!("{} ({})", name, etype)
            };
            let emb = embedder.embed(&text).await?;
            if !dry_run {
                let ts = Utc::now().to_rfc3339();
                let q = format!(
                    "UPDATE kg_entities:`{}` SET embedding = $emb, embedding_provider = $prov, embedding_model = $model, embedding_dim = $dim, embedded_at = $ts",
                    id
                );
                db.query(q)
                    .bind(("emb", emb))
                    .bind(("prov", prov.clone()))
                    .bind(("model", model.clone()))
                    .bind(("dim", dims as i64))
                    .bind(("ts", ts))
                    .await?;
            }
            updated_entities += 1;
        }
    }

    // Observations
    {
        let sql = match limit {
            Some(l) => format!("SELECT meta::id(id) as id, name, data, (IF type::is::array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len, embedding_model FROM kg_observations LIMIT {}", l),
            None => "SELECT meta::id(id) as id, name, data, (IF type::is::array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len, embedding_model FROM kg_observations".to_string(),
        };
        let rows: Vec<Value> = db.query(sql).await?.take(0)?;
        println!("📚 Photography observations to check: {}", rows.len());
        for i in 0..rows.len() {
            let r = &rows[i];
            if i % 50 == 0 && i > 0 {
                println!("  Photography observations progress: {}/{}", i, rows.len());
            }
            let id = r
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = r
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let emb_len = r.get("emb_len").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let model = r
                .get("embedding_model")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            // Hygiene counts
            if emb_len == 0 {
                missing_obs += 1;
            }
            if emb_len != dims
                || !(model == "text-embedding-3-small"
                    || model == "BAAI/bge-small-en-v1.5"
                    || model == "bge-small-en-v1.5")
            {
                mismatched_obs += 1;
            }

            if emb_len == dims
                && (model == "text-embedding-3-small"
                    || model == "BAAI/bge-small-en-v1.5"
                    || model == "bge-small-en-v1.5")
            {
                skipped_obs += 1;
                continue;
            }
            // Use name plus lightweight data summary if present
            let mut text = name.clone();
            if let Some(d) = r.get("data") {
                if let Some(obj) = d.as_object() {
                    if let Some(desc) = obj.get("description").and_then(|v| v.as_str()) {
                        text.push_str(" - ");
                        text.push_str(desc);
                    }
                }
            }
            let emb = embedder.embed(&text).await?;
            if !dry_run {
                let ts = Utc::now().to_rfc3339();
                let q = format!(
                    "UPDATE kg_observations:`{}` SET embedding = $emb, embedding_provider = $prov, embedding_model = $model, embedding_dim = $dim, embedded_at = $ts",
                    id
                );
                db.query(q)
                    .bind(("emb", emb))
                    .bind(("prov", prov.clone()))
                    .bind(("model", model.clone()))
                    .bind(("dim", dims as i64))
                    .bind(("ts", ts))
                    .await?;
            }
            updated_obs += 1;
        }
    }

    println!("\n===== PHOTOGRAPHY KG RE-EMBED SUMMARY =====");
    println!(
        "Entities: updated={}, skipped={}, mismatched={}, missing={}",
        updated_entities, skipped_entities, mismatched_entities, missing_entities
    );
    println!(
        "Observations: updated={}, skipped={}, mismatched={}, missing={}",
        updated_obs, skipped_obs, mismatched_obs, missing_obs
    );
    println!("Provider/model: {} / {} ({} dims)", prov, model, dims);

    Ok(())
}
