//! convo_think tool handler for storing thoughts with memory injection

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;

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

        // Redact content at info level to avoid logging full user text
        tracing::info!("convo_think called (content_len={})", params.content.len());
        let dbg_preview: String = params.content.chars().take(200).collect();
        tracing::debug!("convo_think content (first 200 chars): {}", dbg_preview);

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
            // submode intentionally not stored for think_convo
            .bind(("provider", provider))
            .bind(("model", model))
            .bind(("dim", dim))
            .await?;

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
            "memories_injected": mem_count
        });

        Ok(CallToolResult::structured(result))
    }
}
