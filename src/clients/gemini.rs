use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::time::timeout;

use crate::clients::traits::{AgentError, AgentResponse, CognitiveAgent};

const STDERR_CAP_BYTES: usize = 10 * 1024;
const DEFAULT_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_MODEL: &str = "gemini-2.5-pro";

#[derive(Debug, Deserialize)]
pub struct GeminiResponse {
    pub session_id: String,
    pub response: String,
}

#[derive(Debug, Clone)]
pub struct GeminiClient {
    model: String,
    timeout: Duration,
}

impl Default for GeminiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiClient {
    pub fn new() -> Self {
        let model = std::env::var("GEMINI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        let timeout_ms = std::env::var("GEMINI_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_TIMEOUT_MS);
        Self {
            model,
            timeout: Duration::from_millis(timeout_ms),
        }
    }

    pub fn with_timeout_ms(model: impl Into<String>, timeout_ms: u64) -> Self {
        Self {
            model: model.into(),
            timeout: Duration::from_millis(timeout_ms),
        }
    }
}

#[async_trait]
impl CognitiveAgent for GeminiClient {
    async fn call(
        &self,
        prompt: &str,
        session_id: Option<&str>,
    ) -> Result<AgentResponse, AgentError> {
        let mut cmd = Command::new("gemini");
        cmd.kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg("-m")
            .arg(&self.model)
            .arg("-o")
            .arg("json")
            .arg("-p")
            .arg("-");
        if let Some(sid) = session_id {
            cmd.arg("--resume").arg(sid);
        }
        let mut child = cmd.spawn().map_err(map_spawn_err)?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| AgentError::StdinError("stdin unavailable".to_string()))?;
        stdin
            .write_all(prompt.as_bytes())
            .await
            .map_err(|e| AgentError::StdinError(e.to_string()))?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|e| AgentError::StdinError(e.to_string()))?;
        stdin
            .flush()
            .await
            .map_err(|e| AgentError::StdinError(e.to_string()))?;
        drop(stdin);

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentError::CliError("stdout unavailable".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AgentError::CliError("stderr unavailable".to_string()))?;
        let stdout_task = tokio::spawn(async move { read_all(stdout).await });
        let stderr_task =
            tokio::spawn(async move { read_with_cap(stderr, STDERR_CAP_BYTES).await });

        let status = match timeout(self.timeout, child.wait()).await {
            Ok(res) => res.map_err(|e| AgentError::CliError(e.to_string()))?,
            Err(_) => {
                let _ = child.kill().await;
                stdout_task.abort();
                stderr_task.abort();
                return Err(AgentError::Timeout {
                    timeout_ms: self.timeout.as_millis() as u64,
                });
            }
        };
        let stdout_bytes = stdout_task
            .await
            .map_err(|e| AgentError::CliError(e.to_string()))?
            .map_err(|e| AgentError::CliError(e.to_string()))?;
        let stderr_bytes = stderr_task
            .await
            .map_err(|e| AgentError::CliError(e.to_string()))?
            .map_err(|e| AgentError::CliError(e.to_string()))?;

        if !status.success() {
            let stderr_str = String::from_utf8_lossy(&stderr_bytes);
            return Err(AgentError::CliError(format!(
                "gemini exit {}: {}",
                status,
                stderr_str.trim(),
            )));
        }

        let stdout_str = String::from_utf8_lossy(&stdout_bytes);
        let response = parse_gemini_response(&stdout_str)?;
        let cleaned = strip_ansi_codes(&response.response).trim().to_string();

        Ok(AgentResponse {
            session_id: response.session_id,
            response: cleaned,
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

async fn read_all<R: AsyncRead + Unpin>(mut reader: R) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).await?;
    Ok(buf)
}

async fn read_with_cap<R: AsyncRead + Unpin>(
    mut reader: R,
    cap: usize,
) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let n = reader.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        let remaining = cap.saturating_sub(buf.len());
        if remaining > 0 {
            let take = remaining.min(n);
            buf.extend_from_slice(&chunk[..take]);
        }
        if buf.len() >= cap {
            while reader.read(&mut chunk).await? != 0 {}
            break;
        }
    }
    Ok(buf)
}

fn parse_gemini_response(output: &str) -> Result<GeminiResponse, AgentError> {
    let cleaned = strip_ansi_codes(output);
    if let Ok(resp) = serde_json::from_str::<GeminiResponse>(&cleaned) {
        return Ok(resp);
    }

    let candidates = extract_json_candidates(&cleaned);
    for candidate in candidates.iter().rev() {
        if let Ok(resp) = serde_json::from_str::<GeminiResponse>(candidate) {
            return Ok(resp);
        }
    }

    let snippet = truncate_chars(cleaned.trim(), 500);
    Err(AgentError::ParseError(format!(
        "no valid JSON object found in Gemini output: {}",
        snippet
    )))
}

fn extract_json_candidates(text: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut depth: u32 = 0;
    let mut start: Option<usize> = None;
    let mut in_string = false;
    let mut escape = false;

    for (idx, ch) in text.char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = Some(idx);
                }
                depth += 1;
            }
            '}' => {
                if depth > 0 {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(s) = start.take() {
                            candidates.push(text[s..idx + 1].to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    candidates
}

fn strip_ansi_codes(input: &str) -> String {
    static ANSI_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]").unwrap());
    ANSI_RE.replace_all(input, "").to_string()
}

fn truncate_chars(input: &str, max: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in input.chars().enumerate() {
        if idx >= max {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}
