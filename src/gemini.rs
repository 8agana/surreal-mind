use serde::{Deserialize, Serialize};
use std::process::Command;

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
    timeout_ms: u64,
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

    pub fn call(
        &self,
        prompt: &str,
        session_id: Option<&str>,
    ) -> Result<GeminiResponse, Box<dyn std::error::Error>> {
        let mut cmd = Command::new("gemini");
        cmd.args(&["-o", "json"]);

        // Pass prompt via stdin to avoid arg length limits
        cmd.arg(prompt);

        if let Some(sid) = session_id {
            cmd.args(&["--resume", sid]);
        }

        let output = cmd.output()?;

        if !output.status.success() {
            return Err(format!(
                "Gemini CLI failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }

        let response: GeminiResponse = serde_json::from_slice(&output.stdout)?;
        Ok(response)
    }

    pub fn check_available() -> bool {
        Command::new("which")
            .arg("gemini")
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}
