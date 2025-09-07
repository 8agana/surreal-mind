//! tech_think tool handler for technical reasoning with memory injection

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;

/// Maximum content size in bytes (100KB)
const MAX_CONTENT_SIZE: usize = 100 * 1024;

/// Parameters for the tech_think tool (reuses ConvoThinkParams structure)
#[derive(Debug, serde::Deserialize)]
pub struct TechThinkParams {
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
}

impl SurrealMindServer {
    /// Handle the tech_think tool call
    pub async fn handle_tech_think(&self, request: CallToolRequestParam) -> Result<CallToolResult> {
        self.handle_think_with_profile(request, "plan", 2, 0.6, "think_plan")
            .await
    }

    /// Handle think_plan (Architecture and strategy) — injection_scale: 3, significance: 0.7
    pub async fn handle_think_plan(&self, request: CallToolRequestParam) -> Result<CallToolResult> {
        self.handle_think_with_profile(request, "plan", 3, 0.7, "think_plan")
            .await
    }

    /// Handle think_debug (Root cause analysis) — injection_scale: 4, significance: 0.8
    pub async fn handle_think_debug(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        self.handle_think_with_profile(request, "debug", 4, 0.8, "think_debug")
            .await
    }

    /// Handle think_build (Implementation) — injection_scale: 2, significance: 0.6
    pub async fn handle_think_build(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        self.handle_think_with_profile(request, "build", 2, 0.6, "think_build")
            .await
    }

    /// Handle think_stuck (Lateral thinking) — injection_scale: 3, significance: 0.9
    pub async fn handle_think_stuck(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        self.handle_think_with_profile(request, "stuck", 3, 0.9, "think_stuck")
            .await
    }

    async fn handle_think_with_profile(
        &self,
        request: CallToolRequestParam,
        _forced_submode: &str,
        default_injection_scale: u8,
        default_significance: f32,
        tool_name: &str,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: TechThinkParams = serde_json::from_value(serde_json::Value::Object(args))
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

        // Extract mode from tool_name, e.g. "think_debug" -> "debug"
        let mode = tool_name.trim_start_matches("think_");
        let result = self
            .run_technical(
                &params.content,
                params.injection_scale,
                params.tags,
                params.significance,
                params.verbose_analysis,
                mode,
            )
            .await?;

        Ok(CallToolResult::structured(result))
    }
}
