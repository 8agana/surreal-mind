//! inner_voice tool handler for RAG retrieval, synthesis, and optional KG staging

use crate::error::{Result, SurrealMindError};
use crate::kg_extractor::HeuristicExtractor;
use crate::server::SurrealMindServer;
use chrono::{Datelike, Duration, LocalResult, TimeZone, Utc};
use chrono_tz::Tz;
use regex::escape as rx_escape;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::{Value, json};
use std::collections::HashMap;

/// Parameters for the inner_voice tool (RAG + optional NLQ)
#[derive(Debug, serde::Deserialize)]
pub struct InnerVoiceParams {
    pub content: String,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_u64_forgiving"
    )]
    pub top_k: Option<u64>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_f32_forgiving"
    )]
    pub sim_thresh: Option<f32>,
    #[serde(default)]
    pub stage_kg: Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_f32_forgiving"
    )]
    pub confidence_min: Option<f32>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_u64_forgiving"
    )]
    pub max_nodes: Option<u64>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_u64_forgiving"
    )]
    pub max_edges: Option<u64>,
    #[serde(default)]
    pub save: Option<bool>,
    #[serde(default)]
    pub auto_mark_removal: Option<bool>,
    // NLQ-specific parameters (when provided, enables NLQ mode)
    #[serde(default)]
    pub when: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_u64_forgiving"
    )]
    pub limit: Option<u64>,
    #[serde(default)]
    pub order: Option<String>,
}

const STOPWORDS: &[&str] = &["The", "This", "That", "What", "When", "Where"];

fn extract_entities(query: &str) -> Vec<String> {
    let aliases = HashMap::from([
        ("sam", "Sam Atagana"),
        ("cc", "Claude Code"),
        ("codex", "Codex"),
    ]);

    query
        .split_whitespace()
        .filter(|w| w.chars().next().is_some_and(|c| c.is_uppercase()))
        .filter(|w| !STOPWORDS.contains(w))
        .filter_map(|w| aliases.get(w.to_lowercase().as_str()))
        .map(|s| s.to_string())
        .collect()
}

fn parse_temporal(
    phrase: &str,
    tz: Tz,
    now_utc: impl Fn() -> chrono::DateTime<Utc>,
) -> Option<(chrono::DateTime<Utc>, chrono::DateTime<Utc>)> {
    let now_local = now_utc().with_timezone(&tz);

    let day_start = |d: chrono::NaiveDate| -> Option<chrono::DateTime<Utc>> {
        match tz.with_ymd_and_hms(d.year(), d.month(), d.day(), 0, 0, 0) {
            LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => Some(dt.with_timezone(&Utc)),
            LocalResult::None => None,
        }
    };

    let (start_local, end_local) = match phrase {
        "yesterday" => {
            let y = now_local.date_naive().pred_opt()?;
            (y, y.succ_opt()?)
        }
        "two weeks ago" => {
            let target = now_local - Duration::weeks(2);
            let d = target.date_naive();
            (d, d.succ_opt()?)
        }
        "this week" => {
            let start_week = now_local.date_naive()
                - chrono::Duration::days(now_local.weekday().num_days_from_monday() as i64);
            (start_week, start_week + chrono::Duration::weeks(1))
        }
        "last month" => {
            let first_this = now_local.date_naive().with_day(1)?;
            let first_last = first_this - chrono::Months::new(1);
            let last_last = first_this - chrono::Duration::days(1);
            (first_last, last_last.succ_opt()?)
        }
        "last week" => {
            let monday_this = now_local.date_naive()
                - chrono::Duration::days(now_local.weekday().num_days_from_monday() as i64);
            let monday_last = monday_this - chrono::Duration::weeks(1);
            (monday_last, monday_this)
        }
        _ => return None,
    };

    Some((day_start(start_local)?, day_start(end_local)?))
}

impl SurrealMindServer {
    /// Handle the inner_voice tool call (RAG + optional NLQ mode + optional KG staging + save)
    pub async fn handle_inner_voice(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: InnerVoiceParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::Serialization {
                message: format!("Invalid parameters: {}", e),
            })?;

        // Redact content at info level to avoid logging private thoughts
        tracing::info!(
            "inner_voice called (content_len={}, nlq_mode={})",
            params.content.len(),
            params.when.is_some() || params.limit.is_some() || params.order.is_some()
        );
        let dbg_preview: String = params.content.chars().take(50).collect();
        tracing::debug!("inner_voice content (first 50 chars): {}", dbg_preview);

        // Check if we're in NLQ mode (any NLQ parameter provided)
        let nlq_mode = params.when.is_some() || params.limit.is_some() || params.order.is_some();

