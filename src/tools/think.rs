//! think tool handlers for storing thoughts with memory injection

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;

/// Parameters for all `think_*` tools.
#[derive(Debug, serde::Deserialize)]
    pub struct ThinkParams {
        pub content: String,
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
        pub submode: Option<String>,
    }

impl SurrealMindServer {
    /// Generic handler for all `think_*` tools.
    async fn handle_think(
        &self,
        request: CallToolRequestParam,
        origin: &str,
        default_injection_scale: u8,
        default_significance: f32,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: ThinkParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::Serialization {
                message: format!("Invalid parameters: {}", e),
            })?;

        tracing::info!(
            "{} called (content_len={})",
            request.name,
            params.content.len()
        );
        let dbg_preview: String = params.content.chars().take(200).collect();
        tracing::debug!("{} content (first 200 chars): {}", request.name, dbg_preview);

        let embedding = self.embedder.embed(&params.content).await.map_err(|e| {
            SurrealMindError::Embedding {
                message: e.to_string(),
            }
        })?;

        if embedding.is_empty() {
            tracing::error!("Generated embedding is empty for content");
            return Err(SurrealMindError::Embedding {
                message: "Generated embedding is empty".into(),
            });
        }
        tracing::debug!("Generated embedding with {} dimensions", embedding.len());

        let injection_scale = params
            .injection_scale
            .unwrap_or(default_injection_scale) as i64;
        let significance = params.significance.unwrap_or(default_significance) as f64;

        let thought_id = uuid::Uuid::new_v4().to_string();
        let (provider, model, dim) = self.get_embedding_metadata();

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
                    origin: $origin,
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
            .bind(("origin", origin.to_string()))
            .bind(("tags", params.tags.unwrap_or_default()))
            .bind(("provider", provider))
            .bind(("model", model))
            .bind(("dim", dim))
            .await?;

        // Memory injection with proper error handling
        let (mem_count, _enriched) = match self
            .inject_memories(&thought_id, &embedding, injection_scale, Some(&request.name))
            .await
        {
            Ok(result) => result,
            Err(e) => {
                tracing::warn!(
                    "Memory injection failed for thought {}: {}. Proceeding without injection.",
                    thought_id,
                    e
                );
                (0, None)
            }
        };

        let result = json!({
            "thought_id": thought_id,
            "embedding_model": self.get_embedding_metadata().1,
            "embedding_dim": self.embedder.dimensions(),
            "memories_injected": mem_count
        });

        Ok(CallToolResult::structured(result))
    }

    // Public handlers for each think tool, calling the generic handler.

    pub async fn handle_convo_think(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        self.handle_think(request, "human", 1, 0.5).await
    }

    pub async fn handle_think_plan(&self, request: CallToolRequestParam) -> Result<CallToolResult> {
        self.handle_think(request, "tool", 3, 0.7).await
    }

    pub async fn handle_think_debug(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        self.handle_think(request, "tool", 4, 0.8).await
    }

    pub async fn handle_think_build(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        self.handle_think(request, "tool", 2, 0.6).await
    }

    pub async fn handle_think_stuck(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        self.handle_think(request, "tool", 3, 0.9).await
    }
}
