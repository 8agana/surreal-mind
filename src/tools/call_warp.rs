//! call_warp tool handler - synchronous Warp CLI execution
//! Warp is a one-shot executor - no session persistence or resume support

use crate::clients::warp::WarpClient;
use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::Deserialize;
use serde_json::{Value, json};

const DEFAULT_TIMEOUT_MS: u64 = 60_000;

/// Parameters for the call_warp tool
#[derive(Debug, Deserialize)]
pub struct CallWarpParams {
    pub prompt: String,
    #[serde(default)]
    pub task_name: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// Working directory for the Warp CLI subprocess (required)
    pub cwd: String,
    /// Timeout in milliseconds (outer call timeout)
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Mode: "execute" (normal) or "observe" (read-only analysis)
    #[serde(default)]
    pub mode: Option<String>,
    /// Max characters for response (default: 100000)
    #[serde(default)]
    pub max_response_chars: Option<i64>,
}

impl SurrealMindServer {
    /// Handle the call_warp tool call - synchronous execution
    pub async fn handle_call_warp(&self, request: CallToolRequestParam) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: CallWarpParams = serde_json::from_value(Value::Object(args)).map_err(|e| {
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

        let cwd = params.cwd.trim().to_string();
        if cwd.is_empty() {
            return Err(SurrealMindError::InvalidParams {
                message: "cwd is required and cannot be empty".into(),
            });
        }

        let _task_name =
            normalize_optional_string(params.task_name).unwrap_or_else(|| "call_warp".to_string());
        let model = normalize_optional_string(params.model); // None = omit flag
        let timeout_ms = params.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);

        // Build and execute WarpClient synchronously
        let mut warp = WarpClient::new(model);
        warp = warp.with_cwd(&cwd).with_timeout_ms(timeout_ms);

        // Execute with timeout
        let timeout = std::time::Duration::from_millis(timeout_ms);
        let execution = tokio::time::timeout(timeout, warp.execute(&prompt))
            .await
            .map_err(|_| SurrealMindError::Mcp {
                message: format!("Warp execution timed out after {}ms", timeout_ms),
            })?
            .map_err(|e| SurrealMindError::Mcp {
                message: format!("Warp execution failed: {}", e),
            })?;

        // Build response
        let mut metadata = serde_json::Map::new();
        if !execution.stderr.trim().is_empty() {
            metadata.insert(
                "stderr".to_string(),
                Value::String(execution.stderr.clone()),
            );
        }

        Ok(CallToolResult::structured(json!({
            "status": "completed",
            "response": truncate_response(execution.response, params.max_response_chars),
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
    fn test_warp_defaults() {
        assert_eq!(DEFAULT_TIMEOUT_MS, 60_000);
        assert_eq!(DEFAULT_MAX_RESPONSE_CHARS, 100_000);
    }
}
