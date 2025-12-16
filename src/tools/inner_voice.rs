//! inner_voice tool handler for retrieval-only semantic search
//!
//! This module now exposes a reusable entrypoint `run_inner_voice` that allows
//! other namespaces to call the inner_voice workflow with typed parameters
//! without duplicating handler logic. Behavior is preserved by delegating to the
//! existing handler.

use crate::error::{Result, SurrealMindError};
use crate::schemas::Snippet;
use crate::server::SurrealMindServer;
use blake3::Hasher;
use chrono::Utc;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;
use std::time::{Duration, Instant};

use unicode_normalization::UnicodeNormalization;

pub mod providers;

/// Parameters for the inner_voice tool
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct InnerVoiceRetrieveParams {
    pub query: String,
    #[serde(default)]
    pub top_k: Option<usize>,
    #[serde(default)]
    pub floor: Option<f32>,
    #[serde(default)]
    pub mix: Option<f32>,
    #[serde(default)]
    pub include_private: Option<bool>,
    #[serde(default)]
    pub include_tags: Vec<String>,
    #[serde(default)]
    pub exclude_tags: Vec<String>,
    #[serde(default)]
    pub auto_extract_to_kg: Option<bool>,
    #[serde(default)]
    pub previous_thought_id: Option<String>,
    #[serde(default)]
    pub include_feedback: Option<bool>,
    #[serde(default)]
    pub feedback_max_lines: Option<usize>,
}

/// Runtime snapshot and hooks to support reuse while preserving behavior
#[derive(Clone)]
pub struct InnerVoiceRuntime {
    pub planner_enabled: bool,
    pub topk_default: usize,
    pub mix_default: f32,
    pub floor_default: f32,
    pub max_candidates_per_source: usize,
    pub include_private_default: bool,

    // Provider configuration
    pub grok_base: String,
    pub grok_model: String,
    pub grok_allow: bool,
    pub local_fallback: bool,
}

#[derive(Clone, Default)]
pub struct InnerVoiceHooks;

impl InnerVoiceHooks {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Clone)]
pub struct InnerVoiceContext {
    pub runtime: InnerVoiceRuntime,
    pub hooks: InnerVoiceHooks,
}

impl InnerVoiceContext {
    pub fn from_server(server: &SurrealMindServer) -> Self {
        Self {
            runtime: server.build_inner_voice_runtime(),
            hooks: InnerVoiceHooks::new(),
        }
    }
}

impl SurrealMindServer {
    /// Build a runtime snapshot for inner_voice from current config/env (no behavior change)
    pub fn build_inner_voice_runtime(&self) -> InnerVoiceRuntime {
        let cfg = &self.config.runtime.inner_voice;
        let planner_enabled = cfg.plan;
        let topk_default = cfg.topk_default;
        let mix_default = cfg.mix;
        let floor_default = cfg.min_floor;
        let max_candidates_per_source = cfg.max_candidates_per_source;
        let include_private_default = cfg.include_private_default;

        // Provider/env snapshot (preserve existing defaults and precedence)
        let grok_base =
            std::env::var("GROK_BASE_URL").unwrap_or_else(|_| "https://api.x.ai/v1".to_string());
        let grok_model =
            std::env::var("GROK_MODEL").unwrap_or_else(|_| "grok-code-fast-1".to_string());
        let grok_allow =
            std::env::var("IV_ALLOW_GROK").unwrap_or_else(|_| "true".to_string()) != "false";

        // Check for deprecated CLI env vars and warn
        if std::env::var("IV_CLI_CMD").is_ok() {
            tracing::warn!(
                "CLI configuration (IV_CLI_CMD, IV_CLI_ARGS_JSON, etc.) is deprecated and will be removed. Defaulting to Grok (if key present) or local fallback."
            );
        }
        if std::env::var("IV_SYNTH_CLI_CMD").is_ok() {
            tracing::warn!("IV_SYNTH_CLI_CMD is deprecated. Defaulting to Grok or local fallback.");
        }
        if std::env::var("IV_CLI_ARGS_JSON").is_ok() {
            tracing::warn!("IV_CLI_ARGS_JSON is deprecated. Defaulting to Grok or local fallback.");
        }
        if std::env::var("IV_SYNTH_CLI_ARGS_JSON").is_ok() {
            tracing::warn!(
                "IV_SYNTH_CLI_ARGS_JSON is deprecated. Defaulting to Grok or local fallback."
            );
        }
        if std::env::var("IV_CLI_TIMEOUT_MS").is_ok() {
            tracing::warn!(
                "IV_CLI_TIMEOUT_MS is deprecated. Defaulting to Grok or local fallback."
            );
        }
        if std::env::var("IV_SYNTH_TIMEOUT_MS").is_ok() {
            tracing::warn!(
                "IV_SYNTH_TIMEOUT_MS is deprecated. Defaulting to Grok or local fallback."
            );
        }
        let provider_pref =
            std::env::var("IV_SYNTH_PROVIDER").unwrap_or_else(|_| "grok".to_string());
        if provider_pref.eq_ignore_ascii_case("gemini_cli") {
            tracing::warn!(
                "IV_SYNTH_PROVIDER='gemini_cli' is deprecated. Defaulting to Grok (if key present) or local fallback."
            );
        }

        let local_fallback = std::env::var("INNER_VOICE_LOCAL_FALLBACK")
            .map(|v| v.trim() != "false")
            .unwrap_or(true);

        InnerVoiceRuntime {
            planner_enabled,
            topk_default,
            mix_default,
            floor_default,
            max_candidates_per_source,
            include_private_default,
            grok_base,
            grok_model,
            grok_allow,
            local_fallback,
        }
    }
}

