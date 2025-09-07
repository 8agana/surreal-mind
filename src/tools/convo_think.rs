//! convo_think tool handler for storing thoughts with memory injection

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};

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
                message: format!(
                    "Content exceeds maximum size of {}KB",
                    MAX_CONTENT_SIZE / 1024
                ),
            });
        }

        // Redact content at info level to avoid logging full user text
        tracing::info!("convo_think called (content_len={})", params.content.len());
        if std::env::var("SURR_THINK_DEEP_LOG").unwrap_or("0".to_string()) == "1" {
            let dbg_preview: String = params.content.chars().take(200).collect();
            tracing::debug!("convo_think content (first 200 chars): {}", dbg_preview);
        }

        let result = self
            .run_convo(
                &params.content,
                params.injection_scale,
                params.tags.clone(),
                params.significance,
                params.verbose_analysis,
                false,
            )
            .await?;

        Ok(CallToolResult::structured(result))
    }
}
