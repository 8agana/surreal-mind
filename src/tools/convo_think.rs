//! convo_think tool handler for storing thoughts with memory injection

use crate::error::{Result, SurrealMindError};
use crate::frameworks::{ConvoOpts, run_convo};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// Maximum content size in bytes (100KB)
const MAX_CONTENT_SIZE: usize = 100 * 1024;

/// Parameters for the convo_think tool
#[derive(Debug, serde::Deserialize)]
pub struct ConvoThinkParams {
    pub content: String,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_u8_forgiving"
    )]
    pub injection_scale: Option<u8>,
    // submode is deprecated for think_convo; accepted but ignored
    #[serde(default)]
    pub submode: Option<String>,
    #[serde(default, deserialize_with = "crate::deserializers::de_option_tags")]
    pub tags: Option<Vec<String>>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_f32_forgiving"
    )]
    pub significance: Option<f32>,
    #[serde(default)]
    pub verbose_analysis: Option<bool>,
}

impl SurrealMindServer {
    /// Handle the convo_think tool call
    pub async fn handle_convo_think(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: ConvoThinkParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::Serialization {
                message: format!("Invalid parameters: {}", e),
            })?;

        // Validate content size
        if params.content.len() > MAX_CONTENT_SIZE {
            return Err(SurrealMindError::Validation {
                message: format!("Content exceeds maximum size of {}KB", MAX_CONTENT_SIZE / 1024),
            });
        }

        // Redact content at info level to avoid logging full user text
        tracing::info!("convo_think called (content_len={})", params.content.len());
        if std::env::var("SURR_THINK_DEEP_LOG").unwrap_or("0".to_string()) == "1" {
            let dbg_preview: String = params.content.chars().take(200).collect();
            tracing::debug!("convo_think content (first 200 chars): {}", dbg_preview);
        }

        // Compute embedding
        let embedding = self.embedder.embed(&params.content).await.map_err(|e| {
            SurrealMindError::Embedding {
                message: e.to_string(),
            }
        })?;

        // Validate embedding
        if embedding.is_empty() {
            tracing::error!("Generated embedding is empty for content");
            return Err(SurrealMindError::Embedding {
                message: "Generated embedding is empty".into(),
            });
        }
        tracing::debug!("Generated embedding with {} dimensions", embedding.len());

        // Defaults (submode ignored for convo_think)
        let injection_scale = params.injection_scale.unwrap_or(1) as i64;
        let significance = params.significance.unwrap_or(0.5_f32) as f64;

        // Generate a UUID for the thought
        let thought_id = uuid::Uuid::new_v4().to_string();

        // Get embedding metadata for tracking
        let (provider, model, dim) = self.get_embedding_metadata();

        // Insert into SurrealDB using the generated ID
        self.db
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
                    origin: 'human',
                    tags: $tags,
                    is_private: false,
                    embedding_provider: $provider,
                    embedding_model: $model,
                    embedding_dim: $dim,
                    embedded_at: time::now()
                } RETURN NONE;",
            )
            .bind(("id", thought_id.clone()))
            .bind(("content", params.content.clone()))
            .bind(("embedding", embedding.clone()))
            .bind(("injection_scale", injection_scale))
            .bind(("significance", significance))
            .bind(("tags", params.tags.clone().unwrap_or_default()))
            // submode intentionally not stored for think_convo
            .bind(("provider", provider))
            .bind(("model", model))
            .bind(("dim", dim))
            .await?;

        // Framework enhancement (optional, local)
        let verbose_analysis = params.verbose_analysis.unwrap_or(false);
        let enhance_enabled = std::env::var("SURR_THINK_ENHANCE").unwrap_or("1".to_string()) == "1";
        let mut framework_enhanced = false;
        let mut framework_analysis: Option<serde_json::Value> = None;
        let finalize_ms: u64;
        if enhance_enabled || verbose_analysis {
            tracing::debug!("Running framework enhancement for thought {}", thought_id);
            let start = Instant::now();
            let opts = ConvoOpts {
                strict_json: std::env::var("SURR_THINK_STRICT_JSON").unwrap_or("1".to_string())
                    == "1",
                tag_whitelist: std::env::var("SURR_THINK_TAG_WHITELIST")
                    .unwrap_or("plan,debug,dx,photography,idea".to_string())
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect(),
                timeout_ms: std::env::var("SURR_THINK_ENHANCE_TIMEOUT_MS")
                    .unwrap_or("600".to_string())
                    .parse()
                    .unwrap_or(600),
            };
            match tokio::time::timeout(
                Duration::from_millis(opts.timeout_ms),
                run_convo(&params.content, &opts),
            )
            .await
            {
                Ok(Ok(envelope)) => {
                    framework_enhanced = true;
                    framework_analysis = Some(serde_json::to_value(&envelope).unwrap_or(json!({})));
                    finalize_ms = start.elapsed().as_millis() as u64;
                    tracing::info!("think.convo.enhance.calls");
                    tracing::info!("think.convo.methodology.{}", envelope.methodology);
                }
                Ok(Err(e)) => {
                    tracing::warn!(
                        "Framework enhancement failed for thought {}: {}",
                        thought_id,
                        e
                    );
                    tracing::info!("think.convo.enhance.drop_json");
                    finalize_ms = start.elapsed().as_millis() as u64;
                }
                Err(_) => {
                    tracing::warn!("Framework enhancement timed out for thought {}", thought_id);
                    tracing::info!("think.convo.enhance.timeout");
                    finalize_ms = opts.timeout_ms;
                }
            }
            tracing::info!("think.convo.finalize.ms {}", finalize_ms);
        }

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
            if framework_enhanced {
                if let Some(env) = framework_analysis.as_ref().and_then(|a| a.as_object()) {
                    if let Some(data) = env.get("data").and_then(|d| d.as_object()) {
                        if let Some(tags) = data.get("tags").and_then(|t| t.as_array()) {
                            // Merge tags, then filter by whitelist to ensure only allowed tags persist
                            let existing_tags: Vec<String> = params.tags.unwrap_or_default();
                            let envelope_tags: Vec<String> = tags
                                .iter()
                                .filter_map(|t| t.as_str())
                                .map(|s| s.to_string())
                                .collect();
                            let mut merged_set: HashSet<String> =
                                existing_tags.into_iter().collect();
                            merged_set.extend(envelope_tags.into_iter());
                            // Build whitelist from env (same source used by framework)
                            let whitelist: HashSet<String> =
                                std::env::var("SURR_THINK_TAG_WHITELIST")
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
                    }
                }
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
                injection_scale,
                None,
                Some("think_convo"),
            )
            .await
            .unwrap_or((0, None));

        let result = json!({
            "thought_id": thought_id,
            "embedding_model": self.get_embedding_metadata().1,
            "embedding_dim": self.embedder.dimensions(),
            "memories_injected": mem_count,
            "framework_enhanced": framework_enhanced
        });

        Ok(CallToolResult::structured(result))
    }
}