/// Reusable entrypoint for inner_voice, preserving behavior by delegating to the handler.
/// Other namespaces can call this with typed params without duplicating logic.
pub async fn run_inner_voice(
    server: &SurrealMindServer,
    params: &InnerVoiceRetrieveParams,
    ctx: &InnerVoiceContext,
) -> Result<CallToolResult> {
    // Currently behavior is preserved by delegating to the handler. We still accept
    // a context so callers can pass a runtime snapshot and hooks for future reuse.
    // Touch the fields to avoid dead-code lints while keeping behavior unchanged.
    let _ = &ctx.runtime;
    let _ = &ctx.hooks;

    // Convert typed params into the handler's expected CallToolRequestParam
    let args_value =
        serde_json::to_value(params.clone()).map_err(|e| SurrealMindError::Internal {
            message: e.to_string(),
        })?;
    let args_map = args_value
        .as_object()
        .cloned()
        .ok_or_else(|| SurrealMindError::Internal {
            message: "Failed to convert params to object".into(),
        })?;
    let request = CallToolRequestParam {
        name: "inner_voice".into(),
        arguments: Some(args_map),
    };
    server.handle_inner_voice_retrieve(request).await
}

/// Planner response from Grok
#[derive(Debug, Clone, Deserialize)]
pub struct PlannerResponse {
    pub rewritten_query: String,
    #[serde(default)]
    pub date_range: Option<DateRange>,
    #[serde(default)]
    pub recency_days: Option<u32>,
    #[serde(default)]
    pub include_tags: Vec<String>,
    #[serde(default)]
    pub exclude_tags: Vec<String>,
    #[serde(default)]
    pub entity_hints: Vec<String>,
    #[serde(default)]
    pub top_k: Option<usize>,
    #[serde(default)]
    pub mix: Option<f32>,
    #[serde(default)]
    pub floor: Option<f32>,
}

/// Date range for temporal filtering
#[derive(Debug, Clone, Deserialize)]
pub struct DateRange {
    pub from: String,
    pub to: String,
}

/// Internal struct for candidate items
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Candidate {
    pub id: String,
    pub table: String,
    pub source_type: String,
    pub origin: String,
    pub created_at: String,
    pub text: String,
    pub embedding: Vec<f32>,
    pub score: f32,
    pub tags: Vec<String>,
    pub is_private: bool,
    pub content_hash: String,
    pub trust_tier: String,
}

/// Regex for sentence boundary detection
static SENTENCE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"[.!?]["”"']?\s"#).expect("regex should compile"));

/// Match fenced code blocks that contain JSON. Permissive to catch ```json ...``` and bare ``` ...```.
static FENCED_JSON_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?s)```(?:json)?\s*(\{.*?\})\s*```").expect("fenced json regex should compile")
});

