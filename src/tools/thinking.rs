//! thinking module: common run_* helpers for think tools and new legacymind_think

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use anyhow::Context;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;
use std::collections::HashSet;
use std::time::Instant;

/// Maximum content size in bytes (100KB)
const MAX_CONTENT_SIZE: usize = 100 * 1024;

/// Default contradiction patterns for hypothesis verification (case-insensitive)
const CONTRADICTION_PATTERNS: &[&str] = &[
    "not",
    "no",
    "cannot",
    "false",
    "incorrect",
    "fails",
    "broken",
    "doesn't",
    "isn't",
    "won't",
];

/// Evidence item for hypothesis verification
#[derive(Debug, Clone, serde::Serialize)]
pub struct EvidenceItem {
    pub table: String,
    pub id: String,
    pub text: String,
    pub similarity: f32,
    pub provenance: Option<serde_json::Value>,
}

/// Verification result for hypothesis verification
#[derive(Debug, Clone, serde::Serialize)]
pub struct VerificationResult {
    pub hypothesis: String,
    pub supporting: Vec<EvidenceItem>,
    pub contradicting: Vec<EvidenceItem>,
    pub confidence_score: f32,
    pub suggested_revision: Option<String>,
    pub telemetry: serde_json::Value,
}

/// Modes for legacymind_think routing
#[derive(Debug, Clone, PartialEq)]
enum ThinkMode {
    Debug,
    Build,
    Plan,
    Stuck,
    Question,
    Conclude,
}

/// Parameters for legacymind_think
#[derive(Debug, serde::Deserialize)]
pub struct LegacymindThinkParams {
    pub content: String,
    #[serde(default)]
    pub hint: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_u8_forgiving"
    )]
    pub injection_scale: Option<u8>,
    #[serde(default, deserialize_with = "crate::deserializers::de_option_tags")]
    pub tags: Option<Vec<String>>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_f32_forgiving"
    )]
    pub significance: Option<f32>,
    #[serde(default)]
    pub verbose_analysis: Option<bool>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub chain_id: Option<String>,
    #[serde(default)]
    pub previous_thought_id: Option<String>,
    #[serde(default)]
    pub revises_thought: Option<String>,
    #[serde(default)]
    pub branch_from: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_f32_forgiving"
    )]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub hypothesis: Option<String>,
    #[serde(default)]
    pub needs_verification: Option<bool>,
    #[serde(default)]
    pub verify_top_k: Option<usize>,
    #[serde(default)]
    pub min_similarity: Option<f32>,
    #[serde(default)]
    pub evidence_limit: Option<usize>,
    #[serde(default)]
    pub contradiction_patterns: Option<Vec<String>>,
}

/// Result struct for continuity links resolution
#[derive(Debug, serde::Serialize)]
pub struct ContinuityResult {
    pub session_id: Option<String>,
    pub chain_id: Option<String>,
    pub previous_thought_id: Option<String>,
    pub revises_thought: Option<String>,
    pub branch_from: Option<String>,
    pub confidence: Option<f32>,
    pub links_resolved: serde_json::Value,
}

/// Process a database query result for continuity link resolution
/// Takes the original ID and the query result, returns (resolved_id, resolution_type)
/// When the query result is empty, preserves the ID as a string for future resolution
pub fn process_continuity_query_result(
    original_id: String,
    query_result: Vec<serde_json::Value>,
) -> (Option<String>, &'static str) {
    // Normalize the ID format
    let normalized_id = if original_id.starts_with("thoughts:") {
        original_id
    } else {
        format!("thoughts:{}", original_id)
    };

    // Check if the record exists based on query result
    if !query_result.is_empty() {
        // Record found in database
        (Some(normalized_id), "record")
    } else {
        // Record not found - preserve as string for future resolution
        tracing::warn!(
            "Continuity link {} not found in database, preserving as string for future resolution",
            normalized_id
        );
        (Some(normalized_id), "string")
    }
}

/// Builder for creating thoughts with consistent logic
pub struct ThoughtBuilder<'a> {
    server: &'a SurrealMindServer,
    content: String,
    origin: String,
    injection_scale: i64,
    significance: f64,
    tags: Vec<String>,
    confidence: Option<f32>,
    // Continuity params
    session_id: Option<String>,
    chain_id: Option<String>,
    previous_thought_id: Option<String>,
    revises_thought: Option<String>,
    branch_from: Option<String>,
}

impl<'a> ThoughtBuilder<'a> {
    pub fn new(server: &'a SurrealMindServer, content: &str, origin: &str) -> Self {
        Self {
            server,
            content: content.to_string(),
            origin: origin.to_string(),
            injection_scale: 1,
            significance: 0.5,
            tags: Vec::new(),
            confidence: None,
            session_id: None,
            chain_id: None,
            previous_thought_id: None,
            revises_thought: None,
            branch_from: None,
        }
    }

    pub fn scale(mut self, scale: Option<u8>) -> Self {
        self.injection_scale = scale.unwrap_or(1) as i64;
        self
    }

    pub fn significance(mut self, sig: Option<f32>) -> Self {
        self.significance = sig.unwrap_or(0.5) as f64;
        self
    }

