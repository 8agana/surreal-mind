pub mod embeddings;

use anyhow::Result;
use reqwest::Client;
use serde::Deserialize as _;
use std::path::Path;

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

// Load env from a deterministic set of locations so all binaries read the same file
pub fn load_env() {
    // 1) Explicit path wins
    if let Ok(p) = std::env::var("SURR_ENV_FILE") {
        let _ = dotenvy::from_filename(&p);
        return;
    }
    // 2) Current working directory .env (Surrealist, MCP, etc.)
    if Path::new(".env").exists() {
        let _ = dotenvy::from_filename(".env");
        return;
    }
    // 3) Crate directory .env (when running from target/release)
    let crate_env = concat!(env!("CARGO_MANIFEST_DIR"), "/.env");
    if Path::new(crate_env).exists() {
        let _ = dotenvy::from_filename(crate_env);
        return;
    }
    // 4) Project root .env (parent of crate dir)
    let parent_env = concat!(env!("CARGO_MANIFEST_DIR"), "/../.env");
    if Path::new(parent_env).exists() {
        let _ = dotenvy::from_filename(parent_env);
        return;
    }
    // 5) Fallback to dotenv search walk
    let _ = dotenvy::dotenv();
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Thought {
    #[serde(
        serialize_with = "serialize_as_string",
        deserialize_with = "deserialize_thing_or_string"
    )]
    id: String,
    content: String,
    created_at: surrealdb::sql::Datetime,
    embedding: Vec<f32>,
    injected_memories: Vec<String>,
    enriched_content: Option<String>,
    injection_scale: u8,
    significance: f32,
    access_count: u32,
    last_accessed: Option<surrealdb::sql::Datetime>,
    #[serde(default)]
    submode: Option<String>,
    #[serde(default)]
    framework_enhanced: Option<bool>,
    #[serde(default)]
    framework_analysis: Option<serde_json::Value>,
    #[serde(default)]
    is_inner_voice: Option<bool>,
    #[serde(default)]
    inner_visibility: Option<String>,
}

pub async fn run_reembed(
    batch_size: usize,
    limit: Option<usize>,
    missing_only: bool,
    dry_run: bool,
) -> Result<ReembedStats> {
    // HTTP SQL client
    let host = std::env::var("SURR_DB_URL").unwrap_or_else(|_| "127.0.0.1:8000".to_string());
    let http_base = if host.starts_with("http") {
        host
    } else {
        format!("http://{}", host)
    };
    let sql_url = format!("{}/sql", http_base.trim_end_matches('/'));
    let user = std::env::var("SURR_DB_USER").unwrap_or_else(|_| "root".to_string());
    let pass = std::env::var("SURR_DB_PASS").unwrap_or_else(|_| "root".to_string());
    let ns = std::env::var("SURR_DB_NS").unwrap_or_else(|_| "surreal_mind".to_string());
    let dbname = std::env::var("SURR_DB_DB").unwrap_or_else(|_| "consciousness".to_string());
    let http = Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;

    // Embedder
    let embedder = embeddings::create_embedder().await?;
    let expected_dim = embedder.dimensions();

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
                "USE NS {} DB {}; UPDATE type::thing('{}', '{}') SET embedding = {} RETURN NONE;",
                ns, dbname, tb, inner, emb_json
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

// Custom serializer for String (ensures it's always serialized as a plain string)
pub fn serialize_as_string<S>(id: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(id)
}

// Custom deserializer for Thing or String
pub fn deserialize_thing_or_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let value = serde_json::Value::deserialize(deserializer)?;

    match value {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(tb)) = map.get("tb") {
                if let Some(id_val) = map.get("id") {
                    match id_val {
                        serde_json::Value::String(id) => Ok(format!("{}:{}", tb, id)),
                        serde_json::Value::Object(id_obj) => {
                            if let Some(serde_json::Value::String(id_str)) = id_obj.get("String") {
                                Ok(format!("{}:{}", tb, id_str))
                            } else {
                                Ok(format!(
                                    "{}:{}",
                                    tb,
                                    serde_json::to_string(id_val).unwrap_or_default()
                                ))
                            }
                        }
                        _ => Ok(format!(
                            "{}:{}",
                            tb,
                            serde_json::to_string(id_val).unwrap_or_default()
                        )),
                    }
                } else {
                    Err(D::Error::custom("Thing object missing 'id' field"))
                }
            } else {
                Err(D::Error::custom("Expected Thing object or string"))
            }
        }
        _ => Err(D::Error::custom("Expected Thing object or string")),
    }
}
