use anyhow::Result;
use chrono::Utc;
use serde_json::Value;
use surreal_mind::embeddings::create_embedder;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;

fn bool_env(name: &str, default: bool) -> bool {
    std::env::var(name)
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(default)
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    let dry_run = bool_env("DRY_RUN", false);
    let limit = std::env::var("LIMIT")
        .ok()
        .and_then(|s| s.parse::<usize>().ok());

    println!("🚀 KG re-embed starting (entities + observations)");
    if dry_run {
        println!("🔎 Dry run: no writes to DB");
    }

    // Embedder (same policy as core): OpenAI primary (1536) if key; else Candle BGE (384)
    let embedder = create_embedder().await?;
    let dims = embedder.dimensions();
    let prov = std::env::var("SURR_EMBED_PROVIDER").unwrap_or_else(|_| "openai".to_string());
    let model =
        std::env::var("SURR_EMBED_MODEL").unwrap_or_else(|_| "text-embedding-3-small".to_string());
    println!(
        "✅ Embedder ready (provider={}, model={}, dims={})",
        prov, model, dims
    );

    // DB connection
    let url = std::env::var("SURR_DB_URL").unwrap_or_else(|_| "127.0.0.1:8000".to_string());
    let user = std::env::var("SURR_DB_USER").unwrap_or_else(|_| "root".to_string());
    let pass = std::env::var("SURR_DB_PASS").unwrap_or_else(|_| "root".to_string());
    let ns = std::env::var("SURR_DB_NS").unwrap_or_else(|_| "surreal_mind".to_string());
    let dbname = std::env::var("SURR_DB_DB").unwrap_or_else(|_| "consciousness".to_string());
    let db = Surreal::new::<Ws>(url).await?;
    db.signin(Root {
        username: &user,
        password: &pass,
    })
    .await?;
    db.use_ns(ns).use_db(dbname).await?;

    let mut updated_entities = 0usize;
    let mut skipped_entities = 0usize;
    let mut updated_obs = 0usize;
    let mut skipped_obs = 0usize;

    // Entities
    {
        let sql = match limit {
            Some(l) => format!("SELECT meta::id(id) as id, name, data, entity_type, (IF type::is::array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len, embedding_model FROM kg_entities LIMIT {}", l),
            None => "SELECT meta::id(id) as id, name, data, entity_type, (IF type::is::array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len, embedding_model FROM kg_entities".to_string(),
        };
        let rows: Vec<Value> = db.query(sql).await?.take(0)?;
        println!("📚 Entities to check: {}", rows.len());
        for i in 0..rows.len() {
            let r = &rows[i];
            if i % 50 == 0 && i > 0 {
                println!("  Entities progress: {}/{}", i, rows.len());
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
        println!("📚 Observations to check: {}", rows.len());
        for i in 0..rows.len() {
            let r = &rows[i];
            if i % 50 == 0 && i > 0 {
                println!("  Observations progress: {}/{}", i, rows.len());
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

    println!("\n===== KG RE-EMBED SUMMARY =====");
    println!(
        "Entities: updated={}, skipped={}",
        updated_entities, skipped_entities
    );
    println!(
        "Observations: updated={}, skipped={}",
        updated_obs, skipped_obs
    );
    println!("Provider/model: {} / {} ({} dims)", prov, model, dims);

    Ok(())
}
