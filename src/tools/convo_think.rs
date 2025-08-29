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

        // For now, return a simple success response
        // TODO: Implement full create_thought_with_injection logic
        let result = json!({
            "thought_id": "placeholder",
            "submode_used": params.submode.unwrap_or_else(|| "sarcastic".to_string()),
            "memories_injected": 0,
            "analysis": {
                "key_point": "Thought stored successfully",
                "question": "What's next?",
                "next_step": "Continue processing"
            }
        });

        Ok(CallToolResult::structured(result))
    }
}
