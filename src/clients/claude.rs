//! Claude Code CLI client for call_cc tool

use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;

use crate::clients::traits::{AgentError, AgentResponse, CognitiveAgent};

const FALLBACK_MODEL: &str = "claude-haiku-4-5";

/// Get default Claude model from ANTHROPIC_MODEL env var
fn get_default_model() -> String {
    std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| FALLBACK_MODEL.to_string())
}

#[derive(Debug, Clone)]
pub struct ClaudeClient {
    model: String,
    cwd: Option<PathBuf>,
    resume_session_id: Option<String>,
    continue_latest: bool,
    tool_timeout_ms: Option<u64>,
    expose_stream: bool,
}

#[derive(Debug)]
pub struct ClaudeExecution {
    pub session_id: Option<String>,
    pub response: String,
    pub stdout: String,
    pub stderr: String,
    pub events: Vec<Value>,
    pub is_error: bool,
}

impl ClaudeClient {
    pub fn new(model: Option<String>) -> Self {
        Self {
            model: model.unwrap_or_else(get_default_model),
            cwd: None,
            resume_session_id: None,
            continue_latest: false,
            tool_timeout_ms: None,
            expose_stream: false,
        }
    }

    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn with_resume_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.resume_session_id = Some(session_id.into());
        self
    }

    pub fn with_continue_latest(mut self, continue_latest: bool) -> Self {
        self.continue_latest = continue_latest;
        self
    }

    pub fn with_tool_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.tool_timeout_ms = Some(timeout_ms);
        self
    }

    pub fn with_expose_stream(mut self, expose: bool) -> Self {
        self.expose_stream = expose;
        self
    }

    pub async fn execute(&self, prompt: &str) -> Result<ClaudeExecution, AgentError> {
        let mut cmd = Command::new("claude");
        cmd.kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Model via env var (Claude Code doesn't have --model flag)
        cmd.env("ANTHROPIC_MODEL", &self.model);

        // Tool timeout via env var (milliseconds)
        if let Some(timeout_ms) = self.tool_timeout_ms {
            cmd.env("MCP_TOOL_TIMEOUT", timeout_ms.to_string());
        }

        // CWD via Command method (no --cd flag)
        if let Some(ref cwd) = self.cwd {
            cmd.current_dir(cwd);
        }

        // Core flags for non-interactive JSON output
        cmd.arg("-p").arg(prompt);
        cmd.arg("--dangerously-skip-permissions");
        cmd.arg("--verbose"); // Required when using stream-json with -p
        cmd.arg("--output-format").arg("stream-json");

        // Resume handling (mutually exclusive)
        if let Some(ref session_id) = self.resume_session_id {
            cmd.arg("--resume").arg(session_id);
        } else if self.continue_latest {
            cmd.arg("-c");
        }

        let output = cmd.output().await.map_err(map_spawn_err)?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if stdout.trim().is_empty() && !stderr.trim().is_empty() {
            return Err(AgentError::CliError(format!(
                "claude produced no stdout: {}",
                truncate_snippet(stderr.trim(), 500)
            )));
        }

        if !output.status.success() {
            let hint = classify_claude_error(output.status.code(), &stderr);
            let hint_suffix = hint.map(|h| format!(" (hint: {})", h)).unwrap_or_default();
            return Err(AgentError::CliError(format!(
                "claude exit {}: {}{}",
                output.status,
                truncate_snippet(stderr.trim(), 500),
                hint_suffix
            )));
        }

        let (session_id, response, events, is_error) = parse_claude_stream_json(&stdout);
        let response = if response.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            response
        };

        // Check for error events in stream
        if is_error {
            return Err(AgentError::CliError(format!(
                "Claude returned error: {}",
                truncate_snippet(&response, 500)
            )));
        }

        if response.trim().is_empty() {
            return Err(AgentError::CliError(
                "Empty Claude response: no content captured.".to_string(),
            ));
        }

        Ok(ClaudeExecution {
            session_id,
            response,
            stdout,
            stderr,
            events: if self.expose_stream {
                events
            } else {
                Vec::new()
            },
            is_error: false,
        })
    }
}