    pub fn tags(mut self, tags: Option<Vec<String>>) -> Self {
        self.tags = tags.unwrap_or_default();
        self
    }

    pub fn confidence(mut self, conf: Option<f32>) -> Self {
        self.confidence = conf.map(|c| c.clamp(0.0, 1.0));
        self
    }

    pub fn continuity(
        mut self,
        session_id: Option<String>,
        chain_id: Option<String>,
        previous_thought_id: Option<String>,
        revises_thought: Option<String>,
        branch_from: Option<String>,
    ) -> Self {
        self.session_id = session_id;
        self.chain_id = chain_id;
        self.previous_thought_id = previous_thought_id;
        self.revises_thought = revises_thought;
        self.branch_from = branch_from;
        self
    }

    /// Execute the build process: embed, resolve links, and create record
    pub async fn execute(self) -> Result<(String, Vec<f32>, ContinuityResult)> {
        let thought_id = uuid::Uuid::new_v4().to_string();
        let (provider, model, dim) = self.server.get_embedding_metadata();

        // Compute embedding
        let embedding = self
            .server
            .embedder
            .embed(&self.content)
            .await
            .map_err(|e| SurrealMindError::Embedding {
                message: e.to_string(),
            })?;

        if embedding.is_empty() {
            return Err(SurrealMindError::Embedding {
                message: "Generated embedding is empty".into(),
            });
        }

        // Resolve continuity links
        let mut resolved_continuity = self
            .server
            .resolve_continuity_links(
                &thought_id,
                self.previous_thought_id,
                self.revises_thought,
                self.branch_from,
            )
            .await?;
        resolved_continuity.session_id = self.session_id;
        resolved_continuity.chain_id = self.chain_id;
        resolved_continuity.confidence = self.confidence;

        // Create thought with all fields
        self.server
            .db
            .query(
                "CREATE type::thing('thoughts', $id) CONTENT {
            content: $content,
            created_at: time::now(),
            embedding: $embedding,
            injected_memories: [],
            enriched_content: NONE,
            injection_scale: $injection_scale,
            significance: $significance,
            access_count: 0,
            last_accessed: NONE,
            submode: NONE,
            framework_enhanced: NONE,
            framework_analysis: NONE,
            origin: $origin,
            tags: $tags,
            is_private: false,
            embedding_provider: $provider,
            embedding_model: $model,
            embedding_dim: $dim,
            embedded_at: time::now(),
            session_id: $session_id,
            chain_id: $chain_id,
            previous_thought_id: $previous_thought_id,
            revises_thought: $revises_thought,
            branch_from: $branch_from,
            confidence: $confidence
        } RETURN NONE;",
            )
            .bind(("id", thought_id.clone()))
            .bind(("content", self.content))
            .bind(("embedding", embedding.clone()))
            .bind(("injection_scale", self.injection_scale))
            .bind(("significance", self.significance))
            .bind(("origin", self.origin))
            .bind(("tags", self.tags))
            .bind(("provider", provider))
            .bind(("model", model))
            .bind(("dim", dim))
            .bind(("session_id", resolved_continuity.session_id.clone()))
            .bind(("chain_id", resolved_continuity.chain_id.clone()))
            .bind((
                "previous_thought_id",
                resolved_continuity.previous_thought_id.clone(),
            ))
            .bind((
                "revises_thought",
                resolved_continuity.revises_thought.clone(),
            ))
            .bind(("branch_from", resolved_continuity.branch_from.clone()))
            .bind(("confidence", resolved_continuity.confidence))
            .await?;

        Ok((thought_id, embedding, resolved_continuity))
    }
}

