//! thinking module: common run_* helpers for think tools and new legacymind_think
//!
//! Submodules:
//! - `types`: Shared types and constants for thinking operations
//! - `mode_detection`: Heuristics for detecting thinking mode from content
//! - `runners`: Execution paths for conversational and technical thinking
//! - `continuity`: Continuity link resolution and validation
//! - `mode_router`: Mode selection with routing metadata
//! - `verification`: Hypothesis verification against knowledge graph

pub mod continuity;
pub mod mode_detection;
pub mod mode_router;
pub mod runners;
pub mod types;
pub mod verification;

// Re-export types for external use
pub use types::{
    ContinuityResult, EvidenceItem, LegacymindThinkParams, ThinkMode, VerificationResult,
    CONTRADICTION_PATTERNS, MAX_CONTENT_SIZE, process_continuity_query_result,
};

// Re-export mode detection for internal use
use mode_detection::detect_mode;

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use anyhow::Context;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;

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
                _ => detect_mode(&params.content),
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
            detect_mode(&params.content)
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
