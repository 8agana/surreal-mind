use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;

use crate::clients::traits::{AgentError, AgentResponse, CognitiveAgent};

const DEFAULT_MODEL: &str = "gpt-5.2-codex";

#[derive(Debug, Clone)]
pub struct CodexClient {
    model: String,
    cwd: Option<PathBuf>,
    resume_session_id: Option<String>,
    continue_latest: bool,
    tool_timeout_ms: Option<u64>,
    expose_stream: bool,
}

#[derive(Debug)]
pub struct CodexExecution {
    pub session_id: Option<String>,
    pub response: String,
    pub stdout: String,
    pub stderr: String,
    pub events: Vec<Value>,
}

impl CodexClient {
    pub fn new(model: Option<String>) -> Self {
        Self {
            model: model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
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

    pub async fn execute(&self, prompt: &str) -> Result<CodexExecution, AgentError> {
        let mut cmd = Command::new("codex");
        cmd.kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(timeout_ms) = self.tool_timeout_ms {
            let timeout_sec = timeout_ms.div_ceil(1000).max(1);
            cmd.env("TOOL_TIMEOUT_SEC", timeout_sec.to_string());
        }

        if let Some(ref resume) = self.resume_session_id {
            cmd.arg("resume").arg(resume);
        } else if self.continue_latest {
            cmd.arg("resume").arg("--last");
        }

        cmd.arg("exec")
            .arg("--json")
            .arg("--color")
            .arg("never")
            .arg("--model")
            .arg(&self.model)
            .arg("--full-auto");

        if let Some(ref cwd) = self.cwd {
            cmd.arg("--cd").arg(cwd);
        }

        cmd.arg(prompt);

        let output = cmd.output().await.map_err(map_spawn_err)?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if stdout.trim().is_empty() && !stderr.trim().is_empty() {
            return Err(AgentError::CliError(format!(
                "codex produced no stdout: {}",
                truncate_snippet(stderr.trim(), 500)
            )));
        }

        if !output.status.success() {
            let hint = classify_codex_error(output.status.code(), &stderr);
            let hint_suffix = hint.map(|h| format!(" (hint: {})", h)).unwrap_or_default();
            return Err(AgentError::CliError(format!(
                "codex exit {}: {}{}",
                output.status,
                truncate_snippet(stderr.trim(), 500),
                hint_suffix
            )));
        }

        let (session_id, response, events) = parse_codex_ndjson(&stdout);
        let response = if response.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            response
        };

        if response.trim().is_empty() {
            return Err(AgentError::CliError(
                "Empty Codex response: no content captured.".to_string(),
            ));
        }

        Ok(CodexExecution {
            session_id,
            response,
            stdout,
            stderr,
            events: if self.expose_stream {
                events
            } else {
                Vec::new()
            },
        })
    }
}

#[async_trait]
impl CognitiveAgent for CodexClient {
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

fn classify_codex_error(exit_code: Option<i32>, stderr: &str) -> Option<&'static str> {
    if exit_code != Some(1) {
        return None;
    }
    let lower = stderr.to_lowercase();
    if lower.contains("401")
        || lower.contains("unauthorized")
        || lower.contains("auth")
        || lower.contains("device code")
    {
        return Some("auth");
    }
    if lower.contains("rate limit") || lower.contains("429") || lower.contains("quota") {
        return Some("rate_limit");
    }
    None
}

fn parse_codex_ndjson(stdout: &str) -> (Option<String>, String, Vec<Value>) {
    let mut session_id = None;
    let mut response_parts: Vec<String> = Vec::new();
    let mut events: Vec<Value> = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(line) {
            Ok(event) => {
                if session_id.is_none() {
                    session_id = event
                        .get("session_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
                if let Some(text) = extract_response_text(&event) {
                    response_parts.push(text.to_string());
                }
                events.push(event);
            }
            Err(_) => {
                response_parts.push(line.to_string());
            }
        }
    }

    (session_id, response_parts.join(""), events)
}

fn extract_response_text(event: &Value) -> Option<&str> {
    event
        .get("content")
        .and_then(|v| v.as_str())
        .or_else(|| event.get("output").and_then(|v| v.as_str()))
        .or_else(|| event.get("response").and_then(|v| v.as_str()))
        .or_else(|| event.get("text").and_then(|v| v.as_str()))
        .or_else(|| event.get("message").and_then(|v| v.as_str()))
        .or_else(|| {
            event
                .get("message")
                .and_then(|v| v.get("content"))
                .and_then(|v| v.as_str())
        })
}

fn truncate_snippet(input: &str, max: usize) -> String {
    if input.len() <= max {
        return input.to_string();
    }
    format!("{}...", &input[..max])
}