impl SurrealMindServer {
    /// Handle the inner_voice tool call
    pub async fn handle_inner_voice_retrieve(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request
            .arguments
            .ok_or_else(|| SurrealMindError::InvalidParams {
                message: "Missing parameters".into(),
            })?;
        let params: InnerVoiceRetrieveParams =
            serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                SurrealMindError::InvalidParams {
                    message: format!("Invalid parameters: {}", e),
                }
            })?;

        // Gate check
        if !self.config.runtime.inner_voice.enable {
            return Err(SurrealMindError::FeatureDisabled {
                message: "inner_voice is disabled (SURR_ENABLE_INNER_VOICE=0 or SURR_DISABLE_INNER_VOICE=1)".into(),
            });
        }

        // Validate query
        if params.query.trim().is_empty() {
            return Err(SurrealMindError::InvalidParams {
                message: "Query cannot be empty".into(),
            });
        }

        let _start_time = Instant::now();

        // Config
        let cfg = &self.config.runtime.inner_voice;
        let mut top_k = params.top_k.unwrap_or(cfg.topk_default).clamp(1, 50);
        let mut floor = params.floor.unwrap_or(cfg.min_floor).clamp(0.0, 1.0);
        let mut mix = params.mix.unwrap_or(cfg.mix).clamp(0.0, 1.0);
        let include_private = params
            .include_private
            .unwrap_or(cfg.include_private_default);

        // Planner stage (if enabled)
        let mut effective_query = params.query.clone();
        let mut include_tags = params.include_tags.clone();
        let mut exclude_tags = params.exclude_tags.clone();
        let mut date_filter = None;
        let mut planner_response = None;
        if cfg.plan {
            let base = std::env::var("GROK_BASE_URL")
                .unwrap_or_else(|_| "https://api.x.ai/v1".to_string());
            let grok_key = std::env::var("GROK_API_KEY").unwrap_or_default();
            if !grok_key.is_empty() {
                match call_planner_grok(&base, &grok_key, &params.query).await {
                    Ok(planner) => {
                        planner_response = Some(planner.clone());
                        // Use rewritten query
                        effective_query = planner.rewritten_query;

                        // Apply planner overrides
                        if let Some(p_top_k) = planner.top_k {
                            top_k = p_top_k.clamp(1, 50);
                        }
                        if let Some(p_mix) = planner.mix {
                            mix = p_mix.clamp(0.0, 1.0);
                        }
                        if let Some(p_floor) = planner.floor {
                            floor = p_floor.clamp(0.0, 1.0);
                        }

                        // Tags
                        if !planner.include_tags.is_empty() {
                            include_tags.extend(planner.include_tags);
                        }
                        if !planner.exclude_tags.is_empty() {
                            exclude_tags.extend(planner.exclude_tags);
                        }

                        // Date filter
                        if let Some(date_range) = planner.date_range {
                            date_filter = Some(date_range);
                        } else if let Some(days) = planner.recency_days {
                            if days > 0 {
                                let now = Utc::now();
                                let from = now - chrono::Duration::days(days as i64);
                                date_filter = Some(DateRange {
                                    from: from.format("%Y-%m-%d").to_string(),
                                    to: now.format("%Y-%m-%d").to_string(),
                                });
                            }
                        }
                    }
                    Err(_) => {
                        // Fallback to single-pass: use original query
                        effective_query = params.query.clone();
                    }
                }
            }
        }

        // Embed query
        let q_emb = self.embedder.embed(&effective_query).await.map_err(|e| {
            SurrealMindError::EmbedderUnavailable {
                message: e.to_string(),
            }
        })?;
        let q_dim = q_emb.len() as i64;

        // Fetch candidates
        let cap = (3 * top_k).min(cfg.max_candidates_per_source);
        let thought_candidates = self
            .fetch_thought_candidates(
                cap,
                q_dim,
                include_private,
                &date_filter,
                &include_tags,
                &exclude_tags,
            )
            .await?;
        let kg_entity_candidates = self
            .fetch_kg_entity_candidates(&params, cap, q_dim, &date_filter)
            .await?;
        let kg_obs_candidates = self
            .fetch_kg_observation_candidates(&params, cap, q_dim, &date_filter)
            .await?;

        // Compute similarities
        let mut thought_hits: Vec<Candidate> = Vec::new();
        let mut kg_hits: Vec<Candidate> = Vec::new();

        for cand in thought_candidates {
            if cand.embedding.len() == q_emb.len() {
                let score = cosine(&q_emb, &cand.embedding);
                if score >= floor {
                    let mut c = cand;
                    c.score = score;
                    thought_hits.push(c);
                }
            }
        }

        for cand in kg_entity_candidates.into_iter().chain(kg_obs_candidates) {
            if cand.embedding.len() == q_emb.len() {
                let mut score = cosine(&q_emb, &cand.embedding);
                if score >= floor {
                    // Apply entity_hints boost (advisory only)
                    if cfg.plan {
                        if let Some(planner) = &planner_response {
                            if !planner.entity_hints.is_empty() {
                                let name_lower = cand.text.to_lowercase();
                                for hint in &planner.entity_hints {
                                    if name_lower.contains(&hint.to_lowercase()) {
                                        score += 0.05; // Small boost
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    let mut c = cand;
                    c.score = score;
                    kg_hits.push(c);
                }
            }
        }

        // Adaptive floor if needed
        let (t_hits, k_hits, _floor_used) =
            apply_adaptive_floor(&thought_hits, &kg_hits, floor, cfg.min_floor, top_k);

        // Allocate slots
        let (kg_slots, thought_slots) = allocate_slots(mix, top_k, &k_hits, &t_hits);

        // Dedupe and select
        let mut selected =
            select_and_dedupe(t_hits.clone(), k_hits.clone(), thought_slots, kg_slots);

        // Cap text and compute hashes
        for cand in &mut selected {
            cap_text(&mut cand.text, 800);
            cand.content_hash = hash_content(&cand.text);
            cand.trust_tier = compute_trust_tier(&cand.origin, &cand.table);
        }

        // Sort by score desc
        selected.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top_k
        selected.truncate(top_k);

        // Build snippets (internal only)
        let snippets: Vec<Snippet> = selected
            .iter()
            .map(|c| Snippet {
                id: c.id.clone(),
                table: c.table.clone(),
                source_type: c.source_type.clone(),
                origin: c.origin.clone(),
                trust_tier: c.trust_tier.clone(),
                created_at: c.created_at.clone(),
                text: c.text.clone(),
                score: c.score,
                content_hash: c.content_hash.clone(),
                span_start: None,
                span_end: None,
            })
            .collect();

        // Synthesize answer — default to Grok, then local fallback.
        let mut synthesized = String::new();
        let mut synth_provider = String::new();
        let mut synth_model = String::new();

        // Try Grok first if allowed and key present
        if synthesized.trim().is_empty() {
            let base = std::env::var("GROK_BASE_URL")
                .unwrap_or_else(|_| "https://api.x.ai/v1".to_string());
            let model =
                std::env::var("GROK_MODEL").unwrap_or_else(|_| "grok-code-fast-1".to_string());
            let grok_key = std::env::var("GROK_API_KEY").unwrap_or_default();
            let messages = build_synthesis_messages(&params.query, &snippets);
            if crate::tools::inner_voice::providers::allow_grok() && !grok_key.is_empty() {
                match crate::tools::inner_voice::providers::grok_call(
                    &base, &model, &grok_key, &messages,
                )
                .await
                {
                    Ok(ans) => {
                        synthesized = ans;
                        synth_provider = "grok".to_string();
                        synth_model = model;
                    }
                    Err(e) => {
                        tracing::warn!("inner_voice: grok_call failed: {}", e);
                    }
                }
            } else {
                tracing::debug!("inner_voice: grok disabled or key missing");
            }
        }

        if synthesized.trim().is_empty() {
            // Last-resort fallback: minimal grounded summary style, no refusals
            synthesized = crate::tools::inner_voice::providers::fallback_from_snippets(&snippets);
            if synth_provider.is_empty() {
                synth_provider = "local".into();
            }
            if synth_model.is_empty() {
                synth_model = "n/a".into();
            }
        }

        // Minimal citations line from internal selections
        let mut ids: Vec<String> = Vec::new();
        for c in &selected {
            let prefix = match c.table.as_str() {
                "thoughts" => "thoughts:",
                "kg_entities" => "kge:",
                "kg_observations" => "kgo:",
                other => {
                    if other.len() > 3 {
                        &other[0..3]
                    } else {
                        other
                    }
                }
            };
            ids.push(format!("{}{}", prefix, c.id));
        }
        ids.truncate(6); // keep short
        if !ids.is_empty() {
            synthesized.push_str("\n\nSources: ");
            synthesized.push_str(&ids.join(", "));
        }

        // Persist synthesis thought (Thought A)
        let embedding =
            self.embedder
                .embed(&synthesized)
                .await
                .map_err(|e| SurrealMindError::Embedding {
                    message: e.to_string(),
                })?;
        let synth_thought_id = uuid::Uuid::new_v4().to_string();
        let (provider, model_name, dim) = self.get_embedding_metadata();
        let prev_thought_id = params.previous_thought_id.clone();
        self.db
            .query(
                "CREATE type::thing('thoughts', $id) CONTENT {
                    content: $content,
                    created_at: time::now(),
                    embedding: $embedding,
                    injected_memories: [],
                    enriched_content: NONE,
                    injection_scale: 0,
                    significance: 0.5,
                    access_count: 0,
                    last_accessed: NONE,
                    submode: NONE,
                    framework_enhanced: NONE,
                    framework_analysis: NONE,
                    origin: 'inner_voice',
                    embedding_provider: $provider,
                    embedding_model: $model,
                    embedding_dim: $dim,
                    embedded_at: time::now(),
                    previous_thought_id: $prev
                } RETURN NONE;",
            )
            .bind(("id", synth_thought_id.clone()))
            .bind(("content", synthesized.clone()))
            .bind(("embedding", embedding))
            .bind(("provider", provider.clone()))
            .bind(("model", model_name.clone()))
            .bind(("dim", dim))
            .bind(("prev", prev_thought_id))
            .await?;

        // Feedback dropped for CLI paths (no replacement)
        let (feedback_text, feedback_thought_id) = (String::new(), None::<String>);

        // Optional auto-extraction to KG candidates using Grok JSON extraction
        let auto_extract = crate::tools::inner_voice::providers::compute_auto_extract(
            params.auto_extract_to_kg,
            self.config.runtime.inner_voice.auto_extract_default,
        );
        let mut extracted_entities = 0usize;
        let mut extracted_rels = 0usize;
        if auto_extract {
            // Parse appended candidates; fail fast on malformed JSON to surface brittle format changes.
            let mut parsed = match parse_appended_candidates(&synthesized) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(target: "inner_voice", error = %e, "auto_extract: failed to parse appended candidates");
                    return Err(e);
                }
            };

            if parsed.is_none() {
                parsed = match parse_env_candidates() {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!(target: "inner_voice", error = %e, "auto_extract: failed to parse SURR_IV_TEST_CANDIDATES");
                        return Err(e);
                    }
                };
            }

            if let Some(parsed) = parsed {
                let (ec, rc) = self
                    .stage_candidates_from_llm(parsed, &synth_thought_id)
                    .await?;
                extracted_entities = ec;
                extracted_rels = rc;
            }
        }

        // Build sources_compact
        let sources_compact = if !ids.is_empty() {
            format!("Sources: {}", ids.join(", "))
        } else {
            String::new()
        };

        let result = json!({
            "answer": synthesized,
            "synth_thought_id": synth_thought_id,
            "feedback": feedback_text,
            "feedback_thought_id": feedback_thought_id,
            "sources_compact": sources_compact,
            "synth_provider": synth_provider,
            "synth_model": synth_model,
            "embedding_dim": dim,
            "extracted": {"entities": extracted_entities, "relationships": extracted_rels}
        });

        Ok(CallToolResult::structured(result))
    }

    async fn fetch_thought_candidates(
        &self,
        cap: usize,
        q_dim: i64,
        include_private: bool,
        date_filter: &Option<DateRange>,
        include_tags: &[String],
        exclude_tags: &[String],
    ) -> Result<Vec<Candidate>> {
        let mut sql = "SELECT meta::id(id) AS id, content, embedding, created_at, origin ?? 'human' AS origin, tags ?? [] AS tags, is_private ?? false AS is_private FROM thoughts WHERE embedding_dim = $dim".to_string();

        if !include_private {
            sql.push_str(" AND is_private != true");
        }

        // Date filter
        if let Some(_date_range) = date_filter {
            sql.push_str(" AND created_at >= $from_date AND created_at <= $to_date");
        }

        if !include_tags.is_empty() {
            sql.push_str(" AND (");
            for (i, _) in include_tags.iter().enumerate() {
                if i > 0 {
                    sql.push_str(" OR ");
                }
                sql.push_str(&format!("$tag{} IN tags", i));
            }
            sql.push(')');
        }

        if !exclude_tags.is_empty() {
            for (i, _) in exclude_tags.iter().enumerate() {
                sql.push_str(&format!(" AND $etag{} NOT IN tags", i));
            }
        }

        sql.push_str(" LIMIT $limit");

        // Build query after finalizing SQL string
        let mut query = self.db.query(&sql).bind(("dim", q_dim));

        // Date bindings
        if let Some(date_range) = date_filter {
            let from_datetime = format!("{}T00:00:00Z", date_range.from);
            let to_datetime = format!("{}T23:59:59Z", date_range.to);
            query = query
                .bind(("from_date", from_datetime))
                .bind(("to_date", to_datetime));
        }

        // Bind tags
        for (i, tag) in include_tags.iter().enumerate() {
            query = query.bind((format!("tag{}", i), tag.clone()));
        }
        for (i, tag) in exclude_tags.iter().enumerate() {
            query = query.bind((format!("etag{}", i), tag.clone()));
        }

        let mut response = query.bind(("limit", cap as i64)).await?;

        #[derive(Deserialize)]
        struct ThoughtRow {
            id: String,
            content: String,
            embedding: Vec<f32>,
            created_at: surrealdb::sql::Datetime,
            origin: String,
            tags: Vec<String>,
            is_private: bool,
        }

        let rows: Vec<ThoughtRow> = response.take(0)?;
        let candidates = rows
            .into_iter()
            .map(|r| Candidate {
                id: r.id,
                table: "thoughts".to_string(),
                source_type: "thought".to_string(),
                origin: r.origin,
                created_at: r.created_at.to_string(),
                text: r.content,
                embedding: r.embedding,
                score: 0.0,
                tags: r.tags,
                is_private: r.is_private,
                content_hash: String::new(),
                trust_tier: String::new(),
            })
            .collect();

        Ok(candidates)
    }

    async fn fetch_kg_entity_candidates(
        &self,
        _params: &InnerVoiceRetrieveParams,
        cap: usize,
        q_dim: i64,
        date_filter: &Option<DateRange>,
    ) -> Result<Vec<Candidate>> {
        let mut sql = "SELECT meta::id(id) AS id, name ?? 'unknown' AS content, embedding, created_at FROM kg_entities WHERE embedding IS NOT NULL AND embedding_dim = $dim".to_string();

        // Date filter
        if date_filter.is_some() {
            sql.push_str(" AND created_at >= $from_date AND created_at <= $to_date");
        }

        sql.push_str(" LIMIT $limit");

        let mut query = self
            .db
            .query(&sql)
            .bind(("dim", q_dim))
            .bind(("limit", cap as i64));

        // Date bindings
        if let Some(date_range) = date_filter {
            let from_datetime = format!("{}T00:00:00Z", date_range.from);
            let to_datetime = format!("{}T23:59:59Z", date_range.to);
            query = query
                .bind(("from_date", from_datetime))
                .bind(("to_date", to_datetime));
        }

        let mut response = query.await?;

        #[derive(Deserialize)]
        struct KgEntityRow {
            id: String,
            content: String,
            embedding: Vec<f32>,
            created_at: surrealdb::sql::Datetime,
        }

        let rows: Vec<KgEntityRow> = response.take(0)?;
        let candidates = rows
            .into_iter()
            .map(|r| Candidate {
                id: r.id,
                table: "kg_entities".to_string(),
                source_type: "kg_entity".to_string(),
                origin: "tool".to_string(), // Assume KG is from tools
                created_at: r.created_at.to_string(),
                text: r.content,
                embedding: r.embedding,
                score: 0.0,
                tags: Vec::new(),
                is_private: false,
                content_hash: String::new(),
                trust_tier: String::new(),
            })
            .collect();

        Ok(candidates)
    }

    async fn fetch_kg_observation_candidates(
        &self,
        _params: &InnerVoiceRetrieveParams,
        cap: usize,
        q_dim: i64,
        date_filter: &Option<DateRange>,
    ) -> Result<Vec<Candidate>> {
        let mut sql = "SELECT meta::id(id) AS id, content ?? 'unknown' AS content, embedding, created_at FROM kg_observations WHERE embedding IS NOT NULL AND embedding_dim = $dim".to_string();

        // Date filter
        if date_filter.is_some() {
            sql.push_str(" AND created_at >= $from_date AND created_at <= $to_date");
        }

        sql.push_str(" LIMIT $limit");

        let mut query = self
            .db
            .query(&sql)
            .bind(("dim", q_dim))
            .bind(("limit", cap as i64));

        // Date bindings
        if let Some(date_range) = date_filter {
            let from_datetime = format!("{}T00:00:00Z", date_range.from);
            let to_datetime = format!("{}T23:59:59Z", date_range.to);
            query = query
                .bind(("from_date", from_datetime))
                .bind(("to_date", to_datetime));
        }

        let mut response = query.await?;

        #[derive(Deserialize)]
        struct KgObsRow {
            id: String,
            content: String,
            embedding: Vec<f32>,
            created_at: surrealdb::sql::Datetime,
        }

        let rows: Vec<KgObsRow> = response.take(0)?;
        let candidates = rows
            .into_iter()
            .map(|r| Candidate {
                id: r.id,
                table: "kg_observations".to_string(),
                source_type: "kg_observation".to_string(),
                origin: "tool".to_string(),
                created_at: r.created_at.to_string(),
                text: r.content,
                embedding: r.embedding,
                score: 0.0,
                tags: Vec::new(),
                is_private: false,
                content_hash: String::new(),
                trust_tier: String::new(),
            })
            .collect();

        Ok(candidates)
    }
}