impl SurrealMindServer {
    /// Run conversational think (with framework enhancement, origin='human')
    #[allow(clippy::too_many_arguments)]
    pub async fn run_convo(
        &self,
        content: &str,
        injection_scale: Option<u8>,
        tags: Option<Vec<String>>,
        significance: Option<f32>,
        verbose_analysis: Option<bool>,
        is_conclude: bool,
        session_id: Option<String>,
        chain_id: Option<String>,
        previous_thought_id: Option<String>,
        revises_thought: Option<String>,
        branch_from: Option<String>,
        confidence: Option<f32>,
    ) -> Result<(serde_json::Value, ContinuityResult)> {
        let injection_scale_val = injection_scale.unwrap_or(1) as i64;
        let tags = tags.unwrap_or_default();
        // let content_str = content.to_string(); // Unused while frameworks disabled

        // Use ThoughtBuilder to create the thought
        let (thought_id, embedding, resolved_continuity) =
            ThoughtBuilder::new(self, content, "human")
                .scale(Some(injection_scale_val as u8))
                .tags(Some(tags.clone()))
                .significance(significance)
                .confidence(confidence)
                .continuity(
                    session_id,
                    chain_id,
                    previous_thought_id,
                    revises_thought,
                    branch_from,
                )
                .execute()
                .await?;

        // Framework enhancement (Temporarily DISABLED due to module deletion)
        // This will be re-implemented using the new src/cognitive module in a future step
        let enhance_enabled = false; 
            // !is_conclude && std::env::var("SURR_THINK_ENHANCE").unwrap_or("1".to_string()) == "1";
        
        let verbose_analysis = verbose_analysis.unwrap_or(false);
        let framework_enhanced = false;
        let framework_analysis: Option<serde_json::Value> = None;
        
        if enhance_enabled || verbose_analysis {
            tracing::warn!("Framework enhancement momentarily disabled pending architecture update");
        }

        /* 
        // LEGACY FRAMEWORK CODE - REMOVED TO ALLOW COMPILATION
        // TODO: Re-wire this to use src/cognitive
        if enhance_enabled || verbose_analysis {
            tracing::debug!("Running framework enhancement for thought {}", thought_id);
            let _start = Instant::now();
            let opts = ConvoOpts { ... };
            match tokio::time::timeout(...) { ... }
        }
        */

        // Update thought with enhancement results and merge tags if enhanced
        if framework_enhanced || framework_analysis.is_some() {
            let mut query = "UPDATE type::thing('thoughts', $id) SET framework_enhanced = $enhanced, framework_analysis = $analysis".to_string();
            let mut binds = vec![
                ("id", serde_json::Value::String(thought_id.clone())),
                ("enhanced", serde_json::Value::Bool(framework_enhanced)),
                (
                    "analysis",
                    framework_analysis
                        .clone()
                        .unwrap_or(serde_json::Value::Null),
                ),
            ];
            if framework_enhanced
                && let Some(env) = framework_analysis.as_ref().and_then(|a| a.as_object())
                && let Some(data) = env.get("data").and_then(|d| d.as_object())
                && let Some(tags_from_analysis) = data.get("tags").and_then(|t| t.as_array())
            {
                // Merge tags, then filter by whitelist to ensure only allowed tags persist
                let existing_tags: Vec<String> = tags.clone();
                let envelope_tags: Vec<String> = tags_from_analysis
                    .iter()
                    .filter_map(|t| t.as_str())
                    .map(|s| s.to_string())
                    .collect();
                let mut merged_set: HashSet<String> = existing_tags.into_iter().collect();
                merged_set.extend(envelope_tags.into_iter());
                // Build whitelist from env (same source used by framework)
                let whitelist: HashSet<String> = std::env::var("SURR_THINK_TAG_WHITELIST")
                    .unwrap_or("plan,debug,dx,photography,idea".to_string())
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
                let merged: Vec<String> = merged_set
                    .into_iter()
                    .filter(|t| whitelist.contains(t))
                    .collect();
                query.push_str(", tags = $merged_tags");
                binds.push((
                    "merged_tags",
                    serde_json::Value::Array(
                        merged.into_iter().map(serde_json::Value::String).collect(),
                    ),
                ));
            }
            query.push_str(" RETURN NONE;");
            let mut db_query = self.db.query(&query);
            for (k, v) in binds {
                db_query = db_query.bind((k, v));
            }
            db_query.await?;
        }

        // Memory injection (simple cosine similarity over recent thoughts)
        let (mem_count, _enriched) = self
            .inject_memories(
                &thought_id,
                &embedding,
                injection_scale_val,
                None,
                Some("think_convo"),
            )
            .await
            .unwrap_or((0, None));

        let original_result = json!({
            "thought_id": thought_id.clone(),
            "embedding_model": self.get_embedding_metadata().1,
            "embedding_dim": self.embedder.dimensions(),
            "memories_injected": mem_count,
            "framework_enhanced": framework_enhanced
        });

        Ok((original_result, resolved_continuity))
    }

    /// Run technical think (no framework, origin='tool', mode-specific defaults)
    #[allow(clippy::too_many_arguments)]
    pub async fn run_technical(
        &self,
        content: &str,
        injection_scale: Option<u8>,
        tags: Option<Vec<String>>,
        significance: Option<f32>,
        _verbose_analysis: Option<bool>,
        mode: &str,
        session_id: Option<String>,
        chain_id: Option<String>,
        previous_thought_id: Option<String>,
        revises_thought: Option<String>,
        branch_from: Option<String>,
        confidence: Option<f32>,
    ) -> Result<(serde_json::Value, ContinuityResult)> {
        let (default_injection_scale, default_significance) = match mode {
            "debug" => (3u8, 0.8_f32),
            "build" => (2u8, 0.6_f32),
            "plan" => (3u8, 0.7_f32),
            "stuck" => (3u8, 0.9_f32),
            _ => (2u8, 0.6_f32), // fallback
        };
        let injection_scale_val = injection_scale.unwrap_or(default_injection_scale) as i64;
        let tags = tags.unwrap_or_default();

        // Use ThoughtBuilder
        let (thought_id, embedding, resolved_continuity) =
            ThoughtBuilder::new(self, content, "tool")
                .scale(Some(injection_scale_val as u8))
                .tags(Some(tags.clone()))
                .significance(significance.or(Some(default_significance)))
                .confidence(confidence)
                .continuity(
                    session_id,
                    chain_id,
                    previous_thought_id,
                    revises_thought,
                    branch_from,
                )
                .execute()
                .await?;

        let tool_name = format!("think_{}", mode);
        let (mem_count, _enriched) = self
            .inject_memories(
                &thought_id,
                &embedding,
                injection_scale_val,
                None,
                Some(&tool_name),
            )
            .await
            .unwrap_or((0, None));

        let original_result = json!({
            "thought_id": thought_id,
            "embedding_model": self.get_embedding_metadata().1,
            "embedding_dim": self.embedder.dimensions(),
            "memories_injected": mem_count
        });

        Ok((original_result, resolved_continuity))
    }

