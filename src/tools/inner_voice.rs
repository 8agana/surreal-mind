//! inner_voice tool handler for storing private inner thoughts

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;

/// Parameters for the inner_voice tool
#[derive(Debug, serde::Deserialize)]
pub struct InnerVoiceParams {
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
    pub inner_visibility: Option<String>,
}

impl SurrealMindServer {
    /// Handle the inner_voice tool call
    pub async fn handle_inner_voice(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: InnerVoiceParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::Serialization {
                message: format!("Invalid parameters: {}", e),
            })?;

        // Redact content at info level to avoid logging private thoughts
        tracing::info!("inner_voice called (content_len={})", params.content.len());
        let dbg_preview: String = params.content.chars().take(50).collect(); // Shorter preview for privacy
        tracing::debug!("inner_voice content (first 50 chars): {}", dbg_preview);

        // Compute embedding
        let embedding = self
            .embedder
            .embed(&params.content)
            .await
            .map_err(|e| SurrealMindError::Embedding { message: e.to_string() })?;

        let injection_scale = params.injection_scale.unwrap_or(0) as i64; // default minimal
        let significance = params.significance.unwrap_or(0.4_f32) as f64;
        let inner_visibility = params
            .inner_visibility
            .unwrap_or_else(|| "context_only".to_string());

        // Insert into SurrealDB
        let created_raw: Vec<surrealdb::sql::Value> = self
            .db
            .query("CREATE thoughts SET content = $content, created_at = time::now(), embedding = $embedding, injected_memories = [], enriched_content = NONE, injection_scale = $injection_scale, significance = $significance, access_count = 0, last_accessed = NONE, submode = NONE, framework_enhanced = NONE, framework_analysis = NONE, is_inner_voice = true, inner_visibility = $inner_visibility RETURN AFTER;")
            .bind(("content", params.content.clone()))
            .bind(("embedding", embedding.clone()))
            .bind(("injection_scale", injection_scale))
            .bind(("significance", significance))
            .bind(("inner_visibility", inner_visibility.clone()))
            .await?
            .take(0)?;

        let created: Vec<serde_json::Value> = created_raw
            .into_iter()
            .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
            .collect();

        let thought_id = created
            .get(0)
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let result = json!({
            "thought_id": thought_id,
            "inner_voice": true,
            "visibility": inner_visibility,
            "memories_injected": 0
        });

        Ok(CallToolResult::structured(result))
    }
}