#[derive(Debug, Deserialize)]
struct CandidateEnvelope {
    #[serde(default)]
    candidates: Vec<ExtractCandidate>,
}

#[derive(Debug, Deserialize)]
struct ExtractCandidate {
    name: String,
    #[serde(rename = "type")]
    kind: String, // "entity" or "relationship"
    #[serde(default)]
    entity_type: Option<String>,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    rel_type: Option<String>,
    #[serde(default)]
    confidence: Option<f32>,
}

impl SurrealMindServer {
    /// Stage LLM-provided candidates into *_candidates tables
    async fn stage_candidates_from_llm(
        &self,
        out: CandidateEnvelope,
        thought_id: &str,
    ) -> Result<(usize, usize)> {
        let mut ecount = 0usize;
        let mut rcount = 0usize;

        for c in out.candidates {
            match c.kind.to_lowercase().as_str() {
                "entity" => {
                    let name = c.name.trim().to_string();
                    if name.is_empty() {
                        continue;
                    }
                    let etype = c.entity_type.clone().unwrap_or_default();
                    let found: Vec<serde_json::Value> = self
                        .db
                        .query("SELECT meta::id(id) as id FROM kg_entity_candidates WHERE name = $n AND entity_type = $t AND status = 'pending' LIMIT 1")
                        .bind(("n", name.clone()))
                        .bind(("t", etype.clone()))
                        .await?
                        .take(0)?;
                    if found.is_empty() {
                        let _ : Vec<serde_json::Value> = self
                        .db
                        .query("CREATE kg_entity_candidates SET created_at = time::now(), name = $n, entity_type = $t, confidence = $c, status = 'pending', data = { staged_by_thought: $th, origin: 'inner_voice' } RETURN meta::id(id) as id")
                        .bind(("n", name))
                        .bind(("t", etype))
                        .bind(("c", c.confidence.unwrap_or(0.7)))
                        .bind(("th", thought_id.to_string()))
                        .await?
                        .take(0)?;
                        ecount += 1;
                    }
                }
                "relationship" => {
                    let src = c.name.trim().to_string();
                    let dst = c.target.as_deref().unwrap_or("").trim().to_string();
                    if src.is_empty() || dst.is_empty() {
                        continue;
                    }
                    let kind = c
                        .rel_type
                        .clone()
                        .unwrap_or_else(|| "related_to".to_string());
                    let conf = c.confidence.unwrap_or(0.6_f32);
                    let found: Vec<serde_json::Value> = self
                        .db
                        .query("SELECT meta::id(id) as id FROM kg_edge_candidates WHERE source_name = $s AND target_name = $t AND rel_type = $k AND status = 'pending' LIMIT 1")
                        .bind(("s", src.clone()))
                        .bind(("t", dst.clone()))
                        .bind(("k", kind.clone()))
                        .await?
                        .take(0)?;
                    if found.is_empty() {
                        let _ : Vec<serde_json::Value> = self
                            .db
                            .query("CREATE kg_edge_candidates SET created_at = time::now(), source_name = $s, target_name = $t, rel_type = $k, confidence = $c, status = 'pending', data = { staged_by_thought: $th, origin: 'inner_voice' } RETURN meta::id(id) as id")
                            .bind(("s", src))
                            .bind(("t", dst))
                            .bind(("k", kind))
                            .bind(("c", conf))
                            .bind(("th", thought_id.to_string()))
                            .await?
                            .take(0)?;
                        rcount += 1;
                    }
                }
                _ => continue,
            }
        }

        Ok((ecount, rcount))
    }
}

