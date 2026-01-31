//! call_cc tool handler - synchronous Claude Code CLI execution

use crate::clients::claude::ClaudeClient;
use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::Deserialize;
use serde_json::{Value, json};

const FALLBACK_MODEL: &str = "claude-sonnet-4-5";
const DEFAULT_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_TOOL_TIMEOUT_MS: u64 = 300_000;

/// Get default Claude model from ANTHROPIC_MODEL env var
fn get_default_model() -> String {
    std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| FALLBACK_MODEL.to_string())
}

/// Parameters for the call_cc tool
#[derive(Debug, Deserialize)]
pub struct CallCcParams {
    pub prompt: String,
    #[serde(default)]
    pub task_name: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// Working directory for the Claude Code CLI subprocess (required)
    #[serde(default)]
    pub cwd: Option<String>,
    /// Resume a specific Claude session id
    #[serde(default)]
    pub resume_session_id: Option<String>,
    /// Resume the latest Claude session at cwd
    #[serde(default)]
    pub continue_latest: bool,
    /// Timeout in milliseconds (outer call timeout)
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Per-tool timeout in milliseconds (mapped to MCP_TOOL_TIMEOUT)
    #[serde(default)]
    pub tool_timeout_ms: Option<u64>,
    /// Expose streaming events in response metadata
    #[serde(default)]
    pub expose_stream: bool,
    /// Mode: "execute" (normal) or "observe" (read-only analysis)
    #[serde(default)]
    pub mode: Option<String>,
    /// Max characters for response (default: no limit)
    #[serde(default)]
    pub max_response_chars: Option<i64>,
}

impl SurrealMindServer {
    /// Handle the call_cc tool call - synchronous execution
    pub async fn handle_call_cc(&self, request: CallToolRequestParam) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: CallCcParams = serde_json::from_value(Value::Object(args)).map_err(|e| {
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

        // Apply federation context and observe mode prefix
        let observe_prefix = if params.mode.as_deref() == Some("observe") {
            "You are in OBSERVE mode. Analyze and report only. Do NOT make any file changes.\n\n"
        } else {
            ""
        };
        let prompt = format!(
            "[FEDERATION CONTEXT: You are being invoked as a subagent by surreal-mind MCP. Your output will be returned to the calling agent.]\n\n{}{}",
            observe_prefix, prompt
        );

        let cwd_input = normalize_optional_string(params.cwd).ok_or_else(|| {
            SurrealMindError::InvalidParams {
                message: "cwd is required and cannot be empty".into(),
            }
        })?;

        // Resolve workspace alias or expand path
        let cwd =
            crate::workspace::resolve_workspace(&cwd_input, &self.config.runtime.workspace_map)?;

        let resume_session_id = normalize_optional_string(params.resume_session_id);
        if resume_session_id.is_some() && params.continue_latest {
            return Err(SurrealMindError::InvalidParams {
                message: "resume_session_id and continue_latest cannot both be set".into(),
            });
        }

        let _task_name =
            normalize_optional_string(params.task_name).unwrap_or_else(|| "call_cc".to_string());
        let model = normalize_optional_string(params.model).unwrap_or_else(get_default_model);
        let timeout_ms = params.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
        let tool_timeout_ms = params.tool_timeout_ms.unwrap_or(DEFAULT_TOOL_TIMEOUT_MS);

        // Build and execute ClaudeClient synchronously
        let mut claude = ClaudeClient::new(Some(model.clone()));
        claude = claude.with_cwd(&cwd).with_tool_timeout_ms(tool_timeout_ms);

        if let Some(ref resume) = resume_session_id {
            claude = claude.with_resume_session_id(resume.clone());
        } else if params.continue_latest {
            claude = claude.with_continue_latest(true);
        }
        if params.expose_stream {
            claude = claude.with_expose_stream(true);
        }

        // Execute with timeout
        let timeout = std::time::Duration::from_millis(timeout_ms);
        let execution = tokio::time::timeout(timeout, claude.execute(&prompt))
            .await
            .map_err(|_| SurrealMindError::Mcp {
                message: format!("Claude execution timed out after {}ms", timeout_ms),
            })?
            .map_err(|e| SurrealMindError::Mcp {
                message: format!("Claude execution failed: {}", e),
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
            "response": truncate_response(execution.response, params.max_response_chars),
            "session_id": session_id,
            "metadata": if metadata.is_empty() { Value::Null } else { Value::Object(metadata) }
        })))
    }
}

/// Default max response chars (100KB)
const DEFAULT_MAX_RESPONSE_CHARS: usize = 100_000;

/// Truncate response if over limit
fn truncate_response(response: String, max_chars: Option<i64>) -> String {
    let limit = match max_chars {
        Some(n) if n > 0 => n as usize,
        Some(0) => return response, // 0 = no limit
        _ => DEFAULT_MAX_RESPONSE_CHARS,
    };

    if response.len() <= limit {
        response
    } else {
        let truncated = &response[..limit];
        format!(
            "{}...\n\n[TRUNCATED: Response was {} chars, limit is {}]",
            truncated,
            response.len(),
            limit
        )
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
    fn test_cc_defaults() {
        assert_eq!(FALLBACK_MODEL, "claude-sonnet-4-5");
        assert_eq!(DEFAULT_TIMEOUT_MS, 60_000);
        assert_eq!(DEFAULT_TOOL_TIMEOUT_MS, 300_000);
    }
}