    /// Detect mode from content if no hint
    fn detect_mode(&self, content: &str) -> ThinkMode {
        let content_lower = content.to_lowercase();
        let keywords = [
            (
                "debug",
                vec![
                    "error",
                    "bug",
                    "stack trace",
                    "failed",
                    "exception",
                    "panic",
                ],
            ),
            (
                "build",
                vec![
                    "implement",
                    "create",
                    "add function",
                    "build",
                    "scaffold",
                    "wire",
                ],
            ),
            (
                "plan",
                vec![
                    "architecture",
                    "design",
                    "approach",
                    "how should",
                    "strategy",
                    "trade-off",
                ],
            ),
            (
                "stuck",
                vec!["stuck", "unsure", "confused", "not sure", "blocked"],
            ),
        ];
        let mut best_mode = "question";
        let mut best_score = 0;
        for (mode, kw) in keywords.iter() {
            let score = kw.iter().filter(|k| content_lower.contains(*k)).count();
            if score > best_score {
                best_score = score;
                best_mode = mode;
            }
        }
        if best_score == 0 {
            ThinkMode::Question
        } else {
            match best_mode {
                "debug" => ThinkMode::Debug,
                "build" => ThinkMode::Build,
                "plan" => ThinkMode::Plan,
                "stuck" => ThinkMode::Stuck,
                _ => ThinkMode::Question,
            }
        }
    }

    /// Resolve continuity links with validation and normalization
    #[allow(clippy::single_match, clippy::redundant_pattern_matching)]
    async fn resolve_continuity_links(
        &self,
        new_thought_id: &str,
        previous_thought_id: Option<String>,
        revises_thought: Option<String>,
        branch_from: Option<String>,
    ) -> Result<ContinuityResult> {
        let mut links_resolved = serde_json::Map::new();

        let mut resolved = ContinuityResult {
            session_id: None,
            chain_id: None,
            previous_thought_id: None,
            revises_thought: None,
            branch_from: None,
            confidence: None,
            links_resolved: serde_json::Value::Object(serde_json::Map::new()),
        };

        // Helper function to resolve and validate a thought reference
        let resolve_thought = |id: String| async move {
            // Determine the full ID format for querying
            let full_id = if id.starts_with("thoughts:") {
                id.clone()
            } else {
                format!("thoughts:{}", id)
            };

            // Query the database to check if the record exists
            let check_query = "SELECT id FROM type::thing($id) LIMIT 1";
            let query_result = match self
                .db
                .query(check_query)
                .bind(("id", full_id.clone()))
                .await
            {
                Ok(mut response) => response
                    .take::<Vec<serde_json::Value>>(0)
                    .unwrap_or_default(),
                Err(e) => {
                    tracing::warn!("Failed to query continuity link {}: {}", full_id, e);
                    Vec::new()
                }
            };

            // Process the query result to determine how to handle the ID
            process_continuity_query_result(id, query_result)
        };

        // Resolve each link
        if let Some(id) = previous_thought_id {
            let (resolved_id, resolution_type) = resolve_thought(id).await;
            resolved.previous_thought_id = resolved_id;
            links_resolved.insert(
                "previous_thought_id".to_string(),
                serde_json::Value::String(resolution_type.to_string()),
            );
        }

        if let Some(id) = revises_thought {
            let (resolved_id, resolution_type) = resolve_thought(id).await;
            resolved.revises_thought = resolved_id;
            links_resolved.insert(
                "revises_thought".to_string(),
                serde_json::Value::String(resolution_type.to_string()),
            );
        }

        if let Some(id) = branch_from {
            let (resolved_id, resolution_type) = resolve_thought(id).await;
            resolved.branch_from = resolved_id;
            links_resolved.insert(
                "branch_from".to_string(),
                serde_json::Value::String(resolution_type.to_string()),
            );
        }

        // Prevent self-links
        if resolved
            .previous_thought_id
            .as_ref()
            .map(|id| id.contains(new_thought_id))
            .unwrap_or(false)
        {
            resolved.previous_thought_id = None;
            links_resolved.insert(
                "previous_thought_id".to_string(),
                serde_json::Value::String("dropped_self_link".to_string()),
            );
        }
        if resolved
            .revises_thought
            .as_ref()
            .map(|id| id.contains(new_thought_id))
            .unwrap_or(false)
        {
            resolved.revises_thought = None;
            links_resolved.insert(
                "revises_thought".to_string(),
                serde_json::Value::String("dropped_self_link".to_string()),
            );
        }
        if resolved
            .branch_from
            .as_ref()
            .map(|id| id.contains(new_thought_id))
            .unwrap_or(false)
        {
            resolved.branch_from = None;
            links_resolved.insert(
                "branch_from".to_string(),
                serde_json::Value::String("dropped_self_link".to_string()),
            );
        }

        // Deduplicate (keep first occurrence)
        let mut seen_ids = std::collections::HashSet::new();
        if let Some(ref id) = resolved.previous_thought_id {
            seen_ids.insert(id.clone());
        }
        if let Some(ref id) = resolved.revises_thought {
            if seen_ids.contains(id) {
                resolved.revises_thought = None;
                links_resolved.insert(
                    "revises_thought".to_string(),
                    serde_json::Value::String("dropped_duplicate".to_string()),
                );
            } else {
                seen_ids.insert(id.clone());
            }
        }
        if let Some(ref id) = resolved.branch_from
            && seen_ids.contains(id)
        {
            resolved.branch_from = None;
            links_resolved.insert(
                "branch_from".to_string(),
                serde_json::Value::String("dropped_duplicate".to_string()),
            );
        }

        resolved.links_resolved = serde_json::Value::Object(links_resolved);
        Ok(resolved)
    }