        let scored_sources = if nlq_mode {
            // NLQ Mode: SQL prefilter + cosine ranking
            self.handle_nlq_retrieval(&params).await?
        } else {
            // Standard RAG Mode: existing logic
            self.handle_rag_retrieval(&params).await?
        };

        // Common processing for both modes
        self.process_inner_voice_results(&params, scored_sources)
            .await
    }

    /// Handle NLQ-style retrieval with SQL prefiltering and cosine ranking
    async fn handle_nlq_retrieval(
        &self,
        params: &InnerVoiceParams,
    ) -> Result<Vec<(String, f32, String)>> {
        // Extract entities for keyword filtering
        let entities: Vec<String> = extract_entities(&params.content);
        tracing::debug!("Extracted entities: {:?}", entities);

        // Parse temporal window
        let tz: Tz = self
            .config
            .nlq
            .timezone
            .parse()
            .unwrap_or_else(|_| "America/Chicago".parse().unwrap());
        let (from, to) = params
            .when
            .as_deref()
            .and_then(|w| parse_temporal(w, tz, Utc::now))
            .unwrap_or_else(|| {
                let now = Utc::now();
                (now - Duration::weeks(4), now) // default to last 4 weeks
            });
        tracing::debug!("Temporal window: {} to {}", from, to);

        // Build keyword regex
        let escaped: Vec<String> = entities
            .iter()
            .filter(|k| !STOPWORDS.contains(&k.as_str()))
            .take(self.config.nlq.max_keywords)
            .map(|k| rx_escape(k))
            .collect();
        let keyword_regex = if escaped.is_empty() {
            String::from(".*")
        } else {
            format!("(?i)({})", escaped.join("|"))
        };
        tracing::debug!("Keyword regex: {}", keyword_regex);

        // ORDER BY clause
        let order_clause = match params.order.as_deref() {
            Some("created_at_asc") => "ORDER BY created_at ASC",
            _ => "ORDER BY created_at DESC",
        };

        let dim = self.embedder.dimensions() as i64;
        let final_limit = params
            .limit
            .unwrap_or(self.config.nlq.default_limit as u64)
            .clamp(1, self.config.nlq.max_limit as u64) as usize;

        // Use larger candidate pool for ranking, then truncate to final_limit
        let sql_limit = std::cmp::min(
            self.config.retrieval.candidates,
            self.config.retrieval.db_limit,
        );

        // Build SQL query
        let sql = format!(
            "SELECT meta::id(id) as id, content, embedding, created_at \
             FROM thoughts \
             WHERE array::len(embedding) = $dim \
               AND created_at >= $from AND created_at < $to \
               {} \
               AND (is_summary IS NONE OR is_summary != true) \
               AND (pipeline IS NONE OR pipeline != 'inner_voice') \
             {} \
             LIMIT $limit",
            if self.config.nlq.enable_keyword_filter {
                "AND content ~ $keyword_regex"
            } else {
                ""
            },
            order_clause
        );

        tracing::debug!("NLQ query: {}", sql);

        let mut query = self
            .db
            .query(&sql)
            .bind(("dim", dim))
            .bind(("from", from))
            .bind(("to", to));
        if self.config.nlq.enable_keyword_filter {
            query = query.bind(("keyword_regex", keyword_regex));
        }
        let rows: Vec<Value> = query.bind(("limit", sql_limit as i64)).await?.take(0)?;

        // Stage B: Compute similarity against query
        let query_embedding = self.embedder.embed(&params.content).await?;
        let mut scored_sources: Vec<(String, f32, String)> = rows
            .iter()
            .filter_map(|row| {
                let id = row["id"].as_str()?;
                let content = row["content"].as_str()?;
                let embedding: Vec<f32> = row["embedding"]
                    .as_array()?
                    .iter()
                    .filter_map(|v| v.as_f64())
                    .map(|v| v as f32)
                    .collect();
                if embedding.len() != query_embedding.len() {
                    return None;
                }
                let score = SurrealMindServer::cosine_similarity(&query_embedding, &embedding);
                let excerpt = content.chars().take(200).collect();
                Some((id.to_string(), score, excerpt))
            })
            .collect();

        // Sort by score descending and take final limit
        scored_sources.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_sources.truncate(final_limit);

        tracing::debug!(
            "NLQ metrics: window_from={}, window_to={}, final_limit={}, sql_limit={}, keywords_count={}, candidates_after_sql={}, returned={}",
            from,
            to,
            final_limit,
            sql_limit,
            entities.len(),
            rows.len(),
            scored_sources.len()
        );

        Ok(scored_sources)
    }

    /// Handle standard RAG retrieval (existing logic)
    async fn handle_rag_retrieval(
        &self,
        params: &InnerVoiceParams,
    ) -> Result<Vec<(String, f32, String)>> {
        // Defaults from env or params
        let top_k = params.top_k.unwrap_or_else(|| {
            std::env::var("SURR_TOP_K")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5)
        }) as usize;
        let sim_thresh = params.sim_thresh.unwrap_or_else(|| {
            std::env::var("SURR_SIM_THRESH")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.3)
        });

        // Embed query
        let query_embedding = self.embedder.embed(&params.content).await?;

        // Retrieve thoughts with similarity filtering
        let limit: usize = std::env::var("SURR_DB_LIMIT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000);
        let q_dim = query_embedding.len() as i64;
        let retrieved: Vec<Value> = self
            .db
            .query("SELECT meta::id(id) as id, content, embedding FROM thoughts WHERE embedding_dim = $dim LIMIT $limit")
            .bind(("dim", q_dim))
            .bind(("limit", limit as i64))
            .await?
            .take(0)?;

        tracing::debug!("Retrieved {} thoughts for inner_voice RAG", retrieved.len());

        let mut scored_sources: Vec<(String, f32, String)> = vec![];
        for row in &retrieved {
            if let (Some(id), Some(content), Some(emb_arr)) = (
                row.get("id").and_then(|v| v.as_str()),
                row.get("content").and_then(|v| v.as_str()),
                row.get("embedding").and_then(|v| v.as_array()),
            ) {
                let emb: Vec<f32> = emb_arr
                    .iter()
                    .filter_map(|v| v.as_f64())
                    .map(|f| f as f32)
                    .collect();
                if emb.len() == query_embedding.len() {
                    let sim = SurrealMindServer::cosine_similarity(&query_embedding, &emb);
                    if sim >= sim_thresh {
                        let excerpt = content.chars().take(200).collect();
                        scored_sources.push((id.to_string(), sim, excerpt));
                    }
                }
            }
        }

        // Sort by similarity desc, take top_k
        scored_sources.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_sources.truncate(top_k);

        Ok(scored_sources)
    }

    /// Process results common to both NLQ and RAG modes
    async fn process_inner_voice_results(
        &self,
        params: &InnerVoiceParams,
        scored_sources: Vec<(String, f32, String)>,
    ) -> Result<CallToolResult> {
        let stage_kg = params.stage_kg.unwrap_or(false);
        let confidence_min = params.confidence_min.unwrap_or(0.6);
        let max_nodes = params.max_nodes.unwrap_or(30) as usize;
        let max_edges = params.max_edges.unwrap_or(60) as usize;
        let save = params.save.unwrap_or(true);
        let auto_mark_removal = params.auto_mark_removal.unwrap_or(false);

        // Synthesize answer via provider chain or simple extraction for NLQ
        let nlq_mode = params.when.is_some() || params.limit.is_some() || params.order.is_some();
        let synthesized_answer = if !scored_sources.is_empty() {
            if nlq_mode {
                // NLQ mode: Extract snippets for grounded response
                let top_sources = scored_sources.iter().take(3).collect::<Vec<_>>();
                let snippets: Vec<String> = top_sources
                    .iter()
                    .map(|(_, _, excerpt)| {
                        if excerpt.len() <= 50 {
                            format!("\"{}\"", excerpt)
                        } else {
                            format!("\"{}...\"", &excerpt[..50])
                        }
                    })
                    .collect();
                if snippets.is_empty() {
                    format!(
                        "Found {} relevant thoughts, but couldn't extract content.",
                        scored_sources.len()
                    )
                } else {
                    format!(
                        "Based on relevant thoughts, here are key insights: {}",
                        snippets.join("; ")
                    )
                }
            } else {
                // RAG mode: Use synthesis provider chain
                use std::fmt::Write as _;
                let mut prompt = String::new();
                prompt.push_str("You are inner_voice. Use ONLY these snippets. Respond strictly as JSON with keys: answer (string), sources (array). Refuse if not grounded.\n\nSnippets:\n");
                for (i, (id, _sim, excerpt)) in scored_sources.iter().take(5).enumerate() {
                    let _ = writeln!(prompt, "[T{}] (id={}) {}", i + 1, id, excerpt);
                }
                prompt.push_str("\nTask: Provide a short, grounded answer.\n");
                match crate::synthesis::synthesize_with_chain(
                    &self.config.synthesis,
                    crate::synthesis::SynthesisInput {
                        prompt,
                        model_hint: None,
                    },
                )
                .await
                {
                    Ok(out) => {
                        tracing::info!(
                            "inner_voice synthesis provider_used={} fallback_used={}",
                            out.provider_used,
                            out.fallback_used
                        );
                        out.answer
                    }
                    Err(e) => {
                        tracing::warn!("synthesis failed; falling back to extract: {}", e);
                        let mut synthesis = String::new();
                        for (i, (_id, _sim, excerpt)) in scored_sources.iter().enumerate().take(3) {
                            if i > 0 {
                                synthesis.push('\n');
                            }
                            synthesis.push_str(excerpt);
                        }
                        synthesis.chars().take(600).collect()
                    }
                }
            }
        } else if nlq_mode {
            "I'm sorry, I couldn't find any relevant thoughts for that query.".to_string()
        } else {
            "No relevant thoughts found.".to_string()
        };

        let source_ids: Vec<String> = scored_sources.iter().map(|(id, _, _)| id.clone()).collect();

        // Save synthesized answer as summary thought if enabled
        let mut saved_thought_id: Option<String> = None;
        if save && !nlq_mode {
            // Only save for RAG mode, not NLQ mode
            let query_embedding = self.embedder.embed(&params.content).await?;
            let (provider, model, dim) = self.get_embedding_metadata();

            let created_raw: Vec<Value> = self
                .db
                .query("CREATE thoughts SET content = $synth, created_at = time::now(), embedding = $embedding, injected_memories = [], enriched_content = NONE, injection_scale = 0, significance = 0.5, access_count = 0, last_accessed = NONE, submode = NONE, framework_enhanced = NONE, framework_analysis = NONE, is_summary = true, summary_of = $source_ids, pipeline = 'inner_voice', status = 'active', embedding_provider = $provider, embedding_model = $model, embedding_dim = $dim, embedded_at = time::now() RETURN meta::id(id) as id;")
                .bind(("synth", synthesized_answer.clone()))
                .bind(("embedding", query_embedding))
                .bind(("source_ids", source_ids.clone()))
                .bind(("provider", provider))
                .bind(("model", model))
                .bind(("dim", dim))
                .await?
                .take(0)?;
            saved_thought_id = created_raw
                .first()
                .and_then(|v| v.get("id"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }

        // KG staging (both modes)
        let mut pending_entities = 0;
        let mut pending_relationships = 0;
        let mut marked_for_removal = 0;
        if stage_kg && !scored_sources.is_empty() {
            let source_texts: Vec<String> = scored_sources
                .iter()
                .map(|(_, _, excerpt)| excerpt.clone())
                .collect();
            let extractor = HeuristicExtractor::new();
            let extraction = extractor.extract(&source_texts).await?;

            // Stage entities
            for entity in extraction
                .entities
                .into_iter()
                .filter(|e| e.confidence >= confidence_min)
                .take(max_nodes)
            {
                self.db.query("CREATE kg_entity_candidates SET name = $name, entity_type = $etype, data = $data, confidence = $conf, source_thought_id = $sid")
                    .bind(("name", entity.name))
                    .bind(("etype", entity.entity_type))
                    .bind(("data", entity.properties))
                    .bind(("conf", entity.confidence as f64))
                    .bind(("sid", source_ids.join(",")))
                    .await?;
                pending_entities += 1;
            }

            // Stage relationships
            for rel in extraction
                .relationships
                .into_iter()
                .filter(|r| r.confidence >= confidence_min)
                .take(max_edges)
            {
                self.db.query("CREATE kg_edge_candidates SET source_name = $src, target_name = $tgt, rel_type = $rtype, data = $data, confidence = $conf, source_thought_id = $sid")
                    .bind(("src", rel.source_name))
                    .bind(("tgt", rel.target_name))
                    .bind(("rtype", rel.rel_type))
                    .bind(("data", rel.properties))
                    .bind(("conf", rel.confidence as f64))
                    .bind(("sid", source_ids.join(",")))
                    .await?;
                pending_relationships += 1;
            }

            // Auto-mark for removal if enabled
            if auto_mark_removal && !source_ids.is_empty() {
                self.db
                    .query("UPDATE thoughts SET status = 'removal' WHERE id IN $ids")
                    .bind(("ids", source_ids.clone()))
                    .await?;
                marked_for_removal = source_ids.len();
            }
        }

        // Return appropriate result format
        let result = if nlq_mode {
            // NLQ mode: return sources with scores for compatibility
            let sources: Vec<_> = scored_sources
                .iter()
                .map(|(id, score, _)| {
                    json!({
                        "id": id,
                        "score": score
                    })
                })
                .collect();
            json!({
                "answer": synthesized_answer,
                "sources": sources
            })
        } else {
            // RAG mode: return full result structure
            json!({
                "synthesized_answer": synthesized_answer,
                "saved_thought_id": saved_thought_id,
                "sources": scored_sources.into_iter().map(|(id, sim, excerpt)| json!({
                    "thought_id": id,
                    "similarity": sim,
                    "excerpt": excerpt
                })).collect::<Vec<_>>(),
                "staged": {
                    "pending_entities": pending_entities,
                    "pending_relationships": pending_relationships
                },
                "marked_for_removal": marked_for_removal
            })
        };

        Ok(CallToolResult::structured(result))
    }
}