/// Parse an appended JSON block with candidates from the synthesized answer.
/// Returns Ok(None) when no candidate block is present; returns Err on malformed blocks so
/// auto_extract callers can fail fast instead of silently dropping candidates.
fn parse_appended_candidates(text: &str) -> Result<Option<CandidateEnvelope>> {
    let trimmed = text.trim();

    // 1) Look for fenced code blocks anywhere (prefer the last occurrence)
    let mut last_block: Option<String> = None;
    for caps in FENCED_JSON_REGEX.captures_iter(trimmed) {
        if let Some(m) = caps.get(1) {
            last_block = Some(m.as_str().trim().to_string());
        }
    }
    if let Some(json_block) = last_block {
        match serde_json::from_str::<CandidateEnvelope>(&json_block) {
            Ok(parsed) => return Ok(Some(parsed)),
            Err(e) => {
                let snippet = tail_snippet(&json_block);
                tracing::warn!(target: "inner_voice", error = %e, snippet = %snippet, "Failed to parse fenced JSON candidates");
                return Err(SurrealMindError::Internal {
                    message: format!("Failed to parse fenced JSON candidates: {}", e),
                });
            }
        }
    }

    // 2) Fallback: look for the last '{' and attempt JSON parse if it contains \"candidates\"
    if let Some(pos) = trimmed.rfind('{') {
        let slice = trimmed[pos..].trim();
        if slice.contains("\"candidates\"") {
            match serde_json::from_str::<CandidateEnvelope>(slice) {
                Ok(parsed) => return Ok(Some(parsed)),
                Err(e) => {
                    let snippet = tail_snippet(slice);
                    tracing::warn!(target: "inner_voice", error = %e, snippet = %snippet, "Failed to parse trailing JSON candidates");
                    return Err(SurrealMindError::Internal {
                        message: format!("Failed to parse trailing JSON candidates: {}", e),
                    });
                }
            }
        }
    }

    Ok(None)
}