    /// Build text from KG entity or observation for embedding
    fn build_kg_text(name: &str, data: Option<&serde_json::Value>) -> String {
        let mut text = name.to_string();
        if let Some(d) = data.as_ref().and_then(|v| v.as_object()) {
            if let Some(etype) = d.get("entity_type").and_then(|v| v.as_str()) {
                text = format!("{} ({})", name, etype);
            } else if let Some(desc) = d.get("description").and_then(|v| v.as_str()) {
                text.push_str(" - ");
                text.push_str(desc);
            }
        }
        text
    }

    /// Run hypothesis verification against KG
    pub async fn run_hypothesis_verification(
        &self,
        hypothesis: &str,
        top_k: usize,
        min_similarity: f32,
        evidence_limit: usize,
        contradiction_patterns: Option<&[String]>,
    ) -> Result<Option<VerificationResult>> {
        let start = std::time::Instant::now();

        // Instrumentation: log setup
        if std::env::var("RUST_LOG")
            .unwrap_or_default()
            .contains("debug")
        {
            tracing::debug!(
                "hypothesis_verification_setup: ns={}, db={}, embedder_provider={}, embedder_model={}, embedder_dim={}, hypothesis_prefix={}, verify_top_k={}, min_similarity={}, evidence_limit={}",
                self.config.system.database_ns,
                self.config.system.database_db,
                self.get_embedding_metadata().0,
                self.get_embedding_metadata().1,
                self.get_embedding_metadata().2,
                &hypothesis[..hypothesis.len().min(50)],
                top_k,
                min_similarity,
                evidence_limit
            );
        }

        let embedding = self.embedder.embed(hypothesis).await?;
        let q_dim = embedding.len() as i64;

        let patterns = contradiction_patterns.unwrap_or(&[]).to_vec();
        let default_patterns: Vec<String> = CONTRADICTION_PATTERNS
            .iter()
            .map(|s| s.to_string())
            .collect();
        let all_patterns = if patterns.is_empty() {
            &default_patterns
        } else {
            &patterns
        };

        // Query KG entities and observations
        let query_sql = format!(
            "SELECT meta::id(id) as id, name, data, embedding FROM kg_entities \
             WHERE embedding_dim = $dim AND embedding IS NOT NULL LIMIT {}; \
             SELECT meta::id(id) as id, name, data, embedding FROM kg_observations \
             WHERE embedding_dim = $dim AND embedding IS NOT NULL LIMIT {};",
            top_k as i64, top_k as i64
        );

        if std::env::var("RUST_LOG")
            .unwrap_or_default()
            .contains("debug")
        {
            tracing::debug!(
                "hypothesis_verification_query: query_sql={}, dim={}, lim={}",
                query_sql,
                q_dim,
                top_k as i64
            );
        }

        let mut q = self
            .db
            .query(&query_sql)
            .bind(("dim", q_dim))
            .bind(("lim", top_k as i64))
            .await?;
        let mut rows: Vec<serde_json::Value> = q.take(0).unwrap_or_default();
        let mut rows2: Vec<serde_json::Value> = q.take(1).unwrap_or_default();
        rows.append(&mut rows2);

        let total_candidates = rows.len();

        if std::env::var("RUST_LOG")
            .unwrap_or_default()
            .contains("debug")
        {
            tracing::debug!(
                "hypothesis_verification_candidates: total_candidates_after_query={}",
                total_candidates
            );
        }

        let mut supporting = Vec::new();
        let mut contradicting = Vec::new();
        let mut matched_support = 0;
        let mut matched_contradict = 0;

        let mut candidates_with_embedding = 0;
        let mut candidates_after_similarity = 0;

        for r in rows {
            if let (Some(id), Some(name)) = (
                r.get("id").and_then(|v| v.as_str()),
                r.get("name").and_then(|v| v.as_str()),
            ) {
                let data = r.get("data");
                let text = Self::build_kg_text(name, data);

                // Embed the text if needed, but for now assume we have embedding or skip
                // For simplicity, check if embedding exists; if not, compute and persist
                let mut emb_opt = None;
                if let Some(ev) = r.get("embedding").and_then(|v| v.as_array()) {
                    let vecf: Vec<f32> = ev
                        .iter()
                        .filter_map(|x| x.as_f64())
                        .map(|f| f as f32)
                        .collect();
                    if vecf.len() == embedding.len() {
                        emb_opt = Some(vecf);
                        candidates_with_embedding += 1;
                    }
                }
                if emb_opt.is_none() {
                    let new_emb = self.embedder.embed(&text).await?;
                    if new_emb.len() == embedding.len() {
                        emb_opt = Some(new_emb.clone());
                        // Persist (similar to inject_memories)
                    }
                }
                if let Some(emb_e) = emb_opt {
                    let sim = Self::cosine_similarity(&embedding, &emb_e);
                    if sim >= min_similarity {
                        candidates_after_similarity += 1;
                        let item = EvidenceItem {
                            table: if id.starts_with("kg_entities:") {
                                "kg_entities"
                            } else {
                                "kg_observations"
                            }
                            .to_string(),
                            id: id.to_string(),
                            text: text.clone(),
                            similarity: sim,
                            provenance: data.cloned(),
                        };
                        let lower_text = text.to_lowercase();
                        let is_contradiction = all_patterns
                            .iter()
                            .any(|pat| lower_text.contains(&pat.to_lowercase()));
                        if is_contradiction {
                            contradicting.push(item);
                            matched_contradict += 1;
                        } else {
                            supporting.push(item);
                            matched_support += 1;
                        }
                    }
                }
            }
        }

        if std::env::var("RUST_LOG")
            .unwrap_or_default()
            .contains("debug")
        {
            tracing::debug!(
                "hypothesis_verification_counts: candidates_with_embedding={}, candidates_after_similarity={}",
                candidates_with_embedding,
                candidates_after_similarity
            );
        }

        // Sort and limit
        supporting.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        contradicting.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        supporting.truncate(evidence_limit);
        contradicting.truncate(evidence_limit);

        let total = supporting.len() + contradicting.len();
        let confidence_score = if total > 0 {
            supporting.len() as f32 / total as f32
        } else {
            0.5
        };

        let suggested_revision = if confidence_score < 0.4 {
            Some(format!(
                "Consider revising hypothesis based on {} contradicting items",
                contradicting.len()
            ))
        } else {
            None
        };

        let telemetry = json!({
            "embedding_dim": embedding.len(),
            "provider": self.get_embedding_metadata().0,
            "model": self.get_embedding_metadata().1,
            "dim": self.get_embedding_metadata().2,
            "k": top_k,
            "min_similarity": min_similarity,
            "time_ms": start.elapsed().as_millis(),
            "matched_support": matched_support,
            "matched_contradict": matched_contradict,
            "total_candidates": total_candidates,
            "candidates_with_embedding": candidates_with_embedding,
            "candidates_after_similarity": candidates_after_similarity
        });

        let result = VerificationResult {
            hypothesis: hypothesis.to_string(),
            supporting,
            contradicting,
            confidence_score,
            suggested_revision,
            telemetry,
        };

        Ok(Some(result))
    }

