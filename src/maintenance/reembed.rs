//! Re-embedding functions for thoughts and knowledge graph entities.
//!
//! This module provides functions to re-embed existing records in the database,
//! including thoughts (via run_reembed), knowledge graph entities/observations/edges
//! (via run_reembed_kg), and missing-only embedding (via run_kg_embed).

use anyhow::Result;
use std::future::IntoFuture;

/// The result of examining an UPDATE statement's `RETURN` rows. SurrealDB can
/// return a transport-successful response that contains a statement error, so
/// callers must classify the statement result separately from `.await`.
#[derive(Debug, PartialEq, Eq)]
enum EmbeddingUpdateOutcome {
    Updated,
    NoMatch,
    TransportError(String),
    StatementError(String),
}

fn classify_embedding_update_rows<T, E: std::fmt::Display>(
    rows: std::result::Result<Vec<T>, E>,
) -> EmbeddingUpdateOutcome {
    match rows {
        Ok(rows) if rows.is_empty() => EmbeddingUpdateOutcome::NoMatch,
        Ok(_) => EmbeddingUpdateOutcome::Updated,
        Err(error) => EmbeddingUpdateOutcome::StatementError(error.to_string()),
    }
}

/// Execute and classify one generated-vector UPDATE. All six KG batch paths
/// use this helper so a future edit cannot quietly reintroduce the old
/// `await?`/`take(0)?` abort semantics in just one table variant.
async fn execute_embedding_update<F, E>(update: F) -> EmbeddingUpdateOutcome
where
    F: IntoFuture<Output = std::result::Result<surrealdb::IndexedResults, E>>,
    E: std::fmt::Display,
{
    match update.into_future().await {
        Ok(mut response) => {
            classify_embedding_update_rows(response.take::<Vec<serde_json::Value>>(0))
        }
        Err(error) => EmbeddingUpdateOutcome::TransportError(error.to_string()),
    }
}

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
    /// N-5: count of per-row UPDATE statements that returned HTTP success but
    /// matched zero rows (e.g. an identity mismatch between the WHERE clause and
    /// the stored record id). These are NOT counted in `updated`.
    pub no_match: usize,
    /// Per-row embedding, transport, dimension-guard, or statement failures.
    /// These are distinct from a successfully executed UPDATE that matched no
    /// row, and neither outcome is counted as `updated`.
    pub failed: usize,
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
    /// Same false-success class as ReembedStats::no_match (N-5): a per-record
    /// UPDATE that returned HTTP/driver success but matched zero rows (e.g.
    /// the record was deleted between the SELECT and the UPDATE). NOT counted
    /// in the corresponding `*_updated` field.
    pub entities_no_match: usize,
    pub observations_no_match: usize,
    pub edges_no_match: usize,
    pub entities_failed: usize,
    pub observations_failed: usize,
    pub edges_failed: usize,
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
    /// Same false-success class as ReembedStats::no_match (N-5): the idempotent
    /// WHERE-gated UPDATE returned success but matched zero rows (e.g. a
    /// concurrent writer already cleared the NULL/NONE condition). NOT
    /// counted in the corresponding `*_updated` field.
    pub entities_no_match: usize,
    pub observations_no_match: usize,
    pub edges_no_match: usize,
    pub entities_failed: usize,
    pub observations_failed: usize,
    pub edges_failed: usize,
}

