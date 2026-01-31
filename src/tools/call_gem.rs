//! delegate_gemini tool handler to call Gemini CLI - now synchronous

use crate::clients::traits::CognitiveAgent;
use crate::clients::{AgentError, GeminiClient};
use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::Deserialize;
use serde_json::{Value, json};

const DEFAULT_TIMEOUT_MS: u64 = 60_000;

/// Parameters for the delegate_gemini tool
#[derive(Debug, Deserialize)]
pub struct DelegateGeminiParams {
    pub prompt: String,
    #[serde(default)]
    pub task_name: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// Working directory for the Gemini CLI subprocess
    #[serde(default)]
    pub cwd: Option<String>,
    /// Resume a specific session by ID
    #[serde(default)]
    pub resume_session_id: Option<String>,
    /// Resume the most recent session (--resume without ID)
    #[serde(default)]
    pub continue_latest: bool,
    /// Timeout in milliseconds (overrides GEMINI_TIMEOUT_MS env var)
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Per-tool timeout in milliseconds (overrides GEMINI_TOOL_TIMEOUT_MS env var)
    #[serde(default)]
    pub tool_timeout_ms: Option<u64>,
    /// Expose streaming events in response
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
    /// Handle the delegate_gemini tool call - now synchronous
    pub async fn handle_call_gem(&self, request: CallToolRequestParam) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: DelegateGeminiParams =
            serde_json::from_value(Value::Object(args)).map_err(|e| {
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

        let model_override = normalize_optional_string(params.model);
        let cwd_input = normalize_optional_string(params.cwd);
        let cwd = if let Some(input) = cwd_input {
            Some(crate::workspace::resolve_workspace(
                &input,
                &self.config.runtime.workspace_map,
            )?)
        } else {
            None
        };
        let timeout = params.timeout_ms.unwrap_or_else(gemini_timeout_ms);
        let tool_timeout = params.tool_timeout_ms.unwrap_or_else(|| {
            std::env::var("GEMINI_TOOL_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(300_000) // 5 minutes default
        });

        // Execute synchronously - call GeminiClient directly
        let result = execute_gemini_call(GeminiCallParams {
            prompt: &prompt,
            model_override: model_override.as_deref(),
            cwd: cwd.as_deref(),
            resume_session_id: params.resume_session_id.as_deref(),
            continue_latest: params.continue_latest,
            timeout,
            tool_timeout,
            expose_stream: params.expose_stream,
        })
        .await;

        match result {
            Ok(response) => {
                let truncated_response =
                    truncate_response(response.response, params.max_response_chars);
                let mut result_json = json!({
                    "status": "completed",
                    "session_id": response.session_id,
                    "response": truncated_response,
                });
                if let Some(events) = response.stream_events {
                    result_json["stream_events"] = serde_json::to_value(events).unwrap_or_default();
                }
                Ok(CallToolResult::structured(result_json))
            }
            Err(e) => {
                let error_msg = match e {
                    AgentError::Timeout { timeout_ms } => {
                        format!("Gemini execution timed out after {}ms", timeout_ms)
                    }
                    AgentError::CliError(msg) => format!("Gemini CLI error: {}", msg),
                    AgentError::NotFound => "Gemini CLI not found".to_string(),
                    AgentError::ParseError(msg) => format!("Parse error: {}", msg),
                    AgentError::StdinError(msg) => format!("Stdin error: {}", msg),
                };
                Err(SurrealMindError::Mcp {
                    message: format!("Gemini execution failed: {}", error_msg),
                })
            }
        }
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

fn default_model_name(config: Option<&crate::config::Config>) -> String {
    std::env::var("GEMINI_MODEL").unwrap_or_else(|_| {
        config
            .map(|c| c.system.gemini_model.clone())
            .unwrap_or_else(|| "auto".to_string())
    })
}
fn gemini_timeout_ms() -> u64 {
    std::env::var("GEMINI_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TIMEOUT_MS)
}

#[derive(Debug)]
struct GeminiCallParams<'a> {
    prompt: &'a str,
    model_override: Option<&'a str>,
    cwd: Option<&'a str>,
    resume_session_id: Option<&'a str>,
    continue_latest: bool,
    timeout: u64,
    tool_timeout: u64,
    expose_stream: bool,
}

async fn execute_gemini_call(
    params: GeminiCallParams<'_>,
) -> std::result::Result<crate::clients::traits::AgentResponse, AgentError> {
    // Determine session to resume:
    // 1. Explicit resume_session_id takes priority
    // 2. continue_latest means use --resume without ID (CLI auto-selects latest)
    // 3. Otherwise start fresh (no resume)
    let resume_session: Option<String> = if let Some(sid) = params.resume_session_id {
        Some(sid.to_string())
    } else if params.continue_latest {
        // Empty string signals "use --resume without ID" to GeminiClient
        Some(String::new())
    } else {
        // Fresh session - no resume
        None
    };

    let config = crate::config::Config::load().ok();
    let model = params
        .model_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| default_model_name(config.as_ref()));

    let mut gemini = GeminiClient::with_timeout_ms(model.clone(), params.timeout);
    gemini = gemini.with_tool_timeout_ms(params.tool_timeout);
    if let Some(dir) = params.cwd {
        gemini = gemini.with_cwd(dir);
    }
    if params.expose_stream {
        gemini = gemini.with_expose_stream(true);
    }

    // Pass session_id to GeminiClient
    // Empty string triggers --resume (latest), non-empty triggers --resume <id>
    gemini.call(params.prompt, resume_session.as_deref()).await
}

// Tests for synchronous call_gem would go here if needed