    /// Handle legacymind_think tool
    pub async fn handle_legacymind_think(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: LegacymindThinkParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::InvalidParams {
                message: format!("Invalid parameters: {}", e),
            })?;

        if params.content.len() > MAX_CONTENT_SIZE {
            return Err(SurrealMindError::Validation {
                message: format!(
                    "Content exceeds maximum size of {}KB",
                    MAX_CONTENT_SIZE / 1024
                ),
            });
        }

        let content_lower = params.content.to_lowercase();
        let mode = if let Some(hint) = &params.hint {
            match hint.as_str() {
                "debug" => ThinkMode::Debug,
                "build" => ThinkMode::Build,
                "plan" => ThinkMode::Plan,
                "stuck" => ThinkMode::Stuck,
                "question" => ThinkMode::Question,
                "conclude" => ThinkMode::Conclude,
                _ => self.detect_mode(&params.content),
            }
        } else if content_lower.contains("debug time") {
            ThinkMode::Debug
        } else if content_lower.contains("building time") {
            ThinkMode::Build
        } else if content_lower.contains("plan time") || content_lower.contains("planning time") {
            ThinkMode::Plan
        } else if content_lower.contains("i'm stuck") || content_lower.contains("stuck") {
            ThinkMode::Stuck
        } else if content_lower.contains("question time") {
            ThinkMode::Question
        } else if content_lower.contains("wrap up") || content_lower.contains("conclude") {
            ThinkMode::Conclude
        } else {
            self.detect_mode(&params.content)
        };