/// Optional test hook: stage candidates from SURR_IV_TEST_CANDIDATES env when no JSON is found.
fn parse_env_candidates() -> Result<Option<CandidateEnvelope>> {
    if let Ok(val) = std::env::var("SURR_IV_TEST_CANDIDATES") {
        let trimmed = val.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        match serde_json::from_str::<CandidateEnvelope>(trimmed) {
            Ok(parsed) => Ok(Some(parsed)),
            Err(e) => {
                tracing::warn!(target: "inner_voice", error = %e, "Failed to parse SURR_IV_TEST_CANDIDATES");
                Err(SurrealMindError::Internal {
                    message: format!("Invalid SURR_IV_TEST_CANDIDATES JSON: {}", e),
                })
            }
        }
    } else {
        Ok(None)
    }
}

fn tail_snippet(s: &str) -> String {
    let clean = s.trim();
    if clean.len() <= 200 {
        clean.to_string()
    } else {
        clean[clean.len() - 200..].to_string()
    }
}

/// Compute cosine similarity (delegates to utils)
fn cosine(a: &[f32], b: &[f32]) -> f32 {
    crate::utils::cosine_similarity(a, b)
}

/// Build synthesis messages for Grok using provided snippets
fn build_synthesis_messages(query: &str, snippets: &[Snippet]) -> serde_json::Value {
    let mut lines = Vec::new();
    let max_snips = usize::min(8, snippets.len());
    for (i, sn) in snippets.iter().take(max_snips).enumerate() {
        let mut text = sn.text.clone();
        if text.len() > 800 {
            text.truncate(800);
        }
        let meta = format!("[{}] {}:{} score={:.3}", i + 1, sn.table, sn.id, sn.score);
        lines.push(format!("{}\n{}", meta, text));
    }

    let system = "You are a careful, grounded synthesizer. Only use the provided snippets. Cite sources inline like [1], [2]. Prefer concise answers (<= 4 sentences). If insufficient evidence, say so. If you identify knowledge-graph candidates, append a fenced JSON block exactly at the end: ```json {\"candidates\":[{\"name\":\"<entity_or_source>\",\"type\":\"entity\",\"entity_type\":\"<optional>\"},{\"name\":\"<source>\",\"type\":\"relationship\",\"target\":\"<target>\",\"rel_type\":\"<optional>\"}]}```. Omit the block entirely if no candidates.";
    let user = format!(
        "Query: {}\n\nSnippets:\n{}\n\nTask: Provide a concise, grounded answer with inline [n] citations. If you see high-confidence entities or relationships, append the JSON block described in the system message at the very end. Do not include any other trailing text after the JSON block.",
        query,
        lines.join("\n\n")
    );

    serde_json::json!([
        {"role": "system", "content": system},
        {"role": "user", "content": user}
    ])
}

