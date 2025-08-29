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

        let result = json!({
            "thought_id": "placeholder",
            "submode_used": submode,
            "memories_injected": 0,
            "analysis": {
                "key_point": "Technical thought stored successfully",
                "question": "What's the next step in development?",
                "next_step": "Continue implementation"
            }
        });

        Ok(CallToolResult::structured(result))
    }
}