        let (mode_selected, reason, trigger_matched, heuristics) = match mode {
            ThinkMode::Debug => {
                if params.hint.as_ref().map(|h| h == "debug").unwrap_or(false) {
                    (
                        "debug".to_string(),
                        "hint specified".to_string(),
                        None,
                        None,
                    )
                } else if content_lower.contains("debug time") {
                    (
                        "debug".to_string(),
                        "trigger phrase 'debug time'".to_string(),
                        Some("debug time".to_string()),
                        None,
                    )
                } else if let Some(h) = &params.hint {
                    (
                        "debug".to_string(),
                        format!("heuristic override from hint {}", h),
                        None,
                        None,
                    )
                } else {
                    let matched = [
                        "error",
                        "bug",
                        "stack trace",
                        "failed",
                        "exception",
                        "panic",
                    ];
                    let keywords: Vec<String> = matched
                        .iter()
                        .filter(|k| content_lower.contains(*k))
                        .map(|s| s.to_string())
                        .collect();
                    let score = keywords.len();
                    (
                        "debug".to_string(),
                        "heuristic keyword match".to_string(),
                        None,
                        Some((keywords, score)),
                    )
                }
            }
            ThinkMode::Build => {
                if params.hint.as_ref().map(|h| h == "build").unwrap_or(false) {
                    (
                        "build".to_string(),
                        "hint specified".to_string(),
                        None,
                        None,
                    )
                } else if content_lower.contains("building time") {
                    (
                        "build".to_string(),
                        "trigger phrase 'building time'".to_string(),
                        Some("building time".to_string()),
                        None,
                    )
                } else if let Some(h) = &params.hint {
                    (
                        "build".to_string(),
                        format!("heuristic override from hint {}", h),
                        None,
                        None,
                    )
                } else {
                    let matched = [
                        "implement",
                        "create",
                        "add function",
                        "build",
                        "scaffold",
                        "wire",
                    ];
                    let keywords: Vec<String> = matched
                        .iter()
                        .filter(|k| content_lower.contains(*k))
                        .map(|s| s.to_string())
                        .collect();
                    let score = keywords.len();
                    (
                        "build".to_string(),
                        "heuristic keyword match".to_string(),
                        None,
                        Some((keywords, score)),
                    )
                }
            }
            ThinkMode::Plan => {
                if params.hint.as_ref().map(|h| h == "plan").unwrap_or(false) {
                    ("plan".to_string(), "hint specified".to_string(), None, None)
                } else if content_lower.contains("plan time")
                    || content_lower.contains("planning time")
                {
                    (
                        "plan".to_string(),
                        "trigger phrase".to_string(),
                        Some("plan/planning time".to_string()),
                        None,
                    )
                } else if let Some(h) = &params.hint {
                    (
                        "plan".to_string(),
                        format!("heuristic override from hint {}", h),
                        None,
                        None,
                    )
                } else {
                    let matched = [
                        "architecture",
                        "design",
                        "approach",
                        "how should",
                        "strategy",
                        "trade-off",
                    ];
                    let keywords: Vec<String> = matched
                        .iter()
                        .filter(|k| content_lower.contains(*k))
                        .map(|s| s.to_string())
                        .collect();
                    let score = keywords.len();
                    (
                        "plan".to_string(),
                        "heuristic keyword match".to_string(),
                        None,
                        Some((keywords, score)),
                    )
                }
            }
            ThinkMode::Stuck => {
                if params.hint.as_ref().map(|h| h == "stuck").unwrap_or(false) {
                    (
                        "stuck".to_string(),
                        "hint specified".to_string(),
                        None,
                        None,
                    )
                } else if content_lower.contains("i'm stuck") || content_lower.contains("stuck") {
                    (
                        "stuck".to_string(),
                        "trigger phrase".to_string(),
                        Some("stuck".to_string()),
                        None,
                    )
                } else if let Some(h) = &params.hint {
                    (
                        "stuck".to_string(),
                        format!("heuristic override from hint {}", h),
                        None,
                        None,
                    )
                } else {
                    let matched = ["stuck", "unsure", "confused", "not sure", "blocked"];
                    let keywords: Vec<String> = matched
                        .iter()
                        .filter(|k| content_lower.contains(*k))
                        .map(|s| s.to_string())
                        .collect();
                    let score = keywords.len();
                    (
                        "stuck".to_string(),
                        "heuristic keyword match".to_string(),
                        None,
                        Some((keywords, score)),
                    )
                }
            }
            ThinkMode::Question => {
                if params
                    .hint
                    .as_ref()
                    .map(|h| h == "question")
                    .unwrap_or(false)
                {
                    (
                        "question".to_string(),
                        "hint specified".to_string(),
                        None,
                        None,
                    )
                } else if content_lower.contains("question time") {
                    (
                        "question".to_string(),
                        "trigger phrase 'question time'".to_string(),
                        Some("question time".to_string()),
                        None,
                    )
                } else {
                    (
                        "question".to_string(),
                        "default for general content".to_string(),
                        None,
                        None,
                    )
                }
            }
            ThinkMode::Conclude => {
                if params
                    .hint
                    .as_ref()
                    .map(|h| h == "conclude")
                    .unwrap_or(false)
                {
                    (
                        "conclude".to_string(),
                        "hint specified".to_string(),
                        None,
                        None,
                    )
                } else if content_lower.contains("wrap up") || content_lower.contains("conclude") {
                    (
                        "conclude".to_string(),
                        "trigger phrase".to_string(),
                        Some("wrap up/conclude".to_string()),
                        None,
                    )
                } else if let Some(h) = &params.hint {
                    (
                        "conclude".to_string(),
                        format!("heuristic override from hint {}", h),
                        None,
                        None,
                    )
                } else {
                    (
                        "conclude".to_string(),
                        "trigger match".to_string(),
                        Some("wrap up/conclude".to_string()),
                        None,
                    )
                }
            }
        };

        let injection_scale =
            if matches!(mode, ThinkMode::Conclude) && params.injection_scale.is_none() {
                Some(1)
            } else {
                params.injection_scale
            };

        let is_conclude = matches!(mode, ThinkMode::Conclude);

