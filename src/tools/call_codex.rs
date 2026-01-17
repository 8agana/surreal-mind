//! call_codex tool handler - synchronous Codex CLI execution

use crate::clients::codex::CodexClient;
use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::Deserialize;
use serde_json::{Value, json};

const DEFAULT_MODEL: &str = "gpt-5.2-codex";
const DEFAULT_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_TOOL_TIMEOUT_MS: u64 = 300_000;

/// Parameters for the call_codex tool
#[derive(Debug, Deserialize)]
pub struct CallCodexParams {
    pub prompt: String,
    #[serde(default)]
    pub task_name: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// Working directory for the Codex CLI subprocess
    #[serde(default)]
    pub cwd: Option<String>,
    /// Resume a specific Codex session id
    #[serde(default)]
    pub resume_session_id: Option<String>,
    /// Resume the latest Codex session
    #[serde(default)]
    pub continue_latest: bool,
    /// Timeout in milliseconds (outer call timeout)
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Per-tool timeout in milliseconds (mapped to TOOL_TIMEOUT_SEC)
    #[serde(default)]
    pub tool_timeout_ms: Option<u64>,
    /// Expose streaming events in response metadata
    #[serde(default)]
    pub expose_stream: bool,
    /// If true, job is enqueued and not awaited (async-only for now)
    #[serde(default)]
    pub fire_and_forget: bool,
}

impl SurrealMindServer {
    /// Handle the call_codex tool call - synchronous execution
    pub async fn handle_call_codex(&self, request: CallToolRequestParam) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: CallCodexParams = serde_json::from_value(Value::Object(args)).map_err(|e| {
            SurrealMindError::InvalidParams {
                message: format!("Invalid parameters: {}", e),
            }
        })?;

        let prompt = params.prompt.trim().to_string();
        if prompt.is_empty() {
            return Err(SurrealMindError::InvalidParams {
                message: "prompt cannot be empty".into(),
            });
        }

        let cwd = normalize_optional_string(params.cwd);
        let cwd = cwd.ok_or_else(|| SurrealMindError::InvalidParams {
            message: "cwd is required and cannot be empty".into(),
        })?;

        let resume_session_id = normalize_optional_string(params.resume_session_id);
        if resume_session_id.is_some() && params.continue_latest {
            return Err(SurrealMindError::InvalidParams {
                message: "resume_session_id and continue_latest cannot both be set".into(),
            });
        }

        let _task_name =
            normalize_optional_string(params.task_name).unwrap_or_else(|| "call_codex".to_string());
        let model =
            normalize_optional_string(params.model).unwrap_or_else(|| DEFAULT_MODEL.to_string());
        let timeout_ms = params.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
        let tool_timeout_ms = params.tool_timeout_ms.unwrap_or(DEFAULT_TOOL_TIMEOUT_MS);

        // Build and execute CodexClient synchronously
        let mut codex = CodexClient::new(Some(model.clone()));
        codex = codex.with_cwd(&cwd).with_tool_timeout_ms(tool_timeout_ms);

        if let Some(ref resume) = resume_session_id {
            codex = codex.with_resume_session_id(resume.clone());
        } else if params.continue_latest {
            codex = codex.with_continue_latest(true);
        }
        if params.expose_stream {
            codex = codex.with_expose_stream(true);
        }

        // Execute with timeout
        let timeout = std::time::Duration::from_millis(timeout_ms);
        let execution = tokio::time::timeout(timeout, codex.execute(&prompt))
            .await
            .map_err(|_| SurrealMindError::Mcp {
                message: format!("Codex execution timed out after {}ms", timeout_ms),
            })?
            .map_err(|e| SurrealMindError::Mcp {
                message: format!("Codex execution failed: {}", e),
            })?;

        // Build response
        let session_id = execution.session_id.clone().unwrap_or_default();

        // Build metadata for streaming events if requested
        let mut metadata = serde_json::Map::new();
        if params.expose_stream && !execution.events.is_empty() {
            metadata.insert(
                "stream_events".to_string(),
                Value::Array(execution.events.clone()),
            );
        }
        if !execution.stderr.trim().is_empty() {
            metadata.insert(
                "stderr".to_string(),
                Value::String(execution.stderr.clone()),
            );
        }

        Ok(CallToolResult::structured(json!({
            "status": "completed",
            "response": execution.response,
            "session_id": session_id,
            "metadata": if metadata.is_empty() { Value::Null } else { Value::Object(metadata) }
        })))
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codex_defaults() {
        assert_eq!(DEFAULT_MODEL, "gpt-5.2-codex");
        assert_eq!(DEFAULT_TIMEOUT_MS, 60_000);
        assert_eq!(DEFAULT_TOOL_TIMEOUT_MS, 300_000);
    }
}