/// Parse planner JSON into PlannerResponse with validation
pub fn parse_planner_json(s: &str) -> Result<PlannerResponse> {
    match serde_json::from_str::<PlannerResponse>(s) {
        Ok(planner) => {
            if planner.rewritten_query.trim().is_empty() {
                return Err(SurrealMindError::Internal {
                    message: "Planner returned empty rewritten_query".into(),
                });
            }
            Ok(planner)
        }
        Err(e) => Err(SurrealMindError::Internal {
            message: format!("Failed to parse planner JSON: {}", e),
        }),
    }
}

/// Call Grok for planner constraints
async fn call_planner_grok(base: &str, api_key: &str, query: &str) -> Result<PlannerResponse> {
    let system_prompt = "You are a query planner. Convert the user's request into explicit retrieval constraints. Output strict JSON matching the provided schema. Use concrete ISO-8601 dates. Do not include any text outside JSON.";
    let schema_reminder = r#"{
  "rewritten_query": "string",              // required, non-empty
  "date_range": {                           // optional; concrete ISO-8601 dates
      "from": "YYYY-MM-DD",
      "to": "YYYY-MM-DD"
  },
  "recency_days": 7,                        // optional; integer > 0
  "include_tags": ["string", ...],          // optional
  "exclude_tags": ["string", ...],          // optional
  "entity_hints": ["string", ...],          // optional; advisory only
  "top_k": 10,                              // optional; 1..50
  "mix": 0.6,                               // optional; 0.0..1.0 (kg share)
  "floor": 0.25                             // optional; 0.0..1.0
}"#;
    let user_prompt = format!("Query: {}\n\nSchema: {}", query, schema_reminder);

    let messages = json!([
        {"role": "system", "content": system_prompt},
        {"role": "user", "content": user_prompt}
    ]);

    let url = format!("{}/chat/completions", base.trim_end_matches('/'));
    let body = json!({
        "model": "grok-code-fast-1",
        "messages": messages,
        "temperature": 0.2,
        "max_tokens": 200
    });
    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| SurrealMindError::Internal {
            message: format!("Failed to build HTTP client: {}", e),
        })?;
    let resp = client
        .post(url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| SurrealMindError::Internal {
            message: e.to_string(),
        })?;

    // Check response status before parsing
    let status = resp.status();
    if !status.is_success() {
        let body_text = resp
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read response body".to_string());
        check_http_status(status.as_u16(), &body_text, "Grok planner")?;
        unreachable!(); // check_http_status always returns an error for non-success
    }

    let val: serde_json::Value = resp.json().await.map_err(|e| SurrealMindError::Internal {
        message: e.to_string(),
    })?;

    if let Some(choice) = val.get("choices").and_then(|c| c.get(0)) {
        if let Some(content) = choice
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
        {
            let trimmed = content.trim();
            // Try to parse as JSON via helper
            return parse_planner_json(trimmed);
        }
    }
    Err(SurrealMindError::Internal {
        message: "No valid response from planner".into(),
    })
}

/// Call Grok chat/completions
#[allow(dead_code)]
pub(super) async fn call_grok(
    base: &str,
    model: &str,
    api_key: &str,
    messages: &serde_json::Value,
) -> Result<String> {
    let url = format!("{}/chat/completions", base.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "temperature": 0.2,
        "max_tokens": 400
    });
    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| SurrealMindError::Internal {
            message: format!("Failed to build HTTP client: {}", e),
        })?;
    let resp = client
        .post(url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| SurrealMindError::Internal {
            message: e.to_string(),
        })?;

    // Check response status before parsing
    let status = resp.status();
    if !status.is_success() {
        let body_text = resp
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read response body".to_string());
        check_http_status(status.as_u16(), &body_text, "Grok synthesis")?;
        unreachable!(); // check_http_status always returns an error for non-success
    }

    let val: serde_json::Value = resp.json().await.map_err(|e| SurrealMindError::Internal {
        message: e.to_string(),
    })?;
    if let Some(choice) = val.get("choices").and_then(|c| c.get(0)) {
        if let Some(content) = choice
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
        {
            return Ok(content.trim().to_string());
        }
    }
    // Fallback: return the raw JSON if format unexpected
    Ok(val.to_string())
}

/// Apply adaptive floor
pub fn apply_adaptive_floor(
    t_hits: &[Candidate],
    k_hits: &[Candidate],
    floor: f32,
    min_floor: f32,
    top_k: usize,
) -> (Vec<Candidate>, Vec<Candidate>, f32) {
    let mut floor_used = floor;

    // Sort by score desc
    let mut t_sorted: Vec<Candidate> = t_hits.to_vec();
    t_sorted.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut k_sorted: Vec<Candidate> = k_hits.to_vec();
    k_sorted.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // If we have candidates and total < top_k, try adaptive
    let total_hits = t_sorted.len() + k_sorted.len();
    if total_hits > 0 && total_hits < top_k && floor > min_floor {
        floor_used = (floor - 0.05).max(min_floor);
        // Re-filter with new floor
        t_sorted.retain(|c| c.score >= floor_used);
        k_sorted.retain(|c| c.score >= floor_used);
    }

    (t_sorted, k_sorted, floor_used)
}

