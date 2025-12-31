use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use anyhow::Context;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tracing::{info, warn};

impl SurrealMindServer {
    /// Create a new SurrealMind server instance
    pub async fn new(config: &crate::config::Config) -> Result<Self> {
        info!("Connecting to SurrealDB service via WebSocket");

        // Use the provided configuration directly instead of setting global env vars.
        // Embedder factory will read from the environment, but we keep the existing behaviour.

        // Normalize URL for SurrealDB Ws engine (expects host:port, no scheme)
        fn normalize_ws_url(s: &str) -> String {
            s.strip_prefix("ws://")
                .or_else(|| s.strip_prefix("wss://"))
                .or_else(|| s.strip_prefix("http://"))
                .or_else(|| s.strip_prefix("https://"))
                .unwrap_or(s)
                .to_string()
        }

        // Connect to SurrealDB instance
        // DB connection values from config
        let url = normalize_ws_url(&config.system.database_url);
        let user = &config.runtime.database_user;
        let pass = &config.runtime.database_pass;
        let ns = &config.system.database_ns;
        let dbname = &config.system.database_db;

        // Optional reconnection strategy with backoff
        let db_reconnect_enabled = std::env::var("SURR_DB_RECONNECT")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let max_retries = if db_reconnect_enabled { 5 } else { 0 };

        let mut db = None;
        for attempt in 0..=max_retries {
            match surrealdb::Surreal::new::<surrealdb::engine::remote::ws::Ws>(url.clone()).await {
                Ok(conn) => {
                    db = Some(conn);
                    if attempt > 0 {
                        info!(
                            "Successfully reconnected to SurrealDB after {} attempts",
                            attempt + 1
                        );
                    }
                    break;
                }
                Err(e) => {
                    if attempt == max_retries {
                        return Err(SurrealMindError::Database {
                            message: format!(
                                "Failed to connect to SurrealDB at {} after {} attempts: {}",
                                config.system.database_url,
                                max_retries + 1,
                                e
                            ),
                        });
                    } else {
                        let delay_ms = (1000 * (1u64 << attempt.min(5))).min(60000); // 1s, 2s, 4s, 8s, 16s, then 60s max
                        warn!(
                            "SurrealDB connection attempt {} failed: {}. Retrying in {}ms...",
                            attempt + 1,
                            e,
                            delay_ms
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    }
                }
            }
        }

        let db = db.expect("database should be initialized");

        // Sign in with credentials
        db.signin(surrealdb::opt::auth::Root {
            username: user.as_str(),
            password: pass.as_str(),
        })
        .await
        .with_context(|| format!("Failed to authenticate with SurrealDB as user '{}'", user))?;

        // Select namespace and database
        db.use_ns(ns)
            .await
            .with_context(|| format!("Failed to select namespace '{}'", ns))?;

        db.use_db(dbname)
            .await
            .with_context(|| format!("Failed to select database '{}'", dbname))?;

        // Initialize embedder
        let embedder = crate::embeddings::create_embedder(config)
            .await
            .context("Failed to create embedder")?;
        info!(
            "Embedder initialized with {} dimensions",
            embedder.dimensions()
        );

        // Initialize bounded in-memory cache (LRU)
        let cache_max: usize = std::env::var("SURR_CACHE_MAX")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(5000);
        let thoughts_cache =
            LruCache::new(NonZeroUsize::new(cache_max).unwrap_or(NonZeroUsize::MIN));

        // Initialize job semaphore (limit concurrent async jobs)
        let job_concurrency_limit: usize = std::env::var("SURR_JOB_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(4);
        let job_semaphore = Arc::new(Semaphore::new(job_concurrency_limit));

        let server = Self {
            db: Arc::new(db),
            thoughts: Arc::new(RwLock::new(thoughts_cache)),
            embedder,
            config: Arc::new(config.clone()),
            job_semaphore,
        };

        server
            .initialize_schema()
            .await
            .map_err(|e| SurrealMindError::Mcp {
                message: e.message.to_string(),
            })?;

        if server.config.runtime.transport == "http" {
            let db = server.db.clone();
            let semaphore = server.job_semaphore.clone();
            tokio::spawn(async move {
                crate::tools::delegate_gemini::run_delegate_gemini_worker(db, semaphore).await;
            });
        }

        Ok(server)
    }

    /// Get embedding metadata for tracking model/provider info
    pub fn get_embedding_metadata(&self) -> (String, String, i64) {
        let provider = self.config.system.embedding_provider.clone();
        let model = self.config.system.embedding_model.clone();
        let dim = self.embedder.dimensions() as i64;
        (provider, model, dim)
    }

    /// Calculate cosine similarity between two vectors (delegates to utils)
    #[allow(dead_code)]
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        crate::utils::cosine_similarity(a, b)
    }

    /// Perform KG-only memory injection: find similar KG entities and attach their IDs.
    pub async fn inject_memories(
        &self,
        thought_id: &str,
        embedding: &[f32],
        injection_scale: i64,
        submode: Option<&str>,
        tool_name: Option<&str>,
    ) -> crate::error::Result<(usize, Option<String>)> {
        tracing::debug!("inject_memories: query embedding dims: {}", embedding.len());
        // Orbital mechanics: determine limit and threshold from scale
        let scale = injection_scale.clamp(0, 3) as u8;
        if scale == 0 {
            return Ok((0, None));
        }
        // Thresholds from config.retrieval.t1, with optional env override and warn
        let t1 = std::env::var("SURR_INJECT_T1")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or_else(|| {
                if std::env::var("SURR_INJECT_T1").is_ok() {
                    tracing::warn!("Using env override SURR_INJECT_T1");
                }
                self.config.retrieval.t1
            });
        let t2 = std::env::var("SURR_INJECT_T2")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or_else(|| {
                if std::env::var("SURR_INJECT_T2").is_ok() {
                    tracing::warn!("Using env override SURR_INJECT_T2");
                }
                self.config.retrieval.t2
            });
        let t3 = std::env::var("SURR_INJECT_T3")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or_else(|| {
                if std::env::var("SURR_INJECT_T3").is_ok() {
                    tracing::warn!("Using env override SURR_INJECT_T3");
                }
                self.config.retrieval.t3
            });
        let (limit, mut prox_thresh) = match scale {
            0 => (0usize, 1.0f32),
            1 => (5usize, t1),
            2 => (10usize, t2),
            _ => (20usize, t3),
        };
        if limit == 0 {
            return Ok((0, None));
        }

        // Optional: submode-aware retrieval tweaks
        // Use config flag, with optional env override and warn
        if std::env::var("SURR_SUBMODE_RETRIEVAL").ok().as_deref() == Some("true")
            || (std::env::var("SURR_SUBMODE_RETRIEVAL").is_err()
                && self.config.retrieval.submode_tuning)
        {
            if std::env::var("SURR_SUBMODE_RETRIEVAL").is_ok() {
                tracing::warn!("Using env override SURR_SUBMODE_RETRIEVAL");
            }
            if let Some(sm) = submode {
                // Use lightweight profile deltas to adjust similarity threshold
                use crate::cognitive::profile::{Submode, profile_for};
                let profile = profile_for(Submode::from_str(sm));
                let delta = profile.injection.threshold_delta;
                // Clamp within [0.0, 0.99]
                prox_thresh = (prox_thresh + delta).clamp(0.0, 0.99);
            }
        }
        // Candidate pool size from config, with optional env override and warn
        let mut retrieve = std::env::var("SURR_KG_CANDIDATES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or_else(|| {
                if std::env::var("SURR_KG_CANDIDATES").is_ok() {
                    tracing::warn!("Using env override SURR_KG_CANDIDATES");
                }
                self.config.retrieval.candidates
            });

        // Tool-specific runtime defaults (no behavior drift beyond thresholds)
        if let Some(tool) = tool_name {
            // Only adjust candidate pool size per tool; do not override thresholds here
            retrieve = match tool {
                "think_convo" => 500,
                "think_plan" => 800,
                "think_debug" => 1000,
                "think_build" => 400,
                "think_stuck" => 600,
                _ => retrieve,
            };
        }

        // Fetch candidate entities and observations (two statements to avoid UNION pitfalls)
        // Filter by embedding_dim to avoid dimension mismatches at the DB level
        let q_dim = embedding.len() as i64;
        let mut q = self
            .db
            .query(
                "SELECT meta::id(id) as id, name, data, embedding FROM kg_entities \
                 WHERE embedding_dim = $dim AND embedding IS NOT NULL LIMIT $lim; \
                 SELECT meta::id(id) as id, name, data, embedding FROM kg_observations \
                 WHERE embedding_dim = $dim AND embedding IS NOT NULL LIMIT $lim;",
            )
            .bind(("dim", q_dim))
            .bind(("lim", retrieve as i64))
            .await?;
        let mut rows: Vec<serde_json::Value> = q.take(0).unwrap_or_default();
        let mut rows2: Vec<serde_json::Value> = q.take(1).unwrap_or_default();
        let total_candidates = rows.len() + rows2.len();
        rows.append(&mut rows2);
        tracing::debug!(
            "inject_memories: Retrieved {} candidates from KG (entities+observations)",
            total_candidates
        );

        // Iterate, compute or reuse embeddings, score by cosine similarity
        let mut scored: Vec<(String, f32, String, String)> = Vec::new();
        let mut skipped = 0;
        for r in rows {
            if let Some(id) = r.get("id").and_then(|v| v.as_str()) {
                // Try to use existing embedding; compute and persist if missing and allowed
                let mut emb_opt: Option<Vec<f32>> = None;
                if let Some(ev) = r.get("embedding").and_then(|v| v.as_array()) {
                    let vecf: Vec<f32> = ev
                        .iter()
                        .filter_map(|x| x.as_f64())
                        .map(|f| f as f32)
                        .collect();
                    if vecf.len() == embedding.len() {
                        emb_opt = Some(vecf);
                    }
                }
                if emb_opt.is_none() {
                    // Build text for embedding: name + type or description
                    let name_s = r.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let mut text = name_s.to_string();
                    if let Some(d) = r.get("data").and_then(|v| v.as_object()) {
                        if let Some(etype) = d.get("entity_type").and_then(|v| v.as_str()) {
                            text = format!("{} ({})", name_s, etype);
                        } else if let Some(desc) = d.get("description").and_then(|v| v.as_str()) {
                            text.push_str(" - ");
                            text.push_str(desc);
                        }
                    }
                    let new_emb = self.embedder.embed(&text).await.unwrap_or_default();
                    if new_emb.len() == embedding.len() {
                        emb_opt = Some(new_emb.clone());
                        // Determine table from id (kg_entities or kg_observations)
                        let tb = if id.starts_with("kg_entities:") {
                            "kg_entities"
                        } else if id.starts_with("kg_observations:") {
                            "kg_observations"
                        } else {
                            "kg_entities" // fallback
                        };
                        let inner_id = id
                            .split(':')
                            .nth(1)
                            .unwrap_or(id)
                            .trim_start_matches('⟨')
                            .trim_end_matches('⟩');
                        // Persist embedding for future fast retrieval (best-effort)
                        let (provider, model, dim) = self.get_embedding_metadata();
                        let _ = self
                            .db
                            .query("UPDATE type::thing($tb, $id) SET embedding = $emb, embedding_provider = $provider, embedding_model = $model, embedding_dim = $dim, embedded_at = time::now() RETURN meta::id(id) as id")
                            .bind(("tb", tb))
                            .bind(("id", inner_id.to_string()))
                            .bind(("emb", new_emb))
                            .bind(("provider", provider))
                            .bind(("model", model))
                            .bind(("dim", dim))
                            .await;
                    }
                }
                if let Some(emb_e) = emb_opt {
                    let sim = Self::cosine_similarity(embedding, &emb_e);
                    if sim >= prox_thresh {
                        let name_s = r
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let etype_or_desc = r
                            .get("data")
                            .and_then(|d| d.get("entity_type").or_else(|| d.get("description")))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        scored.push((id.to_string(), sim, name_s, etype_or_desc));
                    } else {
                        skipped += 1;
                    }
                }
            }
        }
        tracing::debug!(
            "inject_memories: {} candidates scored, {} skipped",
            scored.len(),
            skipped
        );

        // Sort by similarity and apply threshold; if nothing passes, take top by limit with a minimal floor (0.15)
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut selected: Vec<(String, f32, String, String)> = scored
            .iter()
            .filter(|&(_, s, _, _)| *s >= prox_thresh)
            .take(limit)
            .cloned()
            .collect();
        if selected.is_empty() && !scored.is_empty() {
            let floor = std::env::var("SURR_INJECT_FLOOR")
                .ok()
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or_else(|| {
                    if std::env::var("SURR_INJECT_FLOOR").is_ok() {
                        tracing::warn!("Using env override SURR_INJECT_FLOOR");
                    }
                    self.config.retrieval.floor
                });
            selected = scored
                .into_iter()
                .filter(|(_, s, _, _)| *s >= floor)
                .take(limit)
                .collect();
        }
        let memory_ids: Vec<String> = selected.iter().map(|(id, _, _, _)| id.clone()).collect();
        tracing::debug!(
            "inject_memories: Top {} matches: {:?}",
            selected.len(),
            selected
                .iter()
                .take(3)
                .map(|(_, sim, name, _)| format!("{:.2} {}", sim, name))
                .collect::<Vec<_>>()
        );

        // Optional enrichment with names/types
        let enriched = if !selected.is_empty() {
            let mut s = String::new();
            if let Some(sm) = submode {
                s.push_str(&format!("Submode: {}\n", sm));
            }
            s.push_str("Nearby entities:\n");
            for (i, (_id, sim, name, etype)) in selected.iter().take(5).enumerate() {
                if etype.is_empty() {
                    s.push_str(&format!("- ({:.2}) {}\n", sim, name));
                } else {
                    s.push_str(&format!("- ({:.2}) {} [{}]\n", sim, name, etype));
                }
                if i >= 4 {
                    break;
                }
            }
            Some(s)
        } else {
            None
        };

        // Persist to the thought
        let q = self
            .db
            .query("UPDATE type::thing($tb, $id) SET injected_memories = $mems, enriched_content = $enr RETURN meta::id(id) as id")
            .bind(("tb", "thoughts"))
            .bind(("id", thought_id.to_string()))
            .bind(("mems", memory_ids.clone()))
            .bind(("enr", enriched.clone().unwrap_or_default()));
        // Note: empty string will act like clearing or setting to empty; acceptable for now
        let _: Vec<serde_json::Value> = q.await?.take(0)?;
        tracing::debug!(
            "inject_memories: Injected {} memories for thought {}, enriched content length: {}",
            memory_ids.len(),
            thought_id,
            enriched.as_ref().map_or(0, |s| s.len())
        );

        Ok((memory_ids.len(), enriched))
    }

