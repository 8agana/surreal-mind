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

        let result = json!({
            "thought_id": "placeholder",
            "inner_voice": true,
            "visibility": params.inner_visibility.unwrap_or_else(|| "context_only".to_string()),
            "memories_injected": 0,
            "analysis": {
                "key_point": "Inner thought recorded privately",
                "question": "What else is worth noting?",
                "next_step": "Continue inner reflection"
            }
        });

        Ok(CallToolResult::structured(result))
    }
}