/// Allocate slots by mix
pub fn allocate_slots(
    mix: f32,
    top_k: usize,
    k_hits: &[Candidate],
    t_hits: &[Candidate],
) -> (usize, usize) {
    // If one source is empty, allocate all to the other
    if k_hits.is_empty() {
        return (0, top_k);
    } else if t_hits.is_empty() {
        return (top_k, 0);
    }

    let kg_slots = (mix * top_k as f32).round() as usize;
    let thought_slots = top_k - kg_slots;

    // Guarantee at least one per source if both have hits
    if kg_slots == 0 {
        return (1, top_k - 1);
    } else if thought_slots == 0 {
        return (top_k - 1, 1);
    }

    (kg_slots, thought_slots)
}

/// Select and dedupe
pub fn select_and_dedupe(
    t_hits: Vec<Candidate>,
    k_hits: Vec<Candidate>,
    thought_slots: usize,
    kg_slots: usize,
) -> Vec<Candidate> {
    let mut selected = Vec::new();
    let mut seen_hashes = HashSet::new();
    let mut seen_ids = HashSet::new();

    // Take from KG first
    for cand in k_hits.into_iter().take(kg_slots) {
        let hash = hash_content(&cand.text);
        if !seen_hashes.contains(&hash)
            && !seen_ids.contains(&format!("{}:{}", cand.table, cand.id))
        {
            seen_hashes.insert(hash);
            seen_ids.insert(format!("{}:{}", cand.table, cand.id));
            selected.push(cand);
        }
    }

    // Then thoughts
    for cand in t_hits.into_iter().take(thought_slots) {
        let hash = hash_content(&cand.text);
        if !seen_hashes.contains(&hash)
            && !seen_ids.contains(&format!("{}:{}", cand.table, cand.id))
        {
            seen_hashes.insert(hash);
            seen_ids.insert(format!("{}:{}", cand.table, cand.id));
            selected.push(cand);
        }
    }

    selected
}

/// Cap text at sentence boundary
pub fn cap_text(text: &mut String, max_len: usize) {
    if text.len() <= max_len {
        return;
    }

    // Try to find sentence boundary
    if let Some(mat) = SENTENCE_REGEX.find_iter(text).next() {
        let end = mat.end();
        if end <= max_len {
            *text = text[..end].to_string();
            return;
        }
    }

    // Hard cut at UTF-8 boundary
    let mut end = max_len;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    if end == 0 {
        end = max_len; // Fallback
    }
    *text = text[..end].to_string();
}

/// Hash content for deduping
pub fn hash_content(text: &str) -> String {
    // Normalize: NFKC, lowercase, collapse whitespace, trim
    let normalized = text
        .nfkc()
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();

    let mut hasher = Hasher::new();
    hasher.update(normalized.as_bytes());
    hasher.finalize().to_hex().to_string()
}

/// Compute trust tier
pub fn compute_trust_tier(origin: &str, table: &str) -> String {
    if table.starts_with("kg_") {
        "green".to_string()
    } else {
        match origin {
            "human" | "logged" => "green".to_string(),
            "tool" => "amber".to_string(),
            _ => "red".to_string(),
        }
    }
}

/// Helper function to check HTTP response status and create appropriate error
pub(super) fn check_http_status(status_code: u16, body_text: &str, context: &str) -> Result<()> {
    if (200..300).contains(&status_code) {
        return Ok(());
    }

    if status_code == 429 {
        tracing::warn!("{} rate limited (429): {}", context, body_text);
    }

    Err(SurrealMindError::Internal {
        message: format!(
            "{} request failed with status {}: {}",
            context, status_code, body_text
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_status_checking() {
        // Test successful status
        assert!(check_http_status(200, "OK", "Test").is_ok());
        assert!(check_http_status(201, "Created", "Test").is_ok());
        assert!(check_http_status(299, "Custom", "Test").is_ok());

        // Test 429 rate limit error
        let result_429 = check_http_status(429, "Rate limit exceeded", "Grok planner");
        assert!(result_429.is_err());
        match result_429.unwrap_err() {
            SurrealMindError::Internal { message } => {
                assert!(message.contains("429"));
                assert!(message.contains("Rate limit exceeded"));
                assert!(message.contains("Grok planner"));
            }
            _ => panic!("Expected Internal error variant"),
        }

        // Test 500 internal server error
        let result_500 = check_http_status(500, "Internal server error", "Grok synthesis");
        assert!(result_500.is_err());
        match result_500.unwrap_err() {
            SurrealMindError::Internal { message } => {
                assert!(message.contains("500"));
                assert!(message.contains("Internal server error"));
                assert!(message.contains("Grok synthesis"));
            }
            _ => panic!("Expected Internal error variant"),
        }

        // Test 404 not found
        let result_404 = check_http_status(404, "Not found", "API");
        assert!(result_404.is_err());
        match result_404.unwrap_err() {
            SurrealMindError::Internal { message } => {
                assert!(message.contains("404"));
                assert!(message.contains("Not found"));
            }
            _ => panic!("Expected Internal error variant"),
        }
    }

    #[test]
    fn test_compute_trust_tier() {
        // Test KG tables get green tier
        assert_eq!(compute_trust_tier("any", "kg_entities"), "green");
        assert_eq!(compute_trust_tier("any", "kg_edges"), "green");

        // Test human/logged origin gets green tier
        assert_eq!(compute_trust_tier("human", "thoughts"), "green");
        assert_eq!(compute_trust_tier("logged", "thoughts"), "green");

        // Test tool origin gets amber tier
        assert_eq!(compute_trust_tier("tool", "thoughts"), "amber");

        // Test unknown origin gets red tier
        assert_eq!(compute_trust_tier("unknown", "thoughts"), "red");
    }
}
