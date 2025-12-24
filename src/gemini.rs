use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiResponse {
    pub session_id: String,
    pub response: String,
    // Additional fields from JSON output as needed
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolSession {
    pub tool_name: String,
    pub gemini_session_id: String,
    pub last_used: chrono::DateTime<chrono::Utc>,
}

pub struct GeminiClient {
    model: String,
    /// Timeout in milliseconds
    timeout_ms: u64,
}

impl Default for GeminiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiClient {
    pub fn new() -> Self {
        Self {
            model: std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.5-pro".to_string()),
            timeout_ms: std::env::var("GEMINI_TIMEOUT_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(60000),
        }
    }

    #[instrument(skip(self, prompt), fields(model = %self.model, timeout_ms = %self.timeout_ms))]
    pub async fn call(
        &self,
        prompt: &str,
        session_id: Option<&str>,
    ) -> Result<GeminiResponse, Box<dyn std::error::Error + Send + Sync>> {
        let mut cmd = Command::new("gemini");
        cmd.args(["-o", "json"]);
        cmd.args(["-m", &self.model]);

        if let Some(sid) = session_id {
            cmd.args(["--resume", sid]);
        }

        // Setup stdin/stdout for piping
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        info!("Starting Gemini CLI process");
        let mut child = cmd.spawn()?;

        // Write prompt to stdin and close it
        if let Some(mut stdin) = child.stdin.take() {
            let mut prompt_bytes = prompt.as_bytes().to_vec();
            prompt_bytes.push(b'\n');
            if let Err(e) = stdin.write_all(&prompt_bytes).await {
                tracing::warn!(
                    "Failed to write to Gemini stdin (process might have exited): {}",
                    e
                );
            }
        } else {
            return Err("Failed to open stdin for Gemini CLI".into());
        }

        // Use configured timeout
        let duration = Duration::from_millis(self.timeout_ms);

        info!(
            "Waiting for Gemini response (timeout: {}ms)",
            self.timeout_ms
        );
        let output_result = timeout(duration, child.wait_with_output()).await;

        let output = match output_result {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => return Err(format!("Gemini CLI execution failed: {}", e).into()),
            Err(_) => {
                return Err(format!("Gemini CLI timed out after {}ms", self.timeout_ms).into());
            }
        };

        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            info!("Gemini CLI failed with stderr: {}", stderr_str);
            return Err(format!("Gemini CLI failed: {}", stderr_str).into());
        }

        if stdout_str.trim().is_empty() {
            return Err(
                format!("Gemini CLI returned empty stdout (stderr: {})", stderr_str).into(),
            );
        }

        info!("Gemini CLI success, parsing response");
        match serde_json::from_str::<GeminiResponse>(&stdout_str) {
            Ok(resp) => Ok(resp),
            Err(e) => Err(format!(
                "Failed to parse Gemini response: {}. stdout (first 500 chars): {} | stderr: {}",
                e,
                stdout_str.chars().take(500).collect::<String>(),
                stderr_str
            )
            .into()),
        }
    }

    pub fn check_available() -> bool {
        std::process::Command::new("which")
            .arg("gemini")
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}
