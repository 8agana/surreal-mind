//! inner_voice tool handler for retrieval-only semantic search

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
use tokio::process::Command;
use unicode_normalization::UnicodeNormalization;

/// Parameters for the inner_voice tool
#[derive(Debug, serde::Deserialize)]
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

        // Synthesize answer — prefer Gemini CLI when configured, else Grok HTTP.
        let mut synthesized = String::new();
        let mut synth_provider = String::new();
        let mut synth_model = String::new();

        let provider_pref =
            std::env::var("IV_SYNTH_PROVIDER").unwrap_or_else(|_| "gemini_cli".to_string());

        // Helper: build a single-text prompt for CLI models from snippets
        fn build_cli_prompt(user_query: &str, snippets: &[Snippet]) -> String {
            let mut p = String::new();
            p.push_str("You are a precise synthesis engine.\n");
            p.push_str("Answer the user's question using ONLY the snippets.\n");
            p.push_str("Constraints: <=3 sentences; no hedging; no requests for more context; cite nothing.\n\n");
            p.push_str(&format!("Question: {}\n\n", user_query.trim()));
            p.push_str("Snippets:\n");
            for (i, s) in snippets.iter().enumerate() {
                let mut text = s.text.clone();
                cap_text(&mut text, 800);
                p.push_str(&format!("[{}] {}\n", i + 1, text));
            }
            p.push_str("\nAnswer:\n");
            p
        }

        // Try Gemini CLI first when requested (even if snippets are empty)
        if provider_pref.eq_ignore_ascii_case("gemini_cli") {
            let cli_cmd =
                std::env::var("IV_SYNTH_CLI_CMD").unwrap_or_else(|_| "gemini".to_string());
            let cli_model =
                std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.5-pro".to_string());
            let cli_args_json = std::env::var("IV_SYNTH_CLI_ARGS_JSON").unwrap_or_else(|_| {
                "[\"generate\",\"--model\",\"{model}\",\"--temperature\",\"0.2\"]".to_string()
            });
            let cli_timeout_ms: u64 = std::env::var("IV_SYNTH_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20_000);
            let cli_args: Vec<String> = serde_json::from_str(&cli_args_json).unwrap_or_else(|_| {
                vec![
                    "generate".into(),
                    "--model".into(),
                    "{model}".into(),
                    "--temperature".into(),
                    "0.2".into(),
                ]
            });

            let args: Vec<String> = cli_args
                .into_iter()
                .map(|a| if a == "{model}" { cli_model.clone() } else { a })
                .collect();

            // Spawn CLI and feed prompt via stdin
            match SurrealMindServer::synth_via_cli(
                &cli_cmd,
                &args,
                &build_cli_prompt(&params.query, &snippets),
                cli_timeout_ms,
            )
            .await
            {
                Ok(out) if !out.trim().is_empty() => {
                    synthesized = out.trim().to_string();
                    synth_provider = "gemini_cli".to_string();
                    synth_model = cli_model;
                }
                _ => { /* fall back to Grok below */ }
            }
        }

        // Grok HTTP fallback or primary if provider_pref != gemini_cli
        if synthesized.trim().is_empty() {
            let base = std::env::var("GROK_BASE_URL")
                .unwrap_or_else(|_| "https://api.x.ai/v1".to_string());
            let model =
                std::env::var("GROK_MODEL").unwrap_or_else(|_| "grok-code-fast-1".to_string());
            let grok_key = std::env::var("GROK_API_KEY").unwrap_or_default();
            let allow_grok =
                std::env::var("IV_ALLOW_GROK").unwrap_or_else(|_| "true".to_string()) != "false";
            let messages = build_synthesis_messages(&params.query, &snippets);
            if allow_grok && !grok_key.is_empty() {
                if let Ok(ans) = call_grok(&base, &model, &grok_key, &messages).await {
                    synthesized = ans;
                    synth_provider = "grok".to_string();
                    synth_model = model;
                }
            }
        }

        if synthesized.trim().is_empty() {
            // Last-resort fallback: minimal grounded summary style, no refusals
            if !snippets.is_empty() {
                let joined = snippets
                    .iter()
                    .take(3)
                    .map(|s| s.text.trim())
                    .collect::<Vec<_>>()
                    .join(" ");
                let summary: String = joined.chars().take(440).collect();
                synthesized = format!("Based on what I could find: {}", summary);
            } else {
                synthesized = "Based on what I could find, there wasn’t enough directly relevant material in the corpus to answer confidently.".to_string();
            }
            if synth_provider.is_empty() {
                synth_provider = "fallback".into();
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

        // Generate feedback prompt if enabled
        let include_feedback = params.include_feedback.unwrap_or(true);
        let feedback_max_lines = params.feedback_max_lines.unwrap_or(3);
        let (feedback_text, feedback_thought_id) = if include_feedback {
            // Generate feedback via Gemini CLI
            let feedback_prompt = format!(
                "Propose the single highest-impact next question that would improve the answer above. Keep it under 2 short lines. No bullets, no preamble.\n\nAnswer:\n{}",
                synthesized
            );
            let feedback_content = match self.generate_feedback_via_cli(&feedback_prompt).await {
                Ok(f) => f.trim().to_string(),
                Err(_) => "No feedback generated.".to_string(),
            };
            // Truncate to feedback_max_lines
            let truncated_feedback = feedback_content
                .lines()
                .take(feedback_max_lines)
                .collect::<Vec<_>>()
                .join("\n");
            // Persist feedback thought (Thought B)
            let feedback_embedding =
                self.embedder
                    .embed(&truncated_feedback)
                    .await
                    .map_err(|e| SurrealMindError::Embedding {
                        message: e.to_string(),
                    })?;
            let feedback_id = uuid::Uuid::new_v4().to_string();
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
                        origin: 'inner_voice.feedback',
                        embedding_provider: $provider,
                        embedding_model: $model,
                        embedding_dim: $dim,
                        embedded_at: time::now(),
                        previous_thought_id: $prev
                    } RETURN NONE;",
                )
                .bind(("id", feedback_id.clone()))
                .bind(("content", truncated_feedback.clone()))
                .bind(("embedding", feedback_embedding))
                .bind(("provider", provider))
                .bind(("model", model_name))
                .bind(("dim", dim))
                .bind(("prev", synth_thought_id.clone()))
                .await?;
            (truncated_feedback, Some(feedback_id))
        } else {
            (String::new(), None)
        };

        // Optional auto-extraction to KG candidates using Grok JSON extraction
        let auto_extract = params
            .auto_extract_to_kg
            .unwrap_or(self.config.runtime.inner_voice.auto_extract_default);
        let mut extracted_entities = 0usize;
        let mut extracted_rels = 0usize;
        if auto_extract {
            // Prefer CLI extractor when enabled; fall back to Grok when allowed
            // Default: CLI extractor enabled, but allow override via env
            let use_cli = std::env::var("IV_USE_CLI_EXTRACTOR")
                .map(|v| v.trim() != "false")
                .unwrap_or(true);
            let allow_grok =
                std::env::var("IV_ALLOW_GROK").unwrap_or_else(|_| "true".to_string()) != "false";

            if use_cli {
                if let Ok((ec, rc)) = self
                    .auto_extract_candidates_via_cli(&synthesized, &synth_thought_id)
                    .await
                {
                    tracing::debug!(
                        "inner_voice: CLI extractor staged candidates: entities={}, edges={}",
                        ec,
                        rc
                    );
                    extracted_entities = ec;
                    extracted_rels = rc;
                }
            }

            if (extracted_entities == 0 && extracted_rels == 0) && allow_grok {
                let grok_base = std::env::var("GROK_BASE_URL")
                    .unwrap_or_else(|_| "https://api.x.ai/v1".to_string());
                let grok_model =
                    std::env::var("GROK_MODEL").unwrap_or_else(|_| "grok-code-fast-1".to_string());
                let grok_key_ex = std::env::var("GROK_API_KEY").unwrap_or_default();
                if !grok_key_ex.is_empty() {
                    if let Ok((ec, rc)) = self
                        .auto_extract_candidates_from_text(
                            &grok_base,
                            &grok_model,
                            &grok_key_ex,
                            &synthesized,
                            &synth_thought_id,
                        )
                        .await
                    {
                        tracing::debug!(
                            "inner_voice: Grok fallback staged candidates: entities={}, edges={}",
                            ec,
                            rc
                        );
                        extracted_entities = ec;
                        extracted_rels = rc;
                    }
                }
            }

            // Optional HeuristicExtractor fallback
            if extracted_entities == 0 && extracted_rels == 0 {
                let heuristic_enabled = std::env::var("SURR_IV_HEURISTIC_FALLBACK")
                    .map(|v| v != "0")
                    .unwrap_or(true);
                if heuristic_enabled {
                    if let Ok((ec, rc)) = self
                        .heuristic_extract(&synthesized, &synth_thought_id)
                        .await
                    {
                        tracing::debug!(
                            "inner_voice: Heuristic fallback staged candidates: entities={}, edges={}",
                            ec,
                            rc
                        );
                        extracted_entities = ec;
                        extracted_rels = rc;
                    }
                }
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

    /// Generate feedback prompt via CLI
    async fn generate_feedback_via_cli(&self, prompt: &str) -> Result<String> {
        let cli_cmd = std::env::var("IV_SYNTH_CLI_CMD").unwrap_or_else(|_| "gemini".to_string());
        let cli_model =
            std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.5-pro".to_string());
        let cli_args_json = std::env::var("IV_SYNTH_CLI_ARGS_JSON")
            .unwrap_or_else(|_| "[\"-m\",\"{model}\"]".to_string());
        let cli_timeout_ms: u64 = std::env::var("IV_SYNTH_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20_000);
        let cli_args: Vec<String> = serde_json::from_str(&cli_args_json)
            .unwrap_or_else(|_| vec!["-m".into(), "{model}".into()]);

        let args: Vec<String> = cli_args
            .into_iter()
            .map(|a| if a == "{model}" { cli_model.clone() } else { a })
            .collect();

        Self::synth_via_cli(&cli_cmd, &args, prompt, cli_timeout_ms).await
    }

    /// HeuristicExtractor fallback
    async fn heuristic_extract(&self, text: &str, thought_id: &str) -> Result<(usize, usize)> {
        // Simple pattern-based extraction
        let entities_cap = std::env::var("SURR_IV_HEURISTIC_MAX_ENTITIES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20);
        let edges_cap = std::env::var("SURR_IV_HEURISTIC_MAX_EDGES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        let mut entities = Vec::new();
        let mut edges = Vec::new();

        // Basic entity extraction (capitalized words)
        for word in text.split_whitespace() {
            if word.chars().next().is_some_and(|c| c.is_uppercase()) && word.len() > 2 {
                entities.push(word.to_string());
                if entities.len() >= entities_cap {
                    break;
                }
            }
        }

        // Basic relationships (simple patterns)
        let patterns = ["uses", "depends on", "related to", "->"];
        for pattern in &patterns {
            if let Some(pos) = text.find(pattern) {
                let before = &text[..pos];
                let after = &text[pos + pattern.len()..];
                if let Some(src) = before.split_whitespace().last() {
                    if let Some(dst) = after.split_whitespace().next() {
                        edges.push((src.to_string(), dst.to_string()));
                        if edges.len() >= edges_cap {
                            break;
                        }
                    }
                }
            }
        }

        // Stage with low confidence
        let mut ecount = 0;
        for name in entities.into_iter().take(entities_cap) {
            let _ = self.db.query("CREATE kg_entity_candidates SET created_at = time::now(), name = $n, entity_type = 'unknown', confidence = 0.7, status = 'pending', data = { staged_by_thought: $th, origin: 'inner_voice' }")
                .bind(("n", name))
                .bind(("th", thought_id.to_string()))
                .await;
            ecount += 1;
        }

        let mut rcount = 0;
        for (src, dst) in edges.into_iter().take(edges_cap) {
            let _ = self.db.query("CREATE kg_edge_candidates SET created_at = time::now(), source_name = $s, target_name = $t, rel_type = 'related_to', confidence = 0.6, status = 'pending', data = { staged_by_thought: $th, origin: 'inner_voice' }")
                .bind(("s", src))
                .bind(("t", dst))
                .bind(("th", thought_id.to_string()))
                .await;
            rcount += 1;
        }

        Ok((ecount, rcount))
    }

    /// Spawn a local CLI (e.g., `gemini`) to synthesize an answer from grounded snippets
    async fn synth_via_cli(
        cmd: &str,
        args: &[String],
        prompt: &str,
        timeout_ms: u64,
    ) -> Result<String> {
        use tokio::io::AsyncWriteExt;
        use tokio::time::{Duration, timeout};

        let mut child = Command::new(cmd)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| SurrealMindError::Internal {
                message: format!("failed to spawn CLI '{}': {}", cmd, e),
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(prompt.as_bytes())
                .await
                .map_err(|e| SurrealMindError::Internal {
                    message: format!("failed to write prompt to CLI: {}", e),
                })?;
        }

        let out = timeout(Duration::from_millis(timeout_ms), child.wait_with_output())
            .await
            .map_err(|_| SurrealMindError::Timeout {
                operation: "cli_synthesis".into(),
                timeout_ms,
            })
            .and_then(|r| {
                r.map_err(|e| SurrealMindError::Internal {
                    message: format!("CLI synthesis failed: {}", e),
                })
            })?;

        if !out.status.success() {
            return Err(SurrealMindError::Internal {
                message: format!("CLI exited with status {}", out.status),
            });
        }

        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        Ok(stdout)
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
struct ExtractOut {
    #[serde(default)]
    entities: Vec<ExtractEntity>,
    #[serde(default)]
    relationships: Vec<ExtractRel>,
}

#[derive(Debug, Deserialize)]
struct ExtractEntity {
    name: String,
    #[serde(default)]
    entity_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExtractRel {
    source_name: String,
    target_name: String,
    #[serde(default)]
    rel_type: Option<String>,
    #[serde(default)]
    confidence: Option<f32>,
}

impl SurrealMindServer {
    /// Use CLI (Gemini-first) to extract candidate entities/relationships and stage them into *_candidates tables
    pub async fn auto_extract_candidates_via_cli(
        &self,
        text: &str,
        thought_id: &str,
    ) -> Result<(usize, usize)> {
        // Preflight: require Node to be available; if missing, disable CLI path
        if !self.cli_prereqs_ok().await {
            tracing::warn!(target: "inner_voice", "CLI extractor prerequisites missing (node). Skipping CLI and allowing fallback.");
            return Ok((0, 0));
        }

        use std::process::Stdio;
        use tokio::process::Command;
        // Prepare input payload
        let mut hasher = Hasher::new();
        hasher.update(text.as_bytes());
        let prompt_hash = hasher.finalize().to_hex().to_string();
        let input = serde_json::json!({
            "synth_text": text,
            "doc_id": thought_id,
            "prompt_hash": prompt_hash,
        });

        // Write to a temp file
        let tmp_path = std::env::temp_dir().join(format!("iv_in_{}.json", thought_id));
        let payload = serde_json::to_vec(&input)?;
        std::fs::write(&tmp_path, payload).map_err(|e| SurrealMindError::Internal {
            message: format!("Failed to write temp file {}: {}", tmp_path.display(), e),
        })?;

        // Execute Node script
        let start = Instant::now();
        let script_path =
            std::env::var("IV_SCRIPT_PATH").unwrap_or_else(|_| "scripts/iv_extract.js".to_string());
        let mut cmd = Command::new("node");
        cmd.arg(&script_path)
            .arg("--input")
            .arg(&tmp_path)
            .arg("--out")
            .arg("-")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = cmd.spawn().map_err(|e| SurrealMindError::Internal {
            message: format!("Failed to spawn CLI extractor: {}", e),
        })?;
        let out = child
            .wait_with_output()
            .await
            .map_err(|e| SurrealMindError::Internal {
                message: format!("CLI extractor wait failed: {}", e),
            })?;
        let latency = start.elapsed().as_millis() as u64;

        // Clean up temp file best-effort
        let _ = std::fs::remove_file(&tmp_path);

        if !out.status.success() {
            let stderr_snip = String::from_utf8_lossy(&out.stderr)
                .chars()
                .take(500)
                .collect::<String>();
            let stdout_snip = String::from_utf8_lossy(&out.stdout)
                .chars()
                .take(500)
                .collect::<String>();
            tracing::debug!(
                cmd = %script_path,
                code = ?out.status.code(),
                stderr_snip = %stderr_snip,
                stdout_snip = %stdout_snip,
                latency_ms = latency,
                "inner_voice.extract_fail"
            );
            return Ok((0, 0));
        }
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        tracing::debug!("inner_voice: CLI extractor produced {} bytes", stdout.len());
        let parsed: serde_json::Value =
            serde_json::from_str(&stdout).unwrap_or(serde_json::json!({
                "entities": [],
                "edges": []
            }));
        let entities = parsed
            .get("entities")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let edges = parsed
            .get("edges")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // Map entity ids to labels for edge name resolution
        use std::collections::HashMap;
        let mut id_to_label: HashMap<String, String> = HashMap::new();
        for e in &entities {
            let id = e
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let label = e
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !id.is_empty() && !label.is_empty() {
                id_to_label.insert(id, label);
            }
        }

        // Stage entities (deterministic IDs for idempotency)
        let mut ecount = 0usize;
        for e in entities {
            let name = e
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if name.is_empty() {
                continue;
            }
            let etype = e
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            // Stable id key: sha1(doc_id|name|etype)
            let mut h = Hasher::new();
            h.update(thought_id.as_bytes());
            h.update(b"|");
            h.update(name.as_bytes());
            h.update(b"|");
            h.update(etype.as_bytes());
            let key = h.finalize().to_hex().to_string();

            let existing: Vec<serde_json::Value> = self
                .db
                .query("SELECT meta::id(id) as id FROM type::thing('kg_entity_candidates', $id)")
                .bind(("id", key.clone()))
                .await?
                .take(0)?;
            if existing.is_empty() {
                // Create with deterministic id; if a race occurs and record exists, ignore error
                let _ = self
                    .db
                    .query("CREATE type::thing('kg_entity_candidates', $id) SET created_at = time::now(), name = $n, entity_type = $t, confidence = 0.6, status = 'pending', data = { staged_by_thought: $th, origin: 'inner_voice' }")
                    .bind(("id", key))
                    .bind(("n", name))
                    .bind(("t", etype))
                    .bind(("th", thought_id.to_string()))
                    .await;
                ecount += 1;
            }
        }

        let mut rcount = 0usize;
        for r in edges {
            let from_id = r.get("from_id").and_then(|v| v.as_str()).unwrap_or("");
            let to_id = r.get("to_id").and_then(|v| v.as_str()).unwrap_or("");
            let kind = r
                .get("relation")
                .and_then(|v| v.as_str())
                .unwrap_or("related_to")
                .to_string();
            let src = id_to_label.get(from_id).cloned().unwrap_or_default();
            let dst = id_to_label.get(to_id).cloned().unwrap_or_default();
            if src.is_empty() || dst.is_empty() {
                continue;
            }
            let conf = r
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.6_f64) as f32;

            // Stable edge id key: sha1(doc_id|src|dst|kind)
            let mut h = Hasher::new();
            h.update(thought_id.as_bytes());
            h.update(b"|");
            h.update(src.as_bytes());
            h.update(b"|");
            h.update(dst.as_bytes());
            h.update(b"|");
            h.update(kind.as_bytes());
            let key = h.finalize().to_hex().to_string();

            let existing: Vec<serde_json::Value> = self
                .db
                .query("SELECT meta::id(id) as id FROM type::thing('kg_edge_candidates', $id)")
                .bind(("id", key.clone()))
                .await?
                .take(0)?;
            if existing.is_empty() {
                let _ = self
                    .db
                    .query("CREATE type::thing('kg_edge_candidates', $id) SET created_at = time::now(), source_name = $s, target_name = $t, rel_type = $k, confidence = $c, status = 'pending', data = { staged_by_thought: $th, origin: 'inner_voice' }")
                    .bind(("id", key))
                    .bind(("s", src))
                    .bind(("t", dst))
                    .bind(("k", kind))
                    .bind(("c", conf))
                    .bind(("th", thought_id.to_string()))
                    .await;
                rcount += 1;
            }
        }

        Ok((ecount, rcount))
    }

    /// Lightweight preflight: ensure Node is present; Gemini CLI availability is handled by the Node runner
    async fn cli_prereqs_ok(&self) -> bool {
        use tokio::process::Command;
        match Command::new("node").arg("--version").output().await {
            Ok(o) => o.status.success(),
            Err(_) => false,
        }
    }

    /// Use Grok to extract candidate entities/relationships and stage them into *_candidates tables
    pub async fn auto_extract_candidates_from_text(
        &self,
        base: &str,
        model: &str,
        api_key: &str,
        text: &str,
        thought_id: &str,
    ) -> Result<(usize, usize)> {
        let messages = build_extraction_messages(text);
        let out = call_grok(base, model, api_key, &messages).await?;
        // Parse JSON; Grok may return markdown fences; strip if present
        let cleaned = out
            .trim()
            .trim_start_matches("```json")
            .trim_end_matches("```")
            .trim()
            .to_string();
        let parsed: ExtractOut = serde_json::from_str(&cleaned).unwrap_or(ExtractOut {
            entities: vec![],
            relationships: vec![],
        });

        let mut ecount = 0usize;
        for e in parsed.entities {
            let name = e.name.trim().to_string();
            if name.is_empty() {
                continue;
            }
            let etype = e.entity_type.clone().unwrap_or_default();
            // Dedup by existing pending with same name+etype
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
                    .query("CREATE kg_entity_candidates SET created_at = time::now(), name = $n, entity_type = $t, confidence = 0.6, status = 'pending', data = { staged_by_thought: $th, origin: 'inner_voice' } RETURN meta::id(id) as id")
                    .bind(("n", name))
                    .bind(("t", etype))
                    .bind(("th", thought_id.to_string()))
                    .await?
                    .take(0)?;
                ecount += 1;
            }
        }

        let mut rcount = 0usize;
        for r in parsed.relationships {
            let src = r.source_name.trim().to_string();
            let dst = r.target_name.trim().to_string();
            if src.is_empty() || dst.is_empty() {
                continue;
            }
            let kind = r
                .rel_type
                .clone()
                .unwrap_or_else(|| "related_to".to_string());
            let conf = r.confidence.unwrap_or(0.6_f32);
            // Dedup by same names+rel_type and status pending
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

        Ok((ecount, rcount))
    }
}

fn build_extraction_messages(text: &str) -> serde_json::Value {
    json!({
        "messages": [
            {"role": "system", "content": "You extract entities and relationships from text and return only JSON exactly matching the schema. No extra commentary."},
            {"role": "user", "content": format!("Extract from the following text. Return JSON: {{\n  \"entities\": [{{\"name\": string, \"entity_type\"?: string}}],\n  \"relationships\": [{{\"source_name\": string, \"target_name\": string, \"rel_type\"?: string, \"confidence\"?: number}}]\n}}\n\nTEXT:\n{}", text) }
        ]
    })
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

    let system = "You are a careful, grounded synthesizer. Only use the provided snippets. Cite sources inline like [1], [2]. Prefer concise answers (<= 4 sentences). If insufficient evidence, say so.";
    let user = format!(
        "Query: {}\n\nSnippets:\n{}\n\nTask: Provide a concise, grounded answer with inline [n] citations.",
        query,
        lines.join("\n\n")
    );

    serde_json::json!([
        {"role": "system", "content": system},
        {"role": "user", "content": user}
    ])
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
            // Try to parse as JSON
            match serde_json::from_str::<PlannerResponse>(trimmed) {
                Ok(planner) => {
                    // Validate required field
                    if planner.rewritten_query.trim().is_empty() {
                        return Err(SurrealMindError::Internal {
                            message: "Planner returned empty rewritten_query".into(),
                        });
                    }
                    return Ok(planner);
                }
                Err(e) => {
                    return Err(SurrealMindError::Internal {
                        message: format!("Failed to parse planner JSON: {}", e),
                    });
                }
            }
        }
    }
    Err(SurrealMindError::Internal {
        message: "No valid response from planner".into(),
    })
}

/// Call Grok chat/completions
async fn call_grok(
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
