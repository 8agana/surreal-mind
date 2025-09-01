//! tech_think tool handler for technical reasoning with memory injection

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;

/// Parameters for the tech_think tool (reuses ConvoThinkParams structure)
#[derive(Debug, serde::Deserialize)]
pub struct TechThinkParams {
    pub content: String,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_u8_forgiving"
    )]
    pub injection_scale: Option<u8>,
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
    /// Handle the tech_think tool call
    pub async fn handle_tech_think(&self, request: CallToolRequestParam) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: TechThinkParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::Serialization {
                message: format!("Invalid parameters: {}", e),
            })?;

        // Default submode for tech_think is "plan"
        let submode = params.submode.unwrap_or_else(|| "plan".to_string());

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

        let injection_scale = params.injection_scale.unwrap_or(2) as i64; // slightly higher default
        let significance = params.significance.unwrap_or(0.6_f32) as f64;

        // Generate a UUID for the thought
        let thought_id = uuid::Uuid::new_v4().to_string();

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
                    submode: $submode,
                    framework_enhanced: NONE,
                    framework_analysis: NONE,
                    is_inner_voice: false,
                    inner_visibility: NONE
                } RETURN NONE;",
            )
            .bind(("id", thought_id.clone()))
            .bind(("content", params.content.clone()))
            .bind(("embedding", embedding.clone()))
            .bind(("injection_scale", injection_scale))
            .bind(("significance", significance))
            .bind(("submode", submode.clone()))
            .await?;

        // Memory injection
        let (mem_count, _enriched) = self
            .inject_memories(&thought_id, &embedding, injection_scale, Some(&submode))
            .await
            .unwrap_or((0, None));

        let result = json!({
            "thought_id": thought_id,
            "submode_used": submode,
            "memories_injected": mem_count
        });

        Ok(CallToolResult::structured(result))
    }
}
