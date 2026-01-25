//! Vibe CLI client for call_vibe tool
//! Mistral Vibe - one-shot executor, no session persistence

use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use tokio::process::Command;

use crate::clients::traits::{AgentError, AgentResponse, CognitiveAgent};

#[derive(Debug, Clone)]
pub struct VibeClient {
    agent: Option<String>, // --agent flag (profile name)
    cwd: Option<PathBuf>,
    timeout_ms: u64,
}

#[derive(Debug)]
pub struct VibeExecution {
    pub response: String,
    pub stdout: String,
    pub stderr: String,
    pub is_error: bool,
}

impl VibeClient {
    pub fn new(agent: Option<String>) -> Self {
        Self {
            agent,
            cwd: None,
            timeout_ms: 60_000,
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

    pub async fn execute(&self, prompt: &str) -> Result<VibeExecution, AgentError> {
        let mut cmd = Command::new("vibe");
        cmd.kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(ref cwd) = self.cwd {
            cmd.current_dir(cwd);
        }

        // vibe --auto-approve -p "<prompt>" [--agent <name>]
        cmd.arg("--auto-approve");
        cmd.arg("-p").arg(prompt);

        if let Some(ref agent) = self.agent {
            cmd.arg("--agent").arg(agent);
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

        if stdout.trim().is_empty() && !stderr.trim().is_empty() {
            return Err(AgentError::CliError(format!(
                "vibe produced no stdout: {}",
                truncate_snippet(stderr.trim(), 500)
            )));
        }

        if !output.status.success() {
            return Err(AgentError::CliError(format!(
                "vibe exit {}: {}",
                output.status,
                truncate_snippet(stderr.trim(), 500)
            )));
        }

        let response = stdout.trim().to_string();
        if response.is_empty() {
            return Err(AgentError::CliError(
                "Empty Vibe response: no content captured.".to_string(),
            ));
        }

        Ok(VibeExecution {
            response,
            stdout,
            stderr,
            is_error: false,
        })
    }
}

#[async_trait]
impl CognitiveAgent for VibeClient {
    async fn call(
        &self,
        prompt: &str,
        _session_id: Option<&str>, // Vibe doesn't support sessions
    ) -> Result<AgentResponse, AgentError> {
        let execution = self.execute(prompt).await?;
        Ok(AgentResponse {
            session_id: String::new(), // Vibe has no sessions
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
