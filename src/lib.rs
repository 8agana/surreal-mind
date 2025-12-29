pub mod bge_embedder;
pub mod clients;
pub mod cognitive;
pub mod config;
pub mod deserializers;
pub mod embeddings;
pub mod error;
pub mod indexes;
pub mod schemas;
pub mod serializers;
pub mod server;
pub mod tools;
pub mod utils;

use anyhow::Result;

#[derive(Debug, serde::Serialize)]
pub struct ReembedStats {
    pub expected_dim: usize,
    pub batch_size: usize,
    pub dry_run: bool,
    pub missing_only: bool,
    pub processed: usize,
    pub updated: usize,
    pub skipped: usize,
    pub missing: usize,
    pub mismatched: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct ReembedKgStats {
    pub expected_dim: usize,
    pub provider: String,
    pub model: String,
    pub dry_run: bool,
    pub entities_updated: usize,
    pub entities_skipped: usize,
    pub entities_missing: usize,
    pub entities_mismatched: usize,
    pub observations_updated: usize,
    pub observations_skipped: usize,
    pub observations_missing: usize,
    pub observations_mismatched: usize,
}

// Load env from a simple, standardized location resolution.
// This uses dotenvy::dotenv().ok() which loads .env if present and silently ignores if missing.
pub fn load_env() {
    let _ = dotenvy::dotenv();
}

// Types are defined in their respective modules

pub async fn run_reembed(
    batch_size: usize,
    limit: Option<usize>,
    missing_only: bool,
    dry_run: bool,
) -> Result<ReembedStats> {
    // Load configuration
    let config = crate::config::Config::load()?;

    // HTTP SQL client using centralized utility
    let http_config = utils::HttpSqlConfig::from_config(&config, "reembed");
    let sql_url = http_config.sql_url();
    let user = http_config.username.clone();
    let pass = http_config.password.clone();
    let ns = http_config.namespace.clone();
    let dbname = http_config.database.clone();
    let http = http_config.build_client()?;

    // Embedder
    let embedder = embeddings::create_embedder(&config).await?;
    let expected_dim = embedder.dimensions();
    let provider = config.system.embedding_provider.clone();
    let model = config.system.embedding_model.clone();

    let mut start: usize = 0;
    let mut processed: usize = 0;
    let mut updated: usize = 0;
    let mut skipped: usize = 0;
    let mut mismatched: usize = 0;
    let mut missing: usize = 0;
    let limit_total = limit.unwrap_or(usize::MAX);

    loop {
        let remaining = limit_total.saturating_sub(processed);
        if remaining == 0 {
            break;
        }
        let take = remaining.min(batch_size);

        let select_sql = format!(
            "USE NS {} DB {}; SELECT meta::id(id) AS id, content, created_at, array::len(embedding) AS elen FROM thoughts ORDER BY created_at ASC LIMIT {} START {};",
            ns, dbname, take, start
        );
        let resp = http
            .post(&sql_url)
            .basic_auth(&user, Some(&pass))
            .header("Accept", "application/json")
            .header("Content-Type", "application/surrealql")
            .body(select_sql)
            .send()
            .await?;
        if !resp.status().is_success() {
            anyhow::bail!(
                "HTTP select failed: {}",
                resp.text().await.unwrap_or_default()
            );
        }
        let blocks: serde_json::Value = resp.json().await?;
        let result = blocks
            .as_array()
            .and_then(|arr| {
                arr.iter()
                    .find_map(|b| b.get("result").and_then(|r| r.as_array()).cloned())
            })
            .unwrap_or_default();
        if result.is_empty() {
            break;
        }

        for item in result.iter() {
            let id_raw = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let content = item
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let cur_len = item.get("elen").and_then(|v| v.as_i64()).unwrap_or(0) as usize;
            let needs_update = if missing_only {
                cur_len != expected_dim
            } else {
                true
            };
            if !needs_update {
                skipped += 1;
                processed += 1;
                continue;
            }
            if dry_run {
                if cur_len == 0 {
                    missing += 1;
                } else if cur_len != expected_dim {
                    mismatched += 1;
                }
                processed += 1;
                continue;
            }
            let new_emb = embedder.embed(&content).await?;
            if new_emb.len() != expected_dim {
                anyhow::bail!(
                    "Embedding dimension mismatch: expected {}, got {}",
                    expected_dim,
                    new_emb.len()
                );
            }
            let emb_json = serde_json::to_string(&new_emb)?;
            let update_sql = format!(
                "USE NS {} DB {}; UPDATE thoughts SET embedding = {}, embedding_provider = '{}', embedding_model = '{}', embedding_dim = {}, embedded_at = time::now() WHERE id = '{}' RETURN NONE;",
                ns, dbname, emb_json, provider, model, expected_dim, id_raw
            );
            let uresp = http
                .post(&sql_url)
                .basic_auth(&user, Some(&pass))
                .header("Accept", "application/json")
                .header("Content-Type", "application/surrealql")
                .body(update_sql)
                .send()
                .await?;
            if !uresp.status().is_success() {
                anyhow::bail!(
                    "HTTP update failed: {}",
                    uresp.text().await.unwrap_or_default()
                );
            }
            if cur_len == 0 {
                missing += 1;
            } else if cur_len != expected_dim {
                mismatched += 1;
            }
            updated += 1;
            processed += 1;
        }

        start += result.len();
    }

    Ok(ReembedStats {
        expected_dim,
        batch_size,
        dry_run,
        missing_only,
        processed,
        updated,
        skipped,
        missing,
        mismatched,
    })
}

pub async fn run_reembed_kg(limit: Option<usize>, dry_run: bool) -> Result<ReembedKgStats> {
    use chrono::Utc;
    use serde_json::Value;
    use surrealdb::Surreal;
    use surrealdb::engine::remote::ws::Ws;
    use surrealdb::opt::auth::Root;

    // Load configuration
    let config = crate::config::Config::load()?;

    // Embedder
    let embedder = embeddings::create_embedder(&config).await?;
    let dims = embedder.dimensions();
    let prov = config.system.embedding_provider.clone();
    let model = config.system.embedding_model.clone();

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
        for r in &rows {
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
            let emb_model = r
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
                || !(emb_model == "text-embedding-3-small"
                    || emb_model == "BAAI/bge-small-en-v1.5"
                    || emb_model == "bge-small-en-v1.5")
            {
                mismatched_entities += 1;
            }

            if emb_len == dims
                && (emb_model == "text-embedding-3-small"
                    || emb_model == "BAAI/bge-small-en-v1.5"
                    || emb_model == "bge-small-en-v1.5")
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
        for r in &rows {
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
            let emb_model = r
                .get("embedding_model")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Hygiene counts
            if emb_len == 0 {
                missing_obs += 1;
            }
            if emb_len != dims
                || !(emb_model == "text-embedding-3-small"
                    || emb_model == "BAAI/bge-small-en-v1.5"
                    || emb_model == "bge-small-en-v1.5")
            {
                mismatched_obs += 1;
            }

            if emb_len == dims
                && (emb_model == "text-embedding-3-small"
                    || emb_model == "BAAI/bge-small-en-v1.5"
                    || emb_model == "bge-small-en-v1.5")
            {
                skipped_obs += 1;
                continue;
            }

            // Use name plus lightweight data summary if present
            let mut text = name.clone();
            if let Some(d) = r.get("data")
                && let Some(obj) = d.as_object()
                && let Some(desc) = obj.get("description").and_then(|v| v.as_str())
            {
                text.push_str(" - ");
                text.push_str(desc);
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

    Ok(ReembedKgStats {
        expected_dim: dims,
        provider: prov,
        model,
        dry_run,
        entities_updated: updated_entities,
        entities_skipped: skipped_entities,
        entities_missing: missing_entities,
        entities_mismatched: mismatched_entities,
        observations_updated: updated_obs,
        observations_skipped: skipped_obs,
        observations_missing: missing_obs,
        observations_mismatched: mismatched_obs,
    })
}
