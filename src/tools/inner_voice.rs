//! inner_voice tool handler for retrieval-only semantic search

use crate::error::{Result, SurrealMindError};
use crate::schemas::Snippet;
use crate::server::SurrealMindServer;
use blake3::Hasher;
use once_cell::sync::Lazy;
use regex::Regex;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;
use std::time::Instant;
use unicode_normalization::UnicodeNormalization;
use reqwest::Client;

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
static SENTENCE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r#"[.!?]["”"']?\s"#).unwrap());

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

        let start_time = Instant::now();

        // Config
        let cfg = &self.config.runtime.inner_voice;
        let top_k = params.top_k.unwrap_or(cfg.topk_default).clamp(1, 50);
        let floor = params.floor.unwrap_or(cfg.min_floor).clamp(0.0, 1.0);
        let mix = params.mix.unwrap_or(cfg.mix).clamp(0.0, 1.0);
        let include_private = params
            .include_private
            .unwrap_or(cfg.include_private_default);

        // Embed query
        let q_emb = self.embedder.embed(&params.query).await.map_err(|e| {
            SurrealMindError::EmbedderUnavailable {
                message: e.to_string(),
            }
        })?;
        let q_dim = q_emb.len() as i64;

        // Fetch candidates
        let cap = (3 * top_k).min(cfg.max_candidates_per_source);
        let thought_candidates = self
            .fetch_thought_candidates(&params, cap, q_dim, include_private)
            .await?;
        let kg_entity_candidates = self.fetch_kg_entity_candidates(&params, cap, q_dim).await?;
        let kg_obs_candidates = self
            .fetch_kg_observation_candidates(&params, cap, q_dim)
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
                let score = cosine(&q_emb, &cand.embedding);
                if score >= floor {
                    let mut c = cand;
                    c.score = score;
                    kg_hits.push(c);
                }
            }
        }

        // Adaptive floor if needed
        let (t_hits, k_hits, floor_used) =
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
        selected.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

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

        // Synthesize with Grok (required behavior). If missing key, produce a gentle self-question.
        let base = std::env::var("GROK_BASE_URL").unwrap_or_else(|_| "https://api.x.ai/v1".to_string());
        let model = std::env::var("GROK_MODEL").unwrap_or_else(|_| "grok-code-fast-1".to_string());
        let grok_key = std::env::var("GROK_API_KEY").unwrap_or_default();
        let messages = build_synthesis_messages(&params.query, &snippets);
        let mut synthesized = String::new();
        if !grok_key.is_empty() && !snippets.is_empty() {
            if let Ok(ans) = call_grok(&base, &model, &grok_key, &messages).await {
                synthesized = ans;
            }
        }
        if synthesized.trim().is_empty() {
            // Fallback: ask a clarifying question rather than erroring
            synthesized = format!(
                "I may need more context to answer precisely. What specific aspect of ‘{}’ should I focus on?",
                params.query.trim()
            );
        }

        // Minimal citations line from internal selections
        let mut ids: Vec<String> = Vec::new();
        for c in &selected {
            let prefix = match c.table.as_str() {
                "thoughts" => "thoughts:",
                "kg_entities" => "kge:",
                "kg_observations" => "kgo:",
                other => if other.len() > 3 { &other[0..3] } else { other },
            };
            ids.push(format!("{}{}", prefix, c.id));
        }
        ids.truncate(6); // keep short
        if !ids.is_empty() {
            synthesized.push_str("\n\nSources: ");
            synthesized.push_str(&ids.join(", "));
        }

        // Embed synthesized content and save as a new thought (origin = inner_voice), injection disabled
        let embedding = self.embedder.embed(&synthesized).await.map_err(|e| {
            SurrealMindError::Embedding { message: e.to_string() }
        })?;
        let thought_id = uuid::Uuid::new_v4().to_string();
        let (provider, model_name, dim) = self.get_embedding_metadata();
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
                    embedded_at: time::now()
                } RETURN NONE;",
            )
            .bind(("id", thought_id.clone()))
            .bind(("content", synthesized.clone()))
            .bind(("embedding", embedding))
            .bind(("provider", provider))
            .bind(("model", model_name.clone()))
            .bind(("dim", dim))
            .await?;

        let result = json!({
            "thought_id": thought_id,
            "embedding_model": model_name,
            "embedding_dim": self.embedder.dimensions()
        });

        Ok(CallToolResult::structured(result))
    }

    async fn fetch_thought_candidates(
        &self,
        params: &InnerVoiceRetrieveParams,
        cap: usize,
        q_dim: i64,
        include_private: bool,
    ) -> Result<Vec<Candidate>> {
        let mut sql = "SELECT meta::id(id) AS id, content, embedding, created_at, origin ?? 'human' AS origin, tags ?? [] AS tags, is_private ?? false AS is_private FROM thoughts WHERE embedding_dim = $dim".to_string();

        if !include_private {
            sql.push_str(" AND is_private != true");
        }

        let mut query = self.db.query(&sql).bind(("dim", q_dim));

        if !params.include_tags.is_empty() {
            sql.push_str(" AND (");
            for (i, _) in params.include_tags.iter().enumerate() {
                if i > 0 {
                    sql.push_str(" OR ");
                }
                sql.push_str(&format!("$tag{} IN tags", i));
            }
            sql.push(')');
        }

        if !params.exclude_tags.is_empty() {
            for (i, _) in params.exclude_tags.iter().enumerate() {
                sql.push_str(&format!(" AND $etag{} NOT IN tags", i));
            }
        }

        sql.push_str(" LIMIT $limit");

        // Bind tags
        for (i, tag) in params.include_tags.iter().enumerate() {
            query = query.bind((format!("tag{}", i), tag.clone()));
        }
        for (i, tag) in params.exclude_tags.iter().enumerate() {
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
    ) -> Result<Vec<Candidate>> {
        let sql = "SELECT meta::id(id) AS id, name ?? 'unknown' AS content, embedding, created_at FROM kg_entities WHERE embedding IS NOT NULL AND embedding_dim = $dim LIMIT $limit";

        let mut response = self
            .db
            .query(sql)
            .bind(("dim", q_dim))
            .bind(("limit", cap as i64))
            .await?;

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
    ) -> Result<Vec<Candidate>> {
        let sql = "SELECT meta::id(id) AS id, content ?? 'unknown' AS content, embedding, created_at FROM kg_observations WHERE embedding IS NOT NULL AND embedding_dim = $dim LIMIT $limit";

        let mut response = self
            .db
            .query(sql)
            .bind(("dim", q_dim))
            .bind(("limit", cap as i64))
            .await?;

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

/// Compute cosine similarity
fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0;
    let mut na = 0.0;
    let mut nb = 0.0;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
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

/// Call Grok chat/completions
async fn call_grok(base: &str, model: &str, api_key: &str, messages: &serde_json::Value) -> Result<String> {
    let url = format!("{}/chat/completions", base.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "temperature": 0.2,
        "max_tokens": 400
    });
    let client = Client::new();
    let resp = client
        .post(url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| SurrealMindError::Internal { message: e.to_string() })?;
    let val: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| SurrealMindError::Internal { message: e.to_string() })?;
    if let Some(choice) = val.get("choices").and_then(|c| c.get(0)) {
        if let Some(content) = choice.get("message").and_then(|m| m.get("content")).and_then(|c| c.as_str()) {
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
    t_sorted.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    let mut k_sorted: Vec<Candidate> = k_hits.to_vec();
    k_sorted.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

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
