use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::clients::traits::{AgentError, AgentResponse, CognitiveAgent};

const STDERR_CAP_BYTES: usize = 10 * 1024;
const DEFAULT_TIMEOUT_MS: u64 = 120_000; // 120s inactivity threshold
const DEFAULT_TOOL_TIMEOUT_MS: u64 = 300_000; // 300s per-tool timeout
const DEFAULT_MODEL: &str = "auto";
const ACTIVITY_CHECK_INTERVAL_MS: u64 = 1000; // Check activity every second

/// Streaming JSON event types from Gemini CLI
#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GeminiStreamEvent {
    Init {
        session_id: String,
        model: String,
    },
    ToolUse {
        tool_name: String,
        parameters: serde_json::Value,
    },
    ToolResult {
        status: String,
        output: Option<String>,
    },
    Content {
        text: String,
    },
    Message {
        role: String,
        content: String,
        #[serde(default)]
        delta: bool,
    },
    Result {
        status: String,
        #[serde(default)]
        stats: serde_json::Value,
    },
    Error {
        message: String,
    },
    End {
        session_id: String,
    },
}

/// Simple streaming JSON parser
pub struct StreamJsonParser {
    buffer: String,
}

impl Default for StreamJsonParser {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamJsonParser {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    pub fn parse_chunk(&mut self, chunk: &str) -> Vec<GeminiStreamEvent> {
        self.buffer.push_str(chunk);
        let mut events = Vec::new();

        // Process complete lines
        while let Some(pos) = self.buffer.find('\n') {
            let line = self.buffer.drain(..=pos).collect::<String>();
            let line = line.trim();
            
            if line.is_empty() {
                continue;
            }
            
            // Strip optional "data:" prefix that some CLI tools add
            let line = if let Some(stripped) = line.strip_prefix("data:") {
                stripped.trim()
            } else {
                line
            };
            
            // Try to parse as GeminiStreamEvent
            match serde_json::from_str::<GeminiStreamEvent>(line) {
                Ok(event) => {
                    events.push(event);
                }
                Err(e) => {
                    tracing::debug!("Failed to parse stream JSON line: {}", e);
                    tracing::debug!("Problematic line: {}", line);
                }
            }
        }

        events
    }
}

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
    tool_timeout: Duration,
    start_time: Instant,
    active_tools: Mutex<HashSet<String>>,
    tool_activity: Mutex<HashMap<String, Instant>>,
}