pub async fn run_reembed(
    batch_size: usize,
    limit: Option<usize>,
    missing_only: bool,
    dry_run: bool,
) -> Result<ReembedStats> {
    // Load configuration
    let config = crate::config::Config::load()?;

    // HTTP SQL client using centralized utility
    let http_config = crate::utils::HttpSqlConfig::from_config(&config, "reembed");
    let sql_url = http_config.sql_url();
    let user = http_config.username.clone();
    let pass = http_config.password.clone();
    let ns = http_config.namespace.clone();
    let dbname = http_config.database.clone();
    let http = http_config.build_client()?;

    // Embedder
    let embedder = crate::embeddings::create_embedder(&config).await?;
    let expected_dim = embedder.dimensions();
    let provider = config.system.embedding_provider.clone();
    let model = config.system.embedding_model.clone();

    let mut start: usize = 0;
    let mut processed: usize = 0;
    let mut updated: usize = 0;
    let mut skipped: usize = 0;
    let mut mismatched: usize = 0;
    let mut missing: usize = 0;
    let mut no_match: usize = 0;
    let mut failed: usize = 0;
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
            let new_emb = match embedder.embed(&content).await {
                Ok(embedding) => embedding,
                Err(error) => {
                    failed += 1;
                    eprintln!(
                        "  ⚠️  reembed: embedding failed for id={}: {}",
                        id_raw, error
                    );
                    processed += 1;
                    continue;
                }
            };
            if let Err(error) =
                crate::embeddings::ensure_generated_embedding_dimension(&new_emb, expected_dim)
            {
                failed += 1;
                eprintln!(
                    "  ⚠️  reembed: refusing wrong-dimension embedding for id={}: {}",
                    id_raw, error
                );
                processed += 1;
                continue;
            }
            let emb_json = serde_json::to_string(&new_emb)?;
            // N-5 fix: `id = '{}'` compared a bare string against a Thing and
            // never matched (measured live: 0 rows affected). type::record()
            // reconstructs the typed record identity from the meta::id() key
            // returned by the SELECT above. RETURN meta::id(id) AS id (instead
            // of RETURN NONE) makes the match observable so a 0-row match no
            // longer gets silently counted as a success.
            let update_sql = format!(
                "USE NS {} DB {}; UPDATE thoughts SET embedding = {}, embedding_provider = '{}', embedding_model = '{}', embedding_dim = {}, embedded_at = time::now() WHERE id = type::record('thoughts', '{}') RETURN meta::id(id) AS id;",
                ns, dbname, emb_json, provider, model, expected_dim, id_raw
            );
            let uresp = match http
                .post(&sql_url)
                .basic_auth(&user, Some(&pass))
                .header("Accept", "application/json")
                .header("Content-Type", "application/surrealql")
                .body(update_sql)
                .send()
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    failed += 1;
                    eprintln!(
                        "  ⚠️  reembed: UPDATE transport failed for id={}: {}",
                        id_raw, error
                    );
                    processed += 1;
                    continue;
                }
            };
            if !uresp.status().is_success() {
                failed += 1;
                eprintln!(
                    "  ⚠️  reembed: UPDATE transport status failed for id={}: {}",
                    id_raw,
                    uresp.text().await.unwrap_or_default()
                );
                processed += 1;
                continue;
            }
            // A per-statement error can still ride back inside an HTTP 200 body
            // (SurrealDB's /sql endpoint), and a WHERE clause that matches zero
            // rows returns success with an empty result array either way. Only
            // count `updated` when the UPDATE's own result block is a non-empty
            // array of rows.
            let update_blocks: serde_json::Value = match uresp.json().await {
                Ok(value) => value,
                Err(error) => {
                    failed += 1;
                    eprintln!(
                        "  ⚠️  reembed: UPDATE response parse failed for id={}: {}",
                        id_raw, error
                    );
                    processed += 1;
                    continue;
                }
            };
            let update_result = update_blocks.as_array().and_then(|arr| {
                arr.iter()
                    .find_map(|b| b.get("result").and_then(|r| r.as_array()).cloned())
            });
            let update_status_ok = update_blocks
                .as_array()
                .and_then(|arr| arr.last())
                .and_then(|b| b.get("status"))
                .and_then(|s| s.as_str())
                == Some("OK");
            let matched = update_status_ok
                && update_result
                    .as_ref()
                    .map(|rows| !rows.is_empty())
                    .unwrap_or(false);

            if matched {
                if cur_len == 0 {
                    missing += 1;
                } else if cur_len != expected_dim {
                    mismatched += 1;
                }
                updated += 1;
            } else if update_status_ok {
                no_match += 1;
                eprintln!(
                    "  ⚠️  reembed: UPDATE for id={} matched 0 rows; not counted as updated",
                    id_raw
                );
            } else {
                failed += 1;
                eprintln!(
                    "  ⚠️  reembed: UPDATE statement failed for id={}; not counted as updated",
                    id_raw
                );
            }
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
        no_match,
        failed,
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
    let embedder = crate::embeddings::create_embedder(&config).await?;
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
        username: user.clone(),
        password: pass.clone(),
    })
    .await?;
    db.use_ns(&ns).use_db(&dbname).await?;

    let mut updated_entities = 0usize;
    let mut skipped_entities = 0usize;
    let mut mismatched_entities = 0usize;
    let mut missing_entities = 0usize;
    let mut no_match_entities = 0usize;
    let mut failed_entities = 0usize;
    let mut updated_obs = 0usize;
    let mut skipped_obs = 0usize;
    let mut mismatched_obs = 0usize;
    let mut missing_obs = 0usize;
    let mut no_match_obs = 0usize;
    let mut failed_obs = 0usize;
    let mut updated_edges = 0usize;
    let mut skipped_edges = 0usize;
    let mut mismatched_edges = 0usize;
    let mut missing_edges = 0usize;
    let mut no_match_edges = 0usize;
    let mut failed_edges = 0usize;

    // Entities
    {
        let sql = match limit {
            Some(l) => format!("SELECT meta::id(id) as id, name, data, entity_type, (IF type::is_array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len, embedding_model FROM kg_entities LIMIT {}", l),
            None => "SELECT meta::id(id) as id, name, data, entity_type, (IF type::is_array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len, embedding_model FROM kg_entities".to_string(),
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
            let emb = match embedder.embed(&text).await {
                Ok(embedding) => embedding,
                Err(error) => {
                    failed_entities += 1;
                    eprintln!(
                        "  ⚠️  reembed_kg: embedding failed for kg_entities:{}: {}",
                        id, error
                    );
                    continue;
                }
            };
            if let Err(error) = crate::embeddings::ensure_generated_embedding_dimension(&emb, dims)
            {
                failed_entities += 1;
                eprintln!(
                    "  ⚠️  reembed_kg: refusing wrong-dimension embedding for kg_entities:{}: {}",
                    id, error
                );
                continue;
            }
            if !dry_run {
                let ts = Utc::now().to_rfc3339();
                // Same false-success class as N-5 in run_reembed above: verify
                // the UPDATE actually matched the record instead of trusting
                // driver-level success alone (the record may have been deleted
                // between the SELECT and this UPDATE).
                let q = format!(
                    "UPDATE kg_entities:`{}` SET embedding = $emb, embedding_provider = $prov, embedding_model = $model, embedding_dim = $dim, embedded_at = $ts RETURN meta::id(id) AS id",
                    id
                );
                match execute_embedding_update(
                    db.query(q)
                        .bind(("emb", emb))
                        .bind(("prov", prov.clone()))
                        .bind(("model", model.clone()))
                        .bind(("dim", dims as i64))
                        .bind(("ts", ts)),
                )
                .await
                {
                    EmbeddingUpdateOutcome::Updated => {}
                    EmbeddingUpdateOutcome::NoMatch => {
                        no_match_entities += 1;
                        eprintln!(
                            "  ⚠️  reembed_kg: UPDATE for kg_entities:{} matched 0 rows; not counted as updated",
                            id
                        );
                        continue;
                    }
                    EmbeddingUpdateOutcome::TransportError(error) => {
                        failed_entities += 1;
                        eprintln!(
                            "  ⚠️  reembed_kg: UPDATE transport failed for kg_entities:{}: {}",
                            id, error
                        );
                        continue;
                    }
                    EmbeddingUpdateOutcome::StatementError(error) => {
                        failed_entities += 1;
                        eprintln!(
                            "  ⚠️  reembed_kg: UPDATE statement failed for kg_entities:{}: {}",
                            id, error
                        );
                        continue;
                    }
                }
            }
            updated_entities += 1;
        }
    }

    // Observations
    {
        let sql = match limit {
            Some(l) => format!("SELECT meta::id(id) as id, name, data, (IF type::is_array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len, embedding_model FROM kg_observations LIMIT {}", l),
            None => "SELECT meta::id(id) as id, name, data, (IF type::is_array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len, embedding_model FROM kg_observations".to_string(),
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
            let emb = match embedder.embed(&text).await {
                Ok(embedding) => embedding,
                Err(error) => {
                    failed_obs += 1;
                    eprintln!(
                        "  ⚠️  reembed_kg: embedding failed for kg_observations:{}: {}",
                        id, error
                    );
                    continue;
                }
            };
            if let Err(error) = crate::embeddings::ensure_generated_embedding_dimension(&emb, dims)
            {
                failed_obs += 1;
                eprintln!(
                    "  ⚠️  reembed_kg: refusing wrong-dimension embedding for kg_observations:{}: {}",
                    id, error
                );
                continue;
            }
            if !dry_run {
                let ts = Utc::now().to_rfc3339();
                let q = format!(
                    "UPDATE kg_observations:`{}` SET embedding = $emb, embedding_provider = $prov, embedding_model = $model, embedding_dim = $dim, embedded_at = $ts RETURN meta::id(id) AS id",
                    id
                );
                match execute_embedding_update(
                    db.query(q)
                        .bind(("emb", emb))
                        .bind(("prov", prov.clone()))
                        .bind(("model", model.clone()))
                        .bind(("dim", dims as i64))
                        .bind(("ts", ts)),
                )
                .await
                {
                    EmbeddingUpdateOutcome::Updated => {}
                    EmbeddingUpdateOutcome::NoMatch => {
                        no_match_obs += 1;
                        eprintln!(
                            "  ⚠️  reembed_kg: UPDATE for kg_observations:{} matched 0 rows; not counted as updated",
                            id
                        );
                        continue;
                    }
                    EmbeddingUpdateOutcome::TransportError(error) => {
                        failed_obs += 1;
                        eprintln!(
                            "  ⚠️  reembed_kg: UPDATE transport failed for kg_observations:{}: {}",
                            id, error
                        );
                        continue;
                    }
                    EmbeddingUpdateOutcome::StatementError(error) => {
                        failed_obs += 1;
                        eprintln!(
                            "  ⚠️  reembed_kg: UPDATE statement failed for kg_observations:{}: {}",
                            id, error
                        );
                        continue;
                    }
                }
            }
            updated_obs += 1;
        }
    }

    // Edges
    {
        let sql = match limit {
            Some(l) => format!("SELECT meta::id(id) as id, source.name as source_name, target.name as target_name, rel_type, data, (IF type::is_array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len, embedding_model FROM kg_edges LIMIT {}", l),
            None => "SELECT meta::id(id) as id, source.name as source_name, target.name as target_name, rel_type, data, (IF type::is_array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len, embedding_model FROM kg_edges".to_string(),
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

            let emb = match embedder.embed(&text).await {
                Ok(embedding) => embedding,
                Err(error) => {
                    failed_edges += 1;
                    eprintln!(
                        "  ⚠️  reembed_kg: embedding failed for kg_edges:{}: {}",
                        id, error
                    );
                    continue;
                }
            };
            if let Err(error) = crate::embeddings::ensure_generated_embedding_dimension(&emb, dims)
            {
                failed_edges += 1;
                eprintln!(
                    "  ⚠️  reembed_kg: refusing wrong-dimension embedding for kg_edges:{}: {}",
                    id, error
                );
                continue;
            }
            if !dry_run {
                let ts = Utc::now().to_rfc3339();
                let q = format!(
                    "UPDATE kg_edges:`{}` SET embedding = $emb, embedding_provider = $prov, embedding_model = $model, embedding_dim = $dim, embedded_at = $ts RETURN meta::id(id) AS id",
                    id
                );
                match execute_embedding_update(
                    db.query(q)
                        .bind(("emb", emb))
                        .bind(("prov", prov.clone()))
                        .bind(("model", model.clone()))
                        .bind(("dim", dims as i64))
                        .bind(("ts", ts)),
                )
                .await
                {
                    EmbeddingUpdateOutcome::Updated => {}
                    EmbeddingUpdateOutcome::NoMatch => {
                        no_match_edges += 1;
                        eprintln!(
                            "  ⚠️  reembed_kg: UPDATE for kg_edges:{} matched 0 rows; not counted as updated",
                            id
                        );
                        continue;
                    }
                    EmbeddingUpdateOutcome::TransportError(error) => {
                        failed_edges += 1;
                        eprintln!(
                            "  ⚠️  reembed_kg: UPDATE transport failed for kg_edges:{}: {}",
                            id, error
                        );
                        continue;
                    }
                    EmbeddingUpdateOutcome::StatementError(error) => {
                        failed_edges += 1;
                        eprintln!(
                            "  ⚠️  reembed_kg: UPDATE statement failed for kg_edges:{}: {}",
                            id, error
                        );
                        continue;
                    }
                }
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
        entities_no_match: no_match_entities,
        entities_failed: failed_entities,
        observations_updated: updated_obs,
        observations_skipped: skipped_obs,
        observations_missing: missing_obs,
        observations_mismatched: mismatched_obs,
        observations_no_match: no_match_obs,
        observations_failed: failed_obs,
        edges_updated: updated_edges,
        edges_skipped: skipped_edges,
        edges_missing: missing_edges,
        edges_mismatched: mismatched_edges,
        edges_no_match: no_match_edges,
        edges_failed: failed_edges,
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
    let embedder = crate::embeddings::create_embedder(&config).await?;
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
        username: user.clone(),
        password: pass.clone(),
    })
    .await?;
    db.use_ns(&ns).use_db(&dbname).await?;

    let mut entities_updated = 0usize;
    let entities_skipped = 0usize;
    let mut entities_no_match = 0usize;
    let mut entities_failed = 0usize;
    let mut observations_updated = 0usize;
    let observations_skipped = 0usize;
    let mut observations_no_match = 0usize;
    let mut observations_failed = 0usize;
    let mut edges_updated = 0usize;
    let edges_skipped = 0usize;
    let mut edges_no_match = 0usize;
    let mut edges_failed = 0usize;

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
                (IF type::is_array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len \
             FROM kg_entities \
             WHERE (embedding IS NULL OR embedding IS NONE OR (type::is_array(embedding) AND array::len(embedding) = 0)) \
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

            let emb = match embedder.embed(&text).await {
                Ok(embedding) => embedding,
                Err(error) => {
                    entities_failed += 1;
                    eprintln!(
                        "  ⚠️  kg_embed: embedding failed for kg_entities:{}: {}",
                        id, error
                    );
                    entity_remaining = entity_remaining.saturating_sub(1);
                    continue;
                }
            };
            if let Err(error) = crate::embeddings::ensure_generated_embedding_dimension(&emb, dims)
            {
                entities_failed += 1;
                eprintln!(
                    "  ⚠️  kg_embed: refusing wrong-dimension embedding for kg_entities:{}: {}",
                    id, error
                );
                entity_remaining = entity_remaining.saturating_sub(1);
                continue;
            }
            let ts = Utc::now().to_rfc3339();

            // Idempotent update: only update if embedding is still NULL.
            // Same false-success class as N-5/run_reembed_kg: RETURN NONE gave
            // nothing to verify a zero-row match against (e.g. a concurrent
            // writer already cleared the NULL/NONE condition between the
            // SELECT and this UPDATE), so the row was always counted as
            // updated even when it wasn't touched.
            let q = format!(
                "UPDATE kg_entities:`{}` SET embedding = $emb, embedding_provider = $prov, embedding_model = $model, embedding_dim = $dim, embedded_at = $ts \
                 WHERE (embedding IS NULL OR embedding IS NONE OR (type::is_array(embedding) AND array::len(embedding) = 0)) RETURN meta::id(id) AS id",
                id
            );
            match execute_embedding_update(
                db.query(q)
                    .bind(("emb", emb))
                    .bind(("prov", prov.clone()))
                    .bind(("model", model.clone()))
                    .bind(("dim", dims as i64))
                    .bind(("ts", ts)),
            )
            .await
            {
                EmbeddingUpdateOutcome::Updated => entities_updated += 1,
                EmbeddingUpdateOutcome::NoMatch => {
                    entities_no_match += 1;
                    eprintln!(
                        "  ⚠️  kg_embed: UPDATE for kg_entities:{} matched 0 rows; not counted as updated",
                        id
                    );
                }
                EmbeddingUpdateOutcome::TransportError(error) => {
                    entities_failed += 1;
                    eprintln!(
                        "  ⚠️  kg_embed: UPDATE transport failed for kg_entities:{}: {}",
                        id, error
                    );
                }
                EmbeddingUpdateOutcome::StatementError(error) => {
                    entities_failed += 1;
                    eprintln!(
                        "  ⚠️  kg_embed: UPDATE statement failed for kg_entities:{}: {}",
                        id, error
                    );
                }
            }
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
                (IF type::is_array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len \
             FROM kg_observations \
             WHERE (embedding IS NULL OR embedding IS NONE OR (type::is_array(embedding) AND array::len(embedding) = 0)) \
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

            let emb = match embedder.embed(text).await {
                Ok(embedding) => embedding,
                Err(error) => {
                    observations_failed += 1;
                    eprintln!(
                        "  ⚠️  kg_embed: embedding failed for kg_observations:{}: {}",
                        id, error
                    );
                    obs_remaining = obs_remaining.saturating_sub(1);
                    continue;
                }
            };
            if let Err(error) = crate::embeddings::ensure_generated_embedding_dimension(&emb, dims)
            {
                observations_failed += 1;
                eprintln!(
                    "  ⚠️  kg_embed: refusing wrong-dimension embedding for kg_observations:{}: {}",
                    id, error
                );
                obs_remaining = obs_remaining.saturating_sub(1);
                continue;
            }
            let ts = Utc::now().to_rfc3339();

            let q = format!(
                "UPDATE kg_observations:`{}` SET embedding = $emb, embedding_provider = $prov, embedding_model = $model, embedding_dim = $dim, embedded_at = $ts \
                 WHERE (embedding IS NOT DEFINED OR embedding IS NULL OR embedding IS NONE OR (type::is_array(embedding) AND array::len(embedding) = 0)) RETURN meta::id(id) AS id",
                id
            );
            match execute_embedding_update(
                db.query(q)
                    .bind(("emb", emb))
                    .bind(("prov", prov.clone()))
                    .bind(("model", model.clone()))
                    .bind(("dim", dims as i64))
                    .bind(("ts", ts)),
            )
            .await
            {
                EmbeddingUpdateOutcome::Updated => observations_updated += 1,
                EmbeddingUpdateOutcome::NoMatch => {
                    observations_no_match += 1;
                    eprintln!(
                        "  ⚠️  kg_embed: UPDATE for kg_observations:{} matched 0 rows; not counted as updated",
                        id
                    );
                }
                EmbeddingUpdateOutcome::TransportError(error) => {
                    observations_failed += 1;
                    eprintln!(
                        "  ⚠️  kg_embed: UPDATE transport failed for kg_observations:{}: {}",
                        id, error
                    );
                }
                EmbeddingUpdateOutcome::StatementError(error) => {
                    observations_failed += 1;
                    eprintln!(
                        "  ⚠️  kg_embed: UPDATE statement failed for kg_observations:{}: {}",
                        id, error
                    );
                }
            }
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
                (IF type::is_array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len \
             FROM kg_edges \
             WHERE (embedding IS NULL OR embedding IS NONE OR (type::is_array(embedding) AND array::len(embedding) = 0)) \
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

            let emb = match embedder.embed(&text).await {
                Ok(embedding) => embedding,
                Err(error) => {
                    edges_failed += 1;
                    eprintln!(
                        "  ⚠️  kg_embed: embedding failed for kg_edges:{}: {}",
                        id, error
                    );
                    edge_remaining = edge_remaining.saturating_sub(1);
                    continue;
                }
            };
            if let Err(error) = crate::embeddings::ensure_generated_embedding_dimension(&emb, dims)
            {
                edges_failed += 1;
                eprintln!(
                    "  ⚠️  kg_embed: refusing wrong-dimension embedding for kg_edges:{}: {}",
                    id, error
                );
                edge_remaining = edge_remaining.saturating_sub(1);
                continue;
            }
            let ts = Utc::now().to_rfc3339();

            let q = format!(
                "UPDATE kg_edges:`{}` SET embedding = $emb, embedding_provider = $prov, embedding_model = $model, embedding_dim = $dim, embedded_at = $ts \
                 WHERE (embedding IS NOT DEFINED OR embedding IS NULL OR embedding IS NONE OR (type::is_array(embedding) AND array::len(embedding) = 0)) RETURN meta::id(id) AS id",
                id
            );
            match execute_embedding_update(
                db.query(q)
                    .bind(("emb", emb))
                    .bind(("prov", prov.clone()))
                    .bind(("model", model.clone()))
                    .bind(("dim", dims as i64))
                    .bind(("ts", ts)),
            )
            .await
            {
                EmbeddingUpdateOutcome::Updated => edges_updated += 1,
                EmbeddingUpdateOutcome::NoMatch => {
                    edges_no_match += 1;
                    eprintln!(
                        "  ⚠️  kg_embed: UPDATE for kg_edges:{} matched 0 rows; not counted as updated",
                        id
                    );
                }
                EmbeddingUpdateOutcome::TransportError(error) => {
                    edges_failed += 1;
                    eprintln!(
                        "  ⚠️  kg_embed: UPDATE transport failed for kg_edges:{}: {}",
                        id, error
                    );
                }
                EmbeddingUpdateOutcome::StatementError(error) => {
                    edges_failed += 1;
                    eprintln!(
                        "  ⚠️  kg_embed: UPDATE statement failed for kg_edges:{}: {}",
                        id, error
                    );
                }
            }
            edge_remaining = edge_remaining.saturating_sub(1);
        }

        if rows.len() < take {
            break;
        }
    }
    println!(
        "[kg_embed] Edges: updated={}, skipped={}, missing(null/none/empty)={}/{}/{}",
        edges_updated, edges_skipped, edges_missing_null, edges_missing_none, edges_missing_empty
    );

    Ok(KgEmbedStats {
        expected_dim: dims,
        provider: prov,
        model,
        dry_run,
        entities_updated,
        entities_skipped,
        entities_no_match,
        entities_failed,
        observations_updated,
        observations_skipped,
        observations_no_match,
        observations_failed,
        edges_updated,
        edges_skipped,
        edges_no_match,
        edges_failed,
    })
}

#[cfg(test)]
mod tests {
    use super::{EmbeddingUpdateOutcome, classify_embedding_update_rows, execute_embedding_update};

    #[tokio::test]
    async fn update_result_classifier_distinguishes_all_outcomes() {
        assert_eq!(
            classify_embedding_update_rows::<(), &str>(Ok(vec![()])),
            EmbeddingUpdateOutcome::Updated
        );
        assert_eq!(
            classify_embedding_update_rows::<(), &str>(Ok(vec![])),
            EmbeddingUpdateOutcome::NoMatch
        );
        assert_eq!(
            classify_embedding_update_rows::<(), _>(Err("synthetic statement failure")),
            EmbeddingUpdateOutcome::StatementError("synthetic statement failure".to_string())
        );
        assert_eq!(
            execute_embedding_update(async {
                Err::<surrealdb::IndexedResults, _>("synthetic transport failure")
            })
            .await,
            EmbeddingUpdateOutcome::TransportError("synthetic transport failure".to_string())
        );
    }

    /// Real-driver negative witness for the shared classifier used by all six
    /// KG batch writers. A statement error must be surfaced without ending
    /// subsequent update classification, while a successful statement that
    /// matches no row remains distinct from failure.
    #[cfg(feature = "db_integration")]
    #[tokio::test]
    async fn update_result_classifier_continues_after_real_statement_error() -> anyhow::Result<()> {
        if std::env::var("RUN_DB_TESTS").is_err()
            || std::env::var("REEMBED_TEST_CONFIRM_DISPOSABLE_NS").is_err()
        {
            return Ok(());
        }

        let config = crate::config::Config::load()?;
        if matches!(
            config.system.database_ns.as_str(),
            "surreal_mind" | "surreal-mind"
        ) {
            anyhow::bail!(
                "update classifier db test requires a disposable namespace, not {:?}",
                config.system.database_ns
            );
        }
        let server = crate::server::SurrealMindServer::new(&config).await?;
        let marker = "__rmcp-sol-update-classifier-negative__";

        server
            .db
            .query(
                "CREATE thoughts SET content = $marker, embedding = NONE, \
                 embedding_provider = 'fixture', embedding_model = 'fixture', embedding_dim = 0, \
                 embedding_status = 'pending', created_at = time::now(), injected_memories = [], \
                 injection_scale = 0, significance = 0.0, access_count = 0",
            )
            .bind(("marker", marker.to_string()))
            .await?
            .check()?;

        let statement_error = execute_embedding_update(
            server
                .db
                .query(
                    "UPDATE thoughts SET access_count = 'not-an-int' \
                 WHERE content = $marker RETURN meta::id(id) AS id",
                )
                .bind(("marker", marker.to_string())),
        )
        .await;
        assert!(matches!(
            statement_error,
            EmbeddingUpdateOutcome::StatementError(_)
        ));

        let no_match = execute_embedding_update(server.db.query(
            "UPDATE thoughts SET access_count = 1 \
                 WHERE content = '__rmcp-sol-update-classifier-absent__' \
                 RETURN meta::id(id) AS id",
        ))
        .await;
        assert_eq!(no_match, EmbeddingUpdateOutcome::NoMatch);

        let subsequent_success = execute_embedding_update(
            server
                .db
                .query(
                    "UPDATE thoughts SET access_count = 1 \
                 WHERE content = $marker RETURN meta::id(id) AS id",
                )
                .bind(("marker", marker.to_string())),
        )
        .await;
        assert_eq!(subsequent_success, EmbeddingUpdateOutcome::Updated);

        server
            .db
            .query("DELETE thoughts WHERE content = $marker")
            .bind(("marker", marker.to_string()))
            .await?
            .check()?;
        Ok(())
    }
}