#[async_trait]
impl CognitiveAgent for ClaudeClient {
    async fn call(
        &self,
        prompt: &str,
        _session_id: Option<&str>,
    ) -> Result<AgentResponse, AgentError> {
        let execution = self.execute(prompt).await?;
        Ok(AgentResponse {
            session_id: execution.session_id.unwrap_or_default(),
            response: execution.response,
            exchange_id: None,
            stream_events: None,
        })
    }
}

fn map_spawn_err(err: std::io::Error) -> AgentError {
    if err.kind() == std::io::ErrorKind::NotFound {
        AgentError::NotFound
    } else {
        AgentError::CliError(err.to_string())
    }
}

fn classify_claude_error(exit_code: Option<i32>, stderr: &str) -> Option<&'static str> {
    if exit_code != Some(1) {
        return None;
    }
    let lower = stderr.to_lowercase();
    if lower.contains("401")
        || lower.contains("unauthorized")
        || lower.contains("auth")
        || lower.contains("api key")
    {
        return Some("auth");
    }
    if lower.contains("rate limit") || lower.contains("429") || lower.contains("quota") {
        return Some("rate_limit");
    }
    None
}

/// Parse Claude Code's stream-json NDJSON output
fn parse_claude_stream_json(stdout: &str) -> (Option<String>, String, Vec<Value>, bool) {
    let mut session_id = None;
    let mut response_parts: Vec<String> = Vec::new();
    let mut events: Vec<Value> = Vec::new();
    let mut is_error = false;

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Handle potential SSE "data: " prefix (Claude might use this)
        let json_str = line.strip_prefix("data: ").unwrap_or(line);

        match serde_json::from_str::<Value>(json_str) {
            Ok(event) => {
                // Check for error events (MCP protocol: isError: true)
                if event
                    .get("isError")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    is_error = true;
                }
                // Also check for result.isError pattern
                if let Some(result) = event.get("result") {
                    if result
                        .get("isError")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                    {
                        is_error = true;
                    }
                }

                // Extract session_id from various possible fields
                if session_id.is_none() {
                    session_id = event
                        .get("session_id")
                        .and_then(|v| v.as_str())
                        .or_else(|| event.get("sessionId").and_then(|v| v.as_str()))
                        .or_else(|| event.get("conversation_id").and_then(|v| v.as_str()))
                        .or_else(|| event.get("thread_id").and_then(|v| v.as_str()))
                        .map(|s| s.to_string());
                }

                // Extract response text
                if let Some(text) = extract_response_text(&event) {
                    response_parts.push(text.to_string());
                }
                events.push(event);
            }
            Err(_) => {
                // Non-JSON line - include as raw text
                response_parts.push(line.to_string());
            }
        }
    }

    (session_id, response_parts.join(""), events, is_error)
}

fn extract_response_text(event: &Value) -> Option<&str> {
    // Check for MCP result content
    if let Some(result) = event.get("result") {
        if let Some(content) = result.get("content") {
            if let Some(arr) = content.as_array() {
                for item in arr {
                    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                        return Some(text);
                    }
                }
            }
        }
    }

    // Check for message content (Claude API format)
    if let Some(message) = event.get("message") {
        if let Some(content) = message.get("content") {
            if let Some(arr) = content.as_array() {
                for item in arr {
                    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                        return Some(text);
                    }
                }
            }
            if let Some(text) = content.as_str() {
                return Some(text);
            }
        }
    }

    // Check for delta content (streaming)
    if let Some(delta) = event.get("delta") {
        if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
            return Some(text);
        }
    }

    // Generic fallbacks
    event
        .get("content")
        .and_then(|v| v.as_str())
        .or_else(|| event.get("output").and_then(|v| v.as_str()))
        .or_else(|| event.get("response").and_then(|v| v.as_str()))
        .or_else(|| event.get("text").and_then(|v| v.as_str()))
}

fn truncate_snippet(input: &str, max: usize) -> String {
    if input.len() <= max {
        return input.to_string();
    }
    format!("{}...", &input[..max])
}
