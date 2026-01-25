//! call_vibe tool handler - synchronous Vibe CLI execution
//! Vibe is a one-shot executor with optional session continuation

use crate::clients::vibe::VibeClient;
use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::Deserialize;
use serde_json::{Value, json};

const DEFAULT_TIMEOUT_MS: u64 = 60_000;

/// Parameters for the call_vibe tool
#[derive(Debug, Deserialize)]
pub struct CallVibeParams {
    pub prompt: String,
    /// Working directory for the Vibe CLI subprocess (required)
    pub cwd: String,
    /// Agent profile name from ~/.vibe/agents/*.toml
    #[serde(default)]
    pub agent: Option<String>,
    /// Mode: "execute" (normal) or "observe" (read-only analysis)
    #[serde(default)]
    pub mode: Option<String>,
    /// Continue from last Vibe session
    #[serde(default)]
    pub continue_latest: Option<bool>,
    /// Timeout in milliseconds (default: 60000)
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Max characters for response (default: 100000)
    #[serde(default)]
    pub max_response_chars: Option<i64>,
}

impl SurrealMindServer {
    /// Handle the call_vibe tool call - synchronous execution
    pub async fn handle_call_vibe(&self, request: CallToolRequestParam) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: CallVibeParams = serde_json::from_value(Value::Object(args)).map_err(|e| {
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

        let cwd = params.cwd.trim().to_string();
        if cwd.is_empty() {
            return Err(SurrealMindError::InvalidParams {
                message: "cwd is required and cannot be empty".into(),
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

        let agent = normalize_optional_string(params.agent);
        let timeout_ms = params.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
        let continue_latest = params.continue_latest.unwrap_or(false);

        // Build and execute VibeClient synchronously
        let vibe = VibeClient::new(agent)
            .with_cwd(&cwd)
            .with_timeout_ms(timeout_ms)
            .with_continue_latest(continue_latest);

        // Execute (client has its own timeout)
        let execution = vibe
            .execute(&prompt)
            .await
            .map_err(|e| SurrealMindError::Mcp {
                message: format!("Vibe execution failed: {}", e),
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
    fn test_vibe_defaults() {
        assert_eq!(DEFAULT_TIMEOUT_MS, 60_000);
        assert_eq!(DEFAULT_MAX_RESPONSE_CHARS, 100_000);
    }
}
