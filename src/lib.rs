pub mod bge_embedder;
pub mod cognitive;
pub mod config;
pub mod deserializers;
pub mod embeddings;
pub mod error;
pub mod kg_extractor;
pub mod schemas;
pub mod serializers;
pub mod server;
pub mod tools;

use anyhow::Result;
use reqwest::Client;

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

    // HTTP SQL client
    let host = config.system.database_url.clone();
    let http_base = if host.starts_with("http") {
        host
    } else {
        format!("http://{}", host)
    };
    let sql_url = format!("{}/sql", http_base.trim_end_matches('/'));
    let user = config.runtime.database_user.clone();
    let pass = config.runtime.database_pass.clone();
    let ns = config.system.database_ns.clone();
    let dbname = config.system.database_db.clone();
    let http = Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;

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
            "USE NS {} DB {}; SELECT id, content, created_at, array::len(embedding) AS elen FROM thoughts ORDER BY created_at ASC LIMIT {} START {};",
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
            .get(1)
            .and_then(|b| b.get("result"))
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();
        if result.is_empty() {
            break;
        }

        for item in result.iter() {
            let id_raw = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let mut parts = id_raw.splitn(2, ':');
            let tb = parts.next().unwrap_or("thoughts");
            let inner = parts.next().unwrap_or("").trim();
            let inner = inner.trim_start_matches('⟨').trim_end_matches('⟩');
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
                "USE NS {} DB {}; UPDATE type::thing('{}', '{}') SET embedding = {}, embedding_provider = '{}', embedding_model = '{}', embedding_dim = {}, embedded_at = time::now() RETURN NONE;",
                ns, dbname, tb, inner, emb_json, provider, model, expected_dim
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