        let (delegated_result, continuity_result) = match mode {
            ThinkMode::Question | ThinkMode::Conclude => {
                self.run_convo(
                    &params.content,
                    injection_scale,
                    params.tags.clone(),
                    params.significance,
                    params.verbose_analysis,
                    is_conclude,
                    params.session_id.clone(),
                    params.chain_id.clone(),
                    params.previous_thought_id.clone(),
                    params.revises_thought.clone(),
                    params.branch_from.clone(),
                    params.confidence,
                )
                .await?
            }
            _ => {
                let mode_str = match mode {
                    ThinkMode::Debug => "debug",
                    ThinkMode::Build => "build",
                    ThinkMode::Plan => "plan",
                    ThinkMode::Stuck => "stuck",
                    _ => unreachable!(),
                };
                self.run_technical(
                    &params.content,
                    injection_scale,
                    params.tags.clone(),
                    params.significance,
                    params.verbose_analysis,
                    mode_str,
                    params.session_id.clone(),
                    params.chain_id.clone(),
                    params.previous_thought_id.clone(),
                    params.revises_thought.clone(),
                    params.branch_from.clone(),
                    params.confidence,
                )
                .await?
            }
        };

        // Run hypothesis verification if requested
        let verification_result = if let (Some(hypothesis), Some(true)) =
            (&params.hypothesis, params.needs_verification)
        {
            if !hypothesis.is_empty() {
                let top_k = params
                    .verify_top_k
                    .unwrap_or(self.config.runtime.verify_topk);
                let min_similarity = params
                    .min_similarity
                    .unwrap_or(self.config.runtime.verify_min_sim);
                let evidence_limit = params
                    .evidence_limit
                    .unwrap_or(self.config.runtime.verify_evidence_limit);
                let contradiction_patterns = params.contradiction_patterns.as_deref();
                self.run_hypothesis_verification(
                    hypothesis,
                    top_k,
                    min_similarity,
                    evidence_limit,
                    contradiction_patterns,
                )
                .await?
            } else {
                None
            }
        } else {
            None
        };

        // Persist verification result if enabled and available
        if let (Some(verification), true) = (
            &verification_result,
            self.config.runtime.persist_verification,
        ) && let Some(thought_id) = delegated_result.get("thought_id").and_then(|v| v.as_str())
        {
            let thought_id = thought_id.to_string();
            let _ = self
                .db
                .query("UPDATE type::thing('thoughts', $id) SET verification = $verif")
                .bind(("id", thought_id))
                .bind((
                    "verif",
                    serde_json::to_value(verification).unwrap_or(serde_json::Value::Null),
                ))
                .await;
        }

        let telemetry = json!({
            "trigger_matched": trigger_matched,
            "heuristics": if let Some((keywords, score)) = heuristics {
                json!({
                    "keywords": keywords,
                    "score": score
                })
            } else {
                serde_json::Value::Null
            },
            "links_telemetry": continuity_result.links_resolved
        });

        let result = json!({
            "mode_selected": mode_selected,
            "reason": reason,
            "delegated_result": delegated_result,
            "links": {
                "session_id": continuity_result.session_id,
                "chain_id": continuity_result.chain_id,
                "previous_thought_id": continuity_result.previous_thought_id,
                "revises_thought": continuity_result.revises_thought,
                "branch_from": continuity_result.branch_from,
                "confidence": continuity_result.confidence
            },
            "telemetry": telemetry
        });

        // Include verification result in the response if present
        let mut final_result = result;
        if let Some(verification) = verification_result {
            let map = final_result
                .as_object_mut()
                .context("Expected final_result to be a JSON object")?;
            map.insert(
                "verification".to_string(),
                serde_json::to_value(verification)
                    .map_err(|e| anyhow::anyhow!("Serialization error: {}", e))?,
            );
            final_result = serde_json::Value::Object(map.clone());
        }

        Ok(CallToolResult::structured(final_result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_continuity_query_result() {
        // Test the actual behavior: when DB returns empty results, ID is preserved as string

        // Test with non-empty query result (record exists)
        let existing_record = vec![serde_json::json!({
            "id": "thoughts:abc123",
            "content": "Some thought content"
        })];
        let (id, resolution_type) =
            process_continuity_query_result("abc123".to_string(), existing_record);
        assert_eq!(id, Some("thoughts:abc123".to_string()));
        assert_eq!(resolution_type, "record");

        // Test with empty query result (record doesn't exist) - MUST preserve ID
        let empty_result = Vec::new();
        let (id, resolution_type) =
            process_continuity_query_result("missing-id".to_string(), empty_result);
        assert_eq!(id, Some("thoughts:missing-id".to_string()));
        assert_eq!(resolution_type, "string");

        // Test with already-prefixed ID that exists
        let existing_prefixed = vec![serde_json::json!({"id": "thoughts:xyz789"})];
        let (id, resolution_type) =
            process_continuity_query_result("thoughts:xyz789".to_string(), existing_prefixed);
        assert_eq!(id, Some("thoughts:xyz789".to_string()));
        assert_eq!(resolution_type, "record");

        // Test with already-prefixed ID that doesn't exist - MUST preserve
        let empty_prefixed = Vec::new();
        let (id, resolution_type) =
            process_continuity_query_result("thoughts:not-found".to_string(), empty_prefixed);
        assert_eq!(id, Some("thoughts:not-found".to_string()));
        assert_eq!(resolution_type, "string");
    }
}
