//! Warp CLI client for call_warp tool
//! Warp is a one-shot executor - no session persistence or resume support

use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use tokio::process::Command;

use crate::clients::traits::{AgentError, AgentResponse, CognitiveAgent};

#[derive(Debug, Clone)]
pub struct WarpClient {
    model: Option<String>, // None = omit flag (Warp picks default)
    cwd: Option<PathBuf>,
    timeout_ms: u64,
}

#[derive(Debug)]
pub struct WarpExecution {
    pub response: String,
    pub stdout: String,
    pub stderr: String,
    pub is_error: bool,
}

impl WarpClient {
    pub fn new(model: Option<String>) -> Self {
        Self {
            model,
            cwd: None,
            timeout_ms: 60_000, // 1 minute default
        }
    }

    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    pub async fn execute(&self, prompt: &str) -> Result<WarpExecution, AgentError> {
        let mut cmd = Command::new("warp");
        cmd.kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // CWD via Command method
        if let Some(ref cwd) = self.cwd {
            cmd.current_dir(cwd);
        }

        // Base command: warp agent run
        cmd.arg("agent").arg("run");

        // Model flag (optional - omit if None)
        if let Some(ref model) = self.model {
            cmd.arg("--model").arg(model);
        }

        // Prompt
        cmd.arg("--prompt").arg(prompt);

        // CWD flag (Warp's --cwd, different from Command current_dir)
        if let Some(ref cwd) = self.cwd {
            cmd.arg("--cwd").arg(cwd);
        }

        // Execute with timeout
        let timeout = std::time::Duration::from_millis(self.timeout_ms);
        let output = tokio::time::timeout(timeout, cmd.output())
            .await
            .map_err(|_| AgentError::Timeout {
                timeout_ms: self.timeout_ms,
            })?
            .map_err(map_spawn_err)?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Check for empty output
        if stdout.trim().is_empty() && !stderr.trim().is_empty() {
            return Err(AgentError::CliError(format!(
                "warp produced no stdout: {}",
                truncate_snippet(stderr.trim(), 500)
            )));
        }

        // Check exit status
        if !output.status.success() {
            return Err(AgentError::CliError(format!(
                "warp exit {}: {}",
                output.status,
                truncate_snippet(stderr.trim(), 500)
            )));
        }

        let response = stdout.trim().to_string();

        // Check for empty response
        if response.is_empty() {
            return Err(AgentError::CliError(
                "Empty Warp response: no content captured.".to_string(),
            ));
        }

        Ok(WarpExecution {
            response,
            stdout,
            stderr,
            is_error: false,
        })
    }
}

#[async_trait]
impl CognitiveAgent for WarpClient {
    async fn call(
        &self,
        prompt: &str,
        _session_id: Option<&str>, // Warp doesn't support sessions
    ) -> Result<AgentResponse, AgentError> {
        let execution = self.execute(prompt).await?;
        Ok(AgentResponse {
            session_id: String::new(), // Warp has no sessions
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

fn truncate_snippet(input: &str, max: usize) -> String {
    if input.len() <= max {
        return input.to_string();
    }
    format!("{}...", &input[..max])
}