impl ActivityTracker {
    fn new(inactivity_threshold: Duration, tool_timeout: Duration) -> Self {
        let now = Instant::now();
        Self {
            last_activity: Mutex::new(now),
            bytes_since_reset: AtomicUsize::new(0),
            inactivity_threshold,
            tool_timeout,
            start_time: now,
            active_tools: Mutex::new(HashSet::new()),
            tool_activity: Mutex::new(HashMap::new()),
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

    /// Track when a tool starts executing
    async fn tool_started(&self, tool_name: String) {
        let mut tools = self.active_tools.lock().await;
        let mut activity = self.tool_activity.lock().await;

        tools.insert(tool_name.clone());
        activity.insert(tool_name, Instant::now());
    }

    /// Track when a tool completes
    async fn tool_completed(&self, tool_name: &str) {
        let mut tools = self.active_tools.lock().await;
        let mut activity = self.tool_activity.lock().await;

        tools.remove(tool_name);
        activity.remove(tool_name);
    }

    /// Check if any tools have timed out
    async fn check_tool_timeouts(&self) -> Option<String> {
        let tools = self.active_tools.lock().await;
        let activity = self.tool_activity.lock().await;

        for tool in tools.iter() {
            if let Some(last) = activity.get(tool)
                && last.elapsed() > self.tool_timeout
            {
                return Some(tool.clone());
            }
        }
        None
    }

    /// Check if we should timeout (inactivity OR tool timeout)
    async fn should_timeout(&self) -> bool {
        if self.is_inactive().await {
            return true;
        }

        if let Some(hung_tool) = self.check_tool_timeouts().await {
            tracing::warn!("Tool timeout detected: {}", hung_tool);
            return true;
        }

        false
    }
}

#[derive(Debug, Clone)]
pub struct GeminiClient {
    model: String,
    timeout: Duration,      // Inactivity threshold
    tool_timeout: Duration, // Per-tool execution timeout
    cwd: Option<PathBuf>,
    expose_stream: bool, // Whether to expose streaming events to callers
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
        let tool_timeout_ms = std::env::var("GEMINI_TOOL_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_TOOL_TIMEOUT_MS);
        Self {
            model,
            timeout: Duration::from_millis(timeout_ms),
            tool_timeout: Duration::from_millis(tool_timeout_ms),
            cwd: None,
            expose_stream: false,
        }
    }

    pub fn with_timeout_ms(model: impl Into<String>, timeout_ms: u64) -> Self {
        Self {
            model: model.into(),
            timeout: Duration::from_millis(timeout_ms),
            tool_timeout: Duration::from_millis(DEFAULT_TOOL_TIMEOUT_MS),
            cwd: None,
            expose_stream: false,
        }
    }

    /// Set the working directory for the Gemini CLI subprocess
    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Enable streaming event exposure
    pub fn with_expose_stream(mut self, expose: bool) -> Self {
        self.expose_stream = expose;
        self
    }

    /// Set per-tool timeout
    pub fn with_tool_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.tool_timeout = Duration::from_millis(timeout_ms);
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

        // Use streaming JSON format for real-time monitoring
        cmd.arg("-e")
            .arg("")
            .arg("--output-format")
            .arg("stream-json");
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

        // Activity-based timeout: track last output and tool execution
        let tracker = Arc::new(ActivityTracker::new(self.timeout, self.tool_timeout));
        let mut stdout_buf = Vec::new();
        let mut stderr_buf = Vec::new();
        let mut stdout_chunk = [0u8; 4096];
        let mut stderr_chunk = [0u8; 1024];

        // Streaming JSON parser
        let mut stream_parser = StreamJsonParser::new();
        let mut session_id_from_stream = None;
        let mut content_buffer = String::new();
        let mut stream_events = Vec::new();

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

                            // Parse streaming JSON events
                            let chunk_str = String::from_utf8_lossy(&stdout_chunk[..n]);
                            let events = stream_parser.parse_chunk(&chunk_str);

                            for event in events {
                                match &event {
                                    GeminiStreamEvent::Init { session_id, .. } => {
                                        session_id_from_stream = Some(session_id.clone());
                                        if self.expose_stream {
                                            stream_events.push(event.clone());
                                        }
                                    }
                                    GeminiStreamEvent::ToolUse { tool_name, .. } => {
                                        tracker.tool_started(tool_name.clone()).await;
                                        if self.expose_stream {
                                            stream_events.push(event.clone());
                                        }
                                    }
                                    GeminiStreamEvent::ToolResult { status: _, .. } => {
                                        // Try to extract tool name from context (simplified)
                                        if let Some(last_event) = stream_events.last()
                                            && let GeminiStreamEvent::ToolUse { tool_name, .. } = last_event {
                                            tracker.tool_completed(tool_name).await;
                                        }
                                        if self.expose_stream {
                                            stream_events.push(event.clone());
                                        }
                                    }
                                    GeminiStreamEvent::Content { text } => {
                                        content_buffer.push_str(text);
                                        if self.expose_stream {
                                            stream_events.push(event.clone());
                                        }
                                    }
                                    GeminiStreamEvent::Message { role, content, delta } => {
                                        // Only capture assistant messages, ignore user/system
                                        if role == "assistant" {
                                            if *delta {
                                                content_buffer.push_str(content);
                                            } else {
                                                // Non-delta message: replace buffer
                                                content_buffer = content.clone();
                                            }
                                            if self.expose_stream {
                                                stream_events.push(event.clone());
                                            }
                                        }
                                    }
                                    GeminiStreamEvent::Result { status, .. } => {
                                        if self.expose_stream {
                                            stream_events.push(event.clone());
                                        }
                                        tracing::debug!("Gemini result status: {}", status);
                                    }
                                    GeminiStreamEvent::Error { message, .. } => {
                                        if self.expose_stream {
                                            stream_events.push(event.clone());
                                        }
                                        tracing::error!("Gemini stream error: {}", message);
                                    }
                                    GeminiStreamEvent::End { .. } => {
                                        if self.expose_stream {
                                            stream_events.push(event.clone());
                                        }
                                    }
                                }
                            }
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

                // Periodic inactivity and tool timeout check
                _ = tokio::time::sleep(Duration::from_millis(ACTIVITY_CHECK_INTERVAL_MS)) => {
                    if tracker.should_timeout().await {
                        let inactivity_secs = tracker.inactivity_duration().await.as_secs();
                        let total_bytes = tracker.total_bytes();
                        let total_runtime = tracker.total_runtime().as_secs();

                        tracing::warn!(
                            "Timeout detected: {}s since last output, {} bytes seen total, {}s runtime",
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

        // Use session ID from stream if available, otherwise parse from final output
        let session_id = session_id_from_stream.unwrap_or_else(|| {
            let stdout_str = String::from_utf8_lossy(&stdout_buf);
            parse_gemini_response(&stdout_str)
                .ok()
                .map(|r| r.session_id)
                .unwrap_or_default()
        });

        // Use content from stream if available, otherwise parse from final output
        let response_text = if !content_buffer.is_empty() {
            content_buffer
        } else {
            let stdout_str = String::from_utf8_lossy(&stdout_buf);
            parse_gemini_response(&stdout_str)
                .ok()
                .map(|r| r.response)
                .unwrap_or_default()
        };

        // Check if response is empty and provide debugging information
        if response_text.trim().is_empty() {
            let stdout_str = String::from_utf8_lossy(&stdout_buf);
            let stderr_str = String::from_utf8_lossy(&stderr_buf);
            
            let stdout_snippet = if stdout_str.len() > 200 {
                format!("{}...", &stdout_str[..200])
            } else {
                stdout_str.to_string()
            };
            
            let stderr_snippet = if stderr_str.len() > 200 {
                format!("{}...", &stderr_str[..200])
            } else {
                stderr_str.to_string()
            };
            
            return Err(AgentError::CliError(format!(
                "Empty Gemini response: no content captured. stdout: {}, stderr: {}",
                stdout_snippet.trim(),
                stderr_snippet.trim()
            )));
        }

        let cleaned = strip_ansi_codes(&response_text).trim().to_string();

        tracing::debug!(
            "Gemini call completed: {} bytes output, {}s runtime, expose_stream: {}",
            tracker.total_bytes(),
            tracker.total_runtime().as_secs(),
            self.expose_stream
        );

        Ok(AgentResponse {
            session_id,
            response: cleaned,
            exchange_id: None,
            stream_events: if self.expose_stream {
                Some(stream_events)
            } else {
                None
            },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_json_parser_legacy_content_events() {
        let mut parser = StreamJsonParser::new();
        
        // Test legacy Content event
        let chunk = r#"{"type":"content","text":"Hello world"}
"#;
        let events = parser.parse_chunk(chunk);
        
        assert_eq!(events.len(), 1);
        if let GeminiStreamEvent::Content { text } = &events[0] {
            assert_eq!(text, "Hello world");
        } else {
            panic!("Expected Content event");
        }
    }

    #[test]
    fn test_stream_json_parser_new_message_events() {
        let mut parser = StreamJsonParser::new();
        
        // Test new Message event with delta
        let chunk = r#"{"type":"message","role":"assistant","content":"Hello","delta":true}
"#;
        let events = parser.parse_chunk(chunk);
        
        assert_eq!(events.len(), 1);
        if let GeminiStreamEvent::Message { role, content, delta } = &events[0] {
            assert_eq!(role, "assistant");
            assert_eq!(content, "Hello");
            assert!(*delta);
        } else {
            panic!("Expected Message event");
        }
    }

    #[test]
    fn test_stream_json_parser_mixed_events() {
        let mut parser = StreamJsonParser::new();
        
        // Test mixed old and new event formats
        let chunk = r#"{"type":"init","session_id":"test123","model":"gemini-pro"}
{"type":"message","role":"assistant","content":"Response","delta":false}
{"type":"result","status":"success"}
{"type":"end","session_id":"test123"}
"#;
        
        let events = parser.parse_chunk(chunk);
        assert_eq!(events.len(), 4);
        
        // Verify each event type
        if let GeminiStreamEvent::Init { session_id, model } = &events[0] {
            assert_eq!(session_id, "test123");
            assert_eq!(model, "gemini-pro");
        } else {
            panic!("Expected Init event");
        }
        
        if let GeminiStreamEvent::Message { role, content, delta } = &events[1] {
            assert_eq!(role, "assistant");
            assert_eq!(content, "Response");
            assert!(!*delta);
        } else {
            panic!("Expected Message event");
        }
        
        if let GeminiStreamEvent::Result { status, .. } = &events[2] {
            assert_eq!(status, "success");
        } else {
            panic!("Expected Result event");
        }
        
        if let GeminiStreamEvent::End { session_id } = &events[3] {
            assert_eq!(session_id, "test123");
        } else {
            panic!("Expected End event");
        }
    }

    #[test]
    fn test_stream_json_parser_data_prefix_stripping() {
        let mut parser = StreamJsonParser::new();
        
        // Test "data:" prefix stripping
        let chunk = r#"data: {"type":"message","role":"assistant","content":"Test"}
"#;
        let events = parser.parse_chunk(chunk);
        
        assert_eq!(events.len(), 1);
        if let GeminiStreamEvent::Message { content, .. } = &events[0] {
            assert_eq!(content, "Test");
        } else {
            panic!("Expected Message event after data: prefix");
        }
    }

    #[test]
    fn test_stream_json_parser_empty_lines() {
        let mut parser = StreamJsonParser::new();
        
        // Test that empty lines are ignored
        let chunk = r#"

{"type":"content","text":"Valid"}

"#;
        let events = parser.parse_chunk(chunk);
        
        assert_eq!(events.len(), 1);
        if let GeminiStreamEvent::Content { text } = &events[0] {
            assert_eq!(text, "Valid");
        } else {
            panic!("Expected Content event");
        }
    }

    #[test]
    fn test_stream_json_parser_malformed_lines() {
        let mut parser = StreamJsonParser::new();
        
        // Test that malformed JSON lines don't crash the parser
        let chunk = r#"{"invalid":json
{"type":"content","text":"Valid"}
not json at all"#;
        let events = parser.parse_chunk(chunk);
        
        // Should still parse the valid line
        assert_eq!(events.len(), 1);
        if let GeminiStreamEvent::Content { text } = &events[0] {
            assert_eq!(text, "Valid");
        } else {
            panic!("Expected Content event despite malformed lines");
        }
    }
}
