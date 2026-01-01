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
    pub edges_updated: usize,
    pub edges_skipped: usize,
    pub edges_missing: usize,
    pub edges_mismatched: usize,
}

/// Stats for kg_embed binary - embeds ONLY records with NULL embeddings
#[derive(Debug, serde::Serialize)]
pub struct KgEmbedStats {
    pub expected_dim: usize,
    pub provider: String,
    pub model: String,
    pub dry_run: bool,
    pub entities_updated: usize,
    pub entities_skipped: usize,
    pub observations_updated: usize,
    pub observations_skipped: usize,
    pub edges_updated: usize,
    pub edges_skipped: usize,
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
    let mut updated_edges = 0usize;
    let mut skipped_edges = 0usize;
    let mut mismatched_edges = 0usize;
    let mut missing_edges = 0usize;

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

    // Edges
    {
        let sql = match limit {
            Some(l) => format!("SELECT meta::id(id) as id, source.name as source_name, target.name as target_name, rel_type, data, (IF type::is::array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len, embedding_model FROM kg_edges LIMIT {}", l),
            None => "SELECT meta::id(id) as id, source.name as source_name, target.name as target_name, rel_type, data, (IF type::is::array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len, embedding_model FROM kg_edges".to_string(),
        };
        let rows: Vec<Value> = db.query(sql).await?.take(0)?;
        for r in &rows {
            let id = r
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let source_name = r
                .get("source_name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            let target_name = r
                .get("target_name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            let rel_type = r
                .get("rel_type")
                .and_then(|v| v.as_str())
                .unwrap_or("related_to");
            let emb_len = r.get("emb_len").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let emb_model = r
                .get("embedding_model")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Hygiene counts
            if emb_len == 0 {
                missing_edges += 1;
            }
            if emb_len != dims
                || !(emb_model == "text-embedding-3-small"
                    || emb_model == "BAAI/bge-small-en-v1.5"
                    || emb_model == "bge-small-en-v1.5")
            {
                mismatched_edges += 1;
            }

            if emb_len == dims
                && (emb_model == "text-embedding-3-small"
                    || emb_model == "BAAI/bge-small-en-v1.5"
                    || emb_model == "bge-small-en-v1.5")
            {
                skipped_edges += 1;
                continue;
            }

            // Construct text: source_name rel_type target_name - description
            let mut text = format!("{} {} {}", source_name, rel_type, target_name);
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
                    "UPDATE kg_edges:`{}` SET embedding = $emb, embedding_provider = $prov, embedding_model = $model, embedding_dim = $dim, embedded_at = $ts",
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
            updated_edges += 1;
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
        edges_updated: updated_edges,
        edges_skipped: skipped_edges,
        edges_missing: missing_edges,
        edges_mismatched: mismatched_edges,
    })
}

/// Embed ONLY KG records with NULL embeddings (missing-only, no re-embedding).
/// This is distinct from run_reembed_kg which also handles mismatched embeddings.
///
/// Text templates per spec:
/// - Entities: "{name} — {description}" (fallback to name if description missing)
/// - Observations: data.content (fallback to name)
/// - Edges: "{from} {rel_type} {to} — {description}" (resolve source/target names)
///
/// Batch sizes: 100 entities, 100 edges, 50 observations
/// Idempotent: UPDATE ... WHERE id = $id AND embedding IS NULL
pub async fn run_kg_embed(limit: Option<usize>, dry_run: bool) -> Result<KgEmbedStats> {
    use chrono::Utc;
    use serde_json::Value;
    use surrealdb::Surreal;
    use surrealdb::engine::remote::ws::Ws;
    use surrealdb::opt::auth::Root;

    const ENTITY_BATCH: usize = 100;
    const EDGE_BATCH: usize = 100;
    const OBS_BATCH: usize = 50;

    // Load configuration
    let config = crate::config::Config::load()?;

    // Embedder
    let embedder = embeddings::create_embedder(&config).await?;
    let dims = embedder.dimensions();
    let prov = config.system.embedding_provider.clone();
    let model = config.system.embedding_model.clone();

    println!(
        "[kg_embed] Starting with provider={}, model={}, dims={}",
        prov, model, dims
    );

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

    let mut entities_updated = 0usize;
    let mut entities_skipped = 0usize;
    let mut observations_updated = 0usize;
    let mut observations_skipped = 0usize;
    let mut edges_updated = 0usize;
    let mut edges_skipped = 0usize;

    let mut entities_missing_null = 0usize;
    let mut entities_missing_none = 0usize;
    let mut entities_missing_empty = 0usize;
    let mut observations_missing_null = 0usize;
    let mut observations_missing_none = 0usize;
    let mut observations_missing_empty = 0usize;
    let mut edges_missing_null = 0usize;
    let mut edges_missing_none = 0usize;
    let mut edges_missing_empty = 0usize;

    let limit_total = limit.unwrap_or(usize::MAX);

    // ========== ENTITIES ==========
    println!(
        "[kg_embed] Processing entities (batch size: {})...",
        ENTITY_BATCH
    );
    let mut entity_remaining = limit_total;
    loop {
        if entity_remaining == 0 {
            break;
        }
        let take = entity_remaining.min(ENTITY_BATCH);

        let sql = format!(
            "SELECT meta::id(id) as id, name, data, \
                (embedding IS NULL) AS emb_is_null, \
                (embedding IS NONE) AS emb_is_none, \
                (IF type::is::array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len \
             FROM kg_entities \
             WHERE (embedding IS NULL OR embedding IS NONE OR (type::is::array(embedding) AND array::len(embedding) = 0)) \
             LIMIT {}",
            take
        );
        let rows: Vec<Value> = db.query(&sql).await?.take(0)?;
        if rows.is_empty() {
            break;
        }

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
            let emb_is_null = r
                .get("emb_is_null")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let emb_is_none = r
                .get("emb_is_none")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let emb_len = r.get("emb_len").and_then(|v| v.as_u64()).unwrap_or(0);

            if emb_is_null {
                entities_missing_null += 1;
            } else if emb_is_none {
                entities_missing_none += 1;
            } else if emb_len == 0 {
                entities_missing_empty += 1;
            }

            // Extract description from data.description
            let description = r
                .get("data")
                .and_then(|d| d.as_object())
                .and_then(|obj| obj.get("description"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // Text template: "{name} — {description}" or just "{name}"
            let text = if description.is_empty() {
                name.clone()
            } else {
                format!("{} — {}", name, description)
            };

            if dry_run {
                println!(
                    "[dry_run] Would embed entity {}: \"{}\"",
                    id,
                    &text[..text.len().min(60)]
                );
                entities_updated += 1;
                entity_remaining = entity_remaining.saturating_sub(1);
                continue;
            }

            let emb = embedder.embed(&text).await?;
            let ts = Utc::now().to_rfc3339();

            // Idempotent update: only update if embedding is still NULL
            let q = format!(
                "UPDATE kg_entities:`{}` SET embedding = $emb, embedding_provider = $prov, embedding_model = $model, embedding_dim = $dim, embedded_at = $ts \
                 WHERE (embedding IS NULL OR embedding IS NONE OR (type::is::array(embedding) AND array::len(embedding) = 0)) RETURN NONE",
                id
            );
            db.query(q)
                .bind(("emb", emb))
                .bind(("prov", prov.clone()))
                .bind(("model", model.clone()))
                .bind(("dim", dims as i64))
                .bind(("ts", ts))
                .await?;
            entities_updated += 1;
            entity_remaining = entity_remaining.saturating_sub(1);
        }

        if rows.len() < take {
            break;
        }
    }
    println!(
        "[kg_embed] Entities: updated={}, skipped={}, missing(null/none/empty)={}/{}/{}",
        entities_updated,
        entities_skipped,
        entities_missing_null,
        entities_missing_none,
        entities_missing_empty
    );

    // ========== OBSERVATIONS ==========
    println!(
        "[kg_embed] Processing observations (batch size: {})...",
        OBS_BATCH
    );
    let mut obs_remaining = limit_total;
    loop {
        if obs_remaining == 0 {
            break;
        }
        let take = obs_remaining.min(OBS_BATCH);

        let sql = format!(
            "SELECT meta::id(id) as id, name, data, \
                (embedding IS NULL) AS emb_is_null, \
                (embedding IS NONE) AS emb_is_none, \
                (IF type::is::array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len \
             FROM kg_observations \
             WHERE (embedding IS NULL OR embedding IS NONE OR (type::is::array(embedding) AND array::len(embedding) = 0)) \
             LIMIT {}",
            take
        );
        let rows: Vec<Value> = db.query(&sql).await?.take(0)?;
        if rows.is_empty() {
            break;
        }

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
            let emb_is_null = r
                .get("emb_is_null")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let emb_is_none = r
                .get("emb_is_none")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let emb_len = r.get("emb_len").and_then(|v| v.as_u64()).unwrap_or(0);

            if emb_is_null {
                observations_missing_null += 1;
            } else if emb_is_none {
                observations_missing_none += 1;
            } else if emb_len == 0 {
                observations_missing_empty += 1;
            }

            // Extract content from data.content, fallback to name
            let content = r
                .get("data")
                .and_then(|d| d.as_object())
                .and_then(|obj| obj.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let text = if content.is_empty() { &name } else { content };

            if dry_run {
                println!(
                    "[dry_run] Would embed observation {}: \"{}\"",
                    id,
                    &text[..text.len().min(60)]
                );
                observations_updated += 1;
                obs_remaining = obs_remaining.saturating_sub(1);
                continue;
            }

            let emb = embedder.embed(text).await?;
            let ts = Utc::now().to_rfc3339();

            let q = format!(
                "UPDATE kg_observations:`{}` SET embedding = $emb, embedding_provider = $prov, embedding_model = $model, embedding_dim = $dim, embedded_at = $ts \
                 WHERE (embedding IS NULL OR embedding IS NONE OR (type::is::array(embedding) AND array::len(embedding) = 0)) RETURN NONE",
                id
            );
            db.query(q)
                .bind(("emb", emb))
                .bind(("prov", prov.clone()))
                .bind(("model", model.clone()))
                .bind(("dim", dims as i64))
                .bind(("ts", ts))
                .await?;
            observations_updated += 1;
            obs_remaining = obs_remaining.saturating_sub(1);
        }

        if rows.len() < take {
            break;
        }
    }
    println!(
        "[kg_embed] Observations: updated={}, skipped={}, missing(null/none/empty)={}/{}/{}",
        observations_updated,
        observations_skipped,
        observations_missing_null,
        observations_missing_none,
        observations_missing_empty
    );

    // ========== EDGES ==========
    println!(
        "[kg_embed] Processing edges (batch size: {})...",
        EDGE_BATCH
    );
    let mut edge_remaining = limit_total;
    loop {
        if edge_remaining == 0 {
            break;
        }
        let take = edge_remaining.min(EDGE_BATCH);

        // Resolve source.name and target.name in the query
        let sql = format!(
            "SELECT meta::id(id) as id, source.name as source_name, target.name as target_name, rel_type, data, \
                (embedding IS NULL) AS emb_is_null, \
                (embedding IS NONE) AS emb_is_none, \
                (IF type::is::array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len \
             FROM kg_edges \
             WHERE (embedding IS NULL OR embedding IS NONE OR (type::is::array(embedding) AND array::len(embedding) = 0)) \
             LIMIT {}",
            take
        );
        let rows: Vec<Value> = db.query(&sql).await?.take(0)?;
        if rows.is_empty() {
            break;
        }

        for r in &rows {
            let id = r
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let source_name = r
                .get("source_name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            let target_name = r
                .get("target_name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            let rel_type = r
                .get("rel_type")
                .and_then(|v| v.as_str())
                .unwrap_or("related_to");
            let emb_is_null = r
                .get("emb_is_null")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let emb_is_none = r
                .get("emb_is_none")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let emb_len = r.get("emb_len").and_then(|v| v.as_u64()).unwrap_or(0);

            if emb_is_null {
                edges_missing_null += 1;
            } else if emb_is_none {
                edges_missing_none += 1;
            } else if emb_len == 0 {
                edges_missing_empty += 1;
            }

            // Extract description from data.description
            let description = r
                .get("data")
                .and_then(|d| d.as_object())
                .and_then(|obj| obj.get("description"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // Text template: "{from} {rel_type} {to} — {description}"
            let text = if description.is_empty() {
                format!("{} {} {}", source_name, rel_type, target_name)
            } else {
                format!(
                    "{} {} {} — {}",
                    source_name, rel_type, target_name, description
                )
            };

            if dry_run {
                println!(
                    "[dry_run] Would embed edge {}: \"{}\"",
                    id,
                    &text[..text.len().min(60)]
                );
                edges_updated += 1;
                edge_remaining = edge_remaining.saturating_sub(1);
                continue;
            }

            let emb = embedder.embed(&text).await?;
            let ts = Utc::now().to_rfc3339();

            let q = format!(
                "UPDATE kg_edges:`{}` SET embedding = $emb, embedding_provider = $prov, embedding_model = $model, embedding_dim = $dim, embedded_at = $ts \
                 WHERE (embedding IS NULL OR embedding IS NONE OR (type::is::array(embedding) AND array::len(embedding) = 0)) RETURN NONE",
                id
            );
            db.query(q)
                .bind(("emb", emb))
                .bind(("prov", prov.clone()))
                .bind(("model", model.clone()))
                .bind(("dim", dims as i64))
                .bind(("ts", ts))
                .await?;
            edges_updated += 1;
            edge_remaining = edge_remaining.saturating_sub(1);
        }

        if rows.len() < take {
            break;
        }
    }
    println!(
        "[kg_embed] Edges: updated={}, skipped={}, missing(null/none/empty)={}/{}/{}",
        edges_updated,
        edges_skipped,
        edges_missing_null,
        edges_missing_none,
        edges_missing_empty
    );

    Ok(KgEmbedStats {
        expected_dim: dims,
        provider: prov,
        model,
        dry_run,
        entities_updated,
        entities_skipped,
        observations_updated,
        observations_skipped,
        edges_updated,
        edges_skipped,
    })
}
