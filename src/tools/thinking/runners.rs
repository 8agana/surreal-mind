//! Runner implementations for thinking operations
//!
//! This module contains the primary execution paths for thinking:
//! - `run_convo`: Conversational thinking with framework enhancement (origin='human')
//! - `run_technical`: Technical thinking with mode-specific defaults (origin='tool')
//!
//! Both runners use `ThoughtBuilder` for consistent thought creation and
//! `CognitiveEngine` for optional framework analysis.

use crate::cognitive::{
    CognitiveEngine,
    profile::{Submode, profile_for},
};
use crate::error::Result;
use crate::server::SurrealMindServer;
use super::types::ContinuityResult;
use super::ThoughtBuilder;
use serde_json::json;

impl SurrealMindServer {
    /// Run conversational think (with framework enhancement, origin='human')
    ///
    /// Creates a thought with origin='human' and applies cognitive framework
    /// analysis when enabled. Memory injection is performed after creation.
    ///
    /// # Arguments
    /// * `content` - The thought content
    /// * `injection_scale` - Memory injection scale (0-3)
    /// * `tags` - Optional tags for the thought
    /// * `significance` - Optional significance score
    /// * `verbose_analysis` - Whether to include detailed framework analysis
    /// * `is_conclude` - Whether this is a conclusion thought
    /// * `session_id`, `chain_id`, etc. - Continuity parameters
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

        // Framework enhancement
        let enhance_enabled = std::env::var("SURR_THINK_ENHANCE").unwrap_or("1".to_string()) == "1";

        let verbose_analysis = verbose_analysis.unwrap_or(false);
        let mut framework_enhanced = false;
        let mut framework_analysis: Option<serde_json::Value> = None;

        if enhance_enabled || verbose_analysis {
            let submode = if is_conclude {
                Submode::Sarcastic
            } else {
                Submode::Philosophical
            };
            let profile = profile_for(submode);
            let engine = CognitiveEngine::new();

            let analysis = engine.blend(content, &profile.weights);

            framework_enhanced = true;
            match serde_json::to_value(&analysis) {
                Ok(val) => framework_analysis = Some(val),
                Err(e) => tracing::error!("Failed to serialize framework analysis: {}", e),
            }
        }

        if framework_enhanced || framework_analysis.is_some() {
            let query = "UPDATE type::thing('thoughts', $id) SET framework_enhanced = $enhanced, framework_analysis = $analysis RETURN NONE;";
            self.db
                .query(query)
                .bind(("id", thought_id.clone()))
                .bind(("enhanced", framework_enhanced))
                .bind((
                    "analysis",
                    framework_analysis
                        .clone()
                        .unwrap_or(serde_json::Value::Null),
                ))
                .await?;
        }

        // Memory injection (simple cosine similarity over recent thoughts)
        let (mem_count, enriched) = self
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
            "enriched_content": enriched,
            "framework_enhanced": framework_enhanced
        });

        Ok((original_result, resolved_continuity))
    }

    /// Run technical think (no framework by default, origin='tool', mode-specific defaults)
    ///
    /// Creates a thought with origin='tool' and mode-specific defaults for
    /// injection scale and significance. Framework analysis is applied when enabled.
    ///
    /// # Mode-specific defaults
    /// - debug: injection_scale=3, significance=0.8
    /// - build: injection_scale=2, significance=0.6
    /// - plan: injection_scale=3, significance=0.7
    /// - stuck: injection_scale=3, significance=0.9
    #[allow(clippy::too_many_arguments)]
    pub async fn run_technical(
        &self,
        content: &str,
        injection_scale: Option<u8>,
        tags: Option<Vec<String>>,
        significance: Option<f32>,
        verbose_analysis: Option<bool>,
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

        // Framework enhancement
        let enhance_enabled = std::env::var("SURR_THINK_ENHANCE").unwrap_or("1".to_string()) == "1";
        let verbose = verbose_analysis.unwrap_or(false);

        let mut framework_enhanced = false;
        let mut framework_analysis: Option<serde_json::Value> = None;

        if enhance_enabled || verbose {
            let submode = Submode::from_str(mode);
            let profile = profile_for(submode);
            let engine = CognitiveEngine::new();
            let analysis = engine.blend(content, &profile.weights);

            framework_enhanced = true;
            match serde_json::to_value(&analysis) {
                Ok(val) => framework_analysis = Some(val),
                Err(e) => tracing::error!("Failed to serialize framework analysis: {}", e),
            }
        }

        if framework_enhanced || framework_analysis.is_some() {
            let query = "UPDATE type::thing('thoughts', $id) SET framework_enhanced = $enhanced, framework_analysis = $analysis RETURN NONE;";
            self.db
                .query(query)
                .bind(("id", thought_id.clone()))
                .bind(("enhanced", framework_enhanced))
                .bind((
                    "analysis",
                    framework_analysis
                        .clone()
                        .unwrap_or(serde_json::Value::Null),
                ))
                .await?;
        }

        let tool_name = format!("think_{}", mode);
        let (mem_count, enriched) = self
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
            "memories_injected": mem_count,
            "enriched_content": enriched,
            "framework_enhanced": framework_enhanced
        });

        Ok((original_result, resolved_continuity))
    }
}
