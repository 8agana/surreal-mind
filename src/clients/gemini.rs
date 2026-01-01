use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::clients::traits::{AgentError, AgentResponse, CognitiveAgent};

const STDERR_CAP_BYTES: usize = 10 * 1024;
const DEFAULT_TIMEOUT_MS: u64 = 120_000; // 120s inactivity threshold
const DEFAULT_MODEL: &str = "auto";
const ACTIVITY_CHECK_INTERVAL_MS: u64 = 1000; // Check activity every second

#[derive(Debug, Deserialize)]
pub struct GeminiResponse {
    pub session_id: String,
    pub response: String,
}

/// Tracks activity from child process to detect hangs
#[derive(Debug)]
struct ActivityTracker {
    last_activity: Mutex<Instant>,
    bytes_since_reset: AtomicUsize,
    inactivity_threshold: Duration,
    start_time: Instant,
}

impl ActivityTracker {
    fn new(inactivity_threshold: Duration) -> Self {
        let now = Instant::now();
        Self {
            last_activity: Mutex::new(now),
            bytes_since_reset: AtomicUsize::new(0),
            inactivity_threshold,
            start_time: now,
        }
    }

    async fn reset(&self, bytes: usize) {
        let mut last = self.last_activity.lock().await;
        *last = Instant::now();
        self.bytes_since_reset.fetch_add(bytes, Ordering::Relaxed);
    }

    async fn is_inactive(&self) -> bool {
        let last = self.last_activity.lock().await;
        last.elapsed() > self.inactivity_threshold
    }

    async fn inactivity_duration(&self) -> Duration {
        let last = self.last_activity.lock().await;
        last.elapsed()
    }

    fn total_bytes(&self) -> usize {
        self.bytes_since_reset.load(Ordering::Relaxed)
    }

    fn total_runtime(&self) -> Duration {
        self.start_time.elapsed()
    }
}

#[derive(Debug, Clone)]
pub struct GeminiClient {
    model: String,
    timeout: Duration, // Now represents inactivity threshold, not total duration
    cwd: Option<PathBuf>,
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
            cwd: None,
        }
    }

    pub fn with_timeout_ms(model: impl Into<String>, timeout_ms: u64) -> Self {
        Self {
            model: model.into(),
            timeout: Duration::from_millis(timeout_ms),
            cwd: None,
        }
    }

    /// Set the working directory for the Gemini CLI subprocess
    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
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
            .env("CI", "true")
            .env("TERM", "dumb")
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg("-y");

        // Use positional prompt instead of -p flag (deprecated)
        // Auto-routing: if model is "auto", omit the -m flag
        if self.model != "auto" && !self.model.is_empty() {
            cmd.arg("-m").arg(&self.model);
        }

        cmd.arg("-e")
            .arg("")
            .arg("--output-format")
            .arg("json");
        if let Some(sid) = session_id {
            cmd.arg("--resume").arg(sid);
        }
        cmd.arg(prompt);
        if let Some(ref dir) = self.cwd {
            cmd.current_dir(dir);
        }
        let mut child = cmd.spawn().map_err(map_spawn_err)?;

        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentError::CliError("stdout unavailable".to_string()))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| AgentError::CliError("stderr unavailable".to_string()))?;

        // Activity-based timeout: track last output, not total duration
        let tracker = Arc::new(ActivityTracker::new(self.timeout));
        let mut stdout_buf = Vec::new();
        let mut stderr_buf = Vec::new();
        let mut stdout_chunk = [0u8; 4096];
        let mut stderr_chunk = [0u8; 1024];
        loop {
            tokio::select! {
                // Check for stdout data
                result = stdout.read(&mut stdout_chunk) => {
                    match result {
                        Ok(0) => {
                            // EOF on stdout - continue, wait for process exit
                        }
                        Ok(n) => {
                            tracker.reset(n).await;
                            stdout_buf.extend_from_slice(&stdout_chunk[..n]);
                        }
                        Err(e) => {
                            return Err(AgentError::CliError(format!("stdout read error: {}", e)));
                        }
                    }
                }

                // Check for stderr data
                result = stderr.read(&mut stderr_chunk) => {
                    match result {
                        Ok(0) => {
                            // EOF on stderr
                        }
                        Ok(n) => {
                            // stderr output IS activity, reset tracker
                            tracker.reset(n).await;
                            // Cap stderr to prevent memory issues
                            let remaining = STDERR_CAP_BYTES.saturating_sub(stderr_buf.len());
                            if remaining > 0 {
                                let take = remaining.min(n);
                                stderr_buf.extend_from_slice(&stderr_chunk[..take]);
                            }
                        }
                        Err(e) => {
                            tracing::warn!("stderr read error (non-fatal): {}", e);
                        }
                    }
                }

                // Check process status
                result = child.wait() => {
                    match result {
                        Ok(status) => {
                            // Process exited, drain remaining output and break
                            // Drain remaining stdout
                            loop {
                                match stdout.read(&mut stdout_chunk).await {
                                    Ok(0) => break,
                                    Ok(n) => stdout_buf.extend_from_slice(&stdout_chunk[..n]),
                                    Err(_) => break,
                                }
                            }
                            // Drain remaining stderr
                            loop {
                                match stderr.read(&mut stderr_chunk).await {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        let remaining = STDERR_CAP_BYTES.saturating_sub(stderr_buf.len());
                                        if remaining > 0 {
                                            let take = remaining.min(n);
                                            stderr_buf.extend_from_slice(&stderr_chunk[..take]);
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }

                            if !status.success() {
                                let stderr_str = String::from_utf8_lossy(&stderr_buf);
                                return Err(AgentError::CliError(format!(
                                    "gemini exit {}: {}",
                                    status,
                                    stderr_str.trim(),
                                )));
                            }
                            break;
                        }
                        Err(e) => {
                            return Err(AgentError::CliError(format!("wait error: {}", e)));
                        }
                    }
                }

                // Periodic inactivity check
                _ = tokio::time::sleep(Duration::from_millis(ACTIVITY_CHECK_INTERVAL_MS)) => {
                    if tracker.is_inactive().await {
                        let inactivity_secs = tracker.inactivity_duration().await.as_secs();
                        let total_bytes = tracker.total_bytes();
                        let total_runtime = tracker.total_runtime().as_secs();

                        tracing::warn!(
                            "Inactivity timeout: {}s since last output, {} bytes seen total, {}s runtime",
                            inactivity_secs,
                            total_bytes,
                            total_runtime
                        );

                        let stdout_str = String::from_utf8_lossy(&stdout_buf);
                        let stderr_str = String::from_utf8_lossy(&stderr_buf);
                        tracing::debug!("Captured stdout so far: {}", stdout_str);
                        tracing::debug!("Captured stderr so far: {}", stderr_str);

                        // Kill the process - kill_on_drop should handle this, but be explicit
                        let _ = child.kill().await;

                        return Err(AgentError::Timeout {
                            timeout_ms: self.timeout.as_millis() as u64,
                        });
                    }
                }
            }
        }

        let stdout_str = String::from_utf8_lossy(&stdout_buf);
        let response = parse_gemini_response(&stdout_str)?;
        let cleaned = strip_ansi_codes(&response.response).trim().to_string();

        tracing::debug!(
            "Gemini call completed: {} bytes output, {}s runtime",
            tracker.total_bytes(),
            tracker.total_runtime().as_secs()
        );

        Ok(AgentResponse {
            session_id: response.session_id,
            response: cleaned,
            exchange_id: None,
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
                    if depth == 0
                        && let Some(s) = start.take()
                    {
                        candidates.push(text[s..idx + 1].to_string());
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