    /// Check for mixed embedding dimensions across thoughts and KG tables
    pub async fn check_embedding_dims(&self) -> Result<()> {
        // Query distinct embedding dimensions in thoughts
        let thoughts_dims: Vec<i64> = self
            .db
            .query("SELECT array::len(embedding) AS dim FROM thoughts GROUP ALL")
            .await
            .map_err(|e| SurrealMindError::Database {
                message: format!("Database query error: {}", e),
            })?
            .take(0)?;

        // Query distinct dimensions in KG entities
        let kg_entity_dims: Vec<i64> = self
            .db
            .query("SELECT array::len(embedding) AS dim FROM kg_entities GROUP ALL")
            .await
            .map_err(|e| SurrealMindError::Database {
                message: format!("Database query error: {}", e),
            })?
            .take(0)?;

        // Query distinct dimensions in KG observations
        let kg_obs_dims: Vec<i64> = self
            .db
            .query("SELECT array::len(embedding) AS dim FROM kg_observations GROUP ALL")
            .await
            .map_err(|e| SurrealMindError::Database {
                message: format!("Database query error: {}", e),
            })?
            .take(0)?;

        let mut all_dims = Vec::new();
        all_dims.extend(thoughts_dims);
        all_dims.extend(kg_entity_dims);
        all_dims.extend(kg_obs_dims);

        let unique_dims: std::collections::HashSet<_> = all_dims.into_iter().collect();

        if unique_dims.len() > 1 {
            return Err(SurrealMindError::Database {
                message: format!(
                    "Mixed embedding dimensions detected: {:?}. Re-embed to fix.",
                    unique_dims
                ),
            });
        }

        Ok(())
    }
}
