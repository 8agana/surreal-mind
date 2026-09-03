//! Antigravity CLI client shared by kg_populate and kg_wander.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::process::Command;

use crate::clients::traits::{AgentError, AgentResponse, CognitiveAgent};

const DEFAULT_PRINT_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_TIMEOUT_MS: u64 = DEFAULT_PRINT_TIMEOUT_MS;
const DEFAULT_BIN: &str = "agy";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AntigravityPermissionMode {
    Default,
    Sandbox,
    SkipPermissions,
}

impl AntigravityPermissionMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "default" => Some(Self::Default),
            "sandbox" => Some(Self::Sandbox),
            "skip"
            | "skip_permissions"
            | "skip-permissions"
            | "dangerous"
            | "dangerously-skip-permissions"
            | "interactive_skip"
            | "interactive-skip" => Some(Self::SkipPermissions),
            _ => None,
        }
    }

    pub fn for_kg() -> Self {
        std::env::var("ANTIGRAVITY_KG_PERMISSION_MODE")
            .ok()
            .or_else(|| std::env::var("ANTIGRAVITY_PERMISSION_MODE").ok())
            .and_then(|v| Self::parse(&v))
            .unwrap_or(Self::Sandbox)
    }
}

#[derive(Debug, Clone)]
struct AntigravityInvocation {
    bin: PathBuf,
    args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AntigravityClient {
    bin: PathBuf,
    model: Option<String>,
    timeout: Duration,
    print_timeout: Duration,
    cwd: Option<PathBuf>,
    permission_mode: AntigravityPermissionMode,
}

#[derive(Debug)]
pub struct AntigravityExecution {
    pub response: String,
    pub stdout: String,
    pub stderr: String,
}

impl Default for AntigravityClient {
    fn default() -> Self {
        Self::new(None)
    }
}

impl AntigravityClient {
    pub fn new(model: Option<String>) -> Self {
        let bin = std::env::var("ANTIGRAVITY_CLI_BIN")
            .or_else(|_| std::env::var("AGY_CLI_BIN"))
            .unwrap_or_else(|_| DEFAULT_BIN.to_string());
        let timeout_ms = std::env::var("ANTIGRAVITY_TIMEOUT_MS")
            .or_else(|_| std::env::var("AGY_TIMEOUT_MS"))
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_TIMEOUT_MS);
        let print_timeout_ms = std::env::var("ANTIGRAVITY_PRINT_TIMEOUT_MS")
            .or_else(|_| std::env::var("AGY_PRINT_TIMEOUT_MS"))
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_PRINT_TIMEOUT_MS);

        Self {
            bin: PathBuf::from(bin),
            model: normalize_model(model),
            timeout: Duration::from_millis(timeout_ms),
            print_timeout: Duration::from_millis(print_timeout_ms),
            cwd: None,
            permission_mode: AntigravityPermissionMode::Default,
        }
    }

    pub fn with_bin(mut self, bin: impl Into<PathBuf>) -> Self {
        self.bin = bin.into();
        self
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout = Duration::from_millis(timeout_ms);
        self
    }

    pub fn with_print_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.print_timeout = Duration::from_millis(timeout_ms);
        self
    }

    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn with_permission_mode(mut self, mode: AntigravityPermissionMode) -> Self {
        self.permission_mode = mode;
        self
    }

    pub async fn execute(
        &self,
        prompt: &str,
        session_id: Option<&str>,
    ) -> Result<AntigravityExecution, AgentError> {
        let invocation = self.build_invocation(prompt, session_id);
        let mut cmd = Command::new(&invocation.bin);
        cmd.kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .args(&invocation.args);

        if let Some(ref cwd) = self.cwd {
            cmd.current_dir(cwd);
        }

        let output = tokio::time::timeout(self.timeout, cmd.output())
            .await
            .map_err(|_| AgentError::Timeout {
                timeout_ms: self.timeout.as_millis() as u64,
            })?
            .map_err(map_spawn_err)?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if looks_like_auth_required(&stdout) || looks_like_auth_required(&stderr) {
            return Err(AgentError::CliError(
                "Antigravity CLI is not authenticated for this process context. Run `agy` interactively in the same user/session and complete browser sign-in.".to_string(),
            ));
        }

        if stdout.trim().is_empty() && !stderr.trim().is_empty() {
            return Err(AgentError::CliError(format!(
                "agy produced no stdout: {}",
                truncate_snippet(stderr.trim(), 500)
            )));
        }

        if !output.status.success() {
            if looks_like_auth_required(&stdout) || looks_like_auth_required(&stderr) {
                return Err(AgentError::CliError(
                    "Antigravity CLI is not authenticated for this process context. Run `agy` interactively in the same user/session and complete browser sign-in.".to_string(),
                ));
            }

            return Err(AgentError::CliError(format!(
                "agy exit {}: stdout: {}; stderr: {}",
                output.status,
                truncate_snippet(stdout.trim(), 500),
                truncate_snippet(stderr.trim(), 500)
            )));
        }

        let response = stdout.trim().to_string();
        if response.is_empty() {
            return Err(AgentError::CliError(
                "Empty Antigravity response: no content captured.".to_string(),
            ));
        }

        Ok(AntigravityExecution {
            response,
            stdout,
            stderr,
        })
    }

    fn build_invocation(&self, prompt: &str, session_id: Option<&str>) -> AntigravityInvocation {
        let mut args = vec![
            "--print".to_string(),
            prompt.to_string(),
            "--print-timeout".to_string(),
            format_go_duration(self.print_timeout),
        ];

        if let Some(ref model) = self.model {
            args.push("--model".to_string());
            args.push(model.clone());
        }

        match session_id {
            Some("") => args.push("--continue".to_string()),
            Some(session) => {
                args.push("--conversation".to_string());
                args.push(session.to_string());
            }
            None => {}
        }

        match self.permission_mode {
            AntigravityPermissionMode::Default => {}
            AntigravityPermissionMode::Sandbox => args.push("--sandbox".to_string()),
            AntigravityPermissionMode::SkipPermissions => {
                args.push("--dangerously-skip-permissions".to_string());
            }
        }

        AntigravityInvocation {
            bin: self.bin.clone(),
            args,
        }
    }
}

#[async_trait]
impl CognitiveAgent for AntigravityClient {
    async fn call(
        &self,
        prompt: &str,
        session_id: Option<&str>,
    ) -> Result<AgentResponse, AgentError> {
        let execution = self.execute(prompt, session_id).await?;
        Ok(AgentResponse {
            session_id: String::new(),
            response: execution.response,
            exchange_id: None,
            stream_events: None,
        })
    }
}

fn normalize_model(model: Option<String>) -> Option<String> {
    model.and_then(|m| {
        let trimmed = m.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn format_go_duration(duration: Duration) -> String {
    let secs = duration.as_secs().max(1);
    format!("{}s", secs)
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

fn looks_like_auth_required(input: &str) -> bool {
    let lower = input.to_ascii_lowercase();
    lower.contains("authentication required")
        || lower.contains("please sign in")
        || lower.contains("authorization code")
        || lower.contains("waiting for authentication")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn builds_basic_print_invocation() {
        let client = AntigravityClient::new(None).with_print_timeout_ms(30_000);
        let invocation = client.build_invocation("hello", None);

        assert_eq!(
            invocation.args,
            ["--print", "hello", "--print-timeout", "30s"]
        );
    }

    #[test]
    fn direct_defaults_keep_process_and_print_timeout_consistent() {
        let client = AntigravityClient::new(None);

        assert_eq!(client.timeout, client.print_timeout);
        assert_eq!(
            client.timeout,
            Duration::from_millis(DEFAULT_PRINT_TIMEOUT_MS)
        );
    }

    #[test]
    fn builds_model_resume_and_permission_flags() {
        let client = AntigravityClient::new(Some("Gemini 3.5 Flash (Low)".to_string()))
            .with_print_timeout_ms(300_000)
            .with_permission_mode(AntigravityPermissionMode::SkipPermissions);
        let invocation = client.build_invocation("hello", Some("conversation-123"));

        assert_eq!(
            invocation.args,
            [
                "--print",
                "hello",
                "--print-timeout",
                "300s",
                "--model",
                "Gemini 3.5 Flash (Low)",
                "--conversation",
                "conversation-123",
                "--dangerously-skip-permissions"
            ]
        );
    }

    #[test]
    fn empty_session_means_continue_latest() {
        let client = AntigravityClient::new(None);
        let invocation = client.build_invocation("hello", Some(""));

        assert!(invocation.args.iter().any(|arg| arg == "--continue"));
        assert!(!invocation.args.iter().any(|arg| arg == "--conversation"));
    }

    #[test]
    fn kg_permission_defaults_to_sandbox() {
        assert_eq!(
            AntigravityPermissionMode::parse("sandbox"),
            Some(AntigravityPermissionMode::Sandbox)
        );
    }

    #[test]
    fn detects_auth_required_output() {
        assert!(looks_like_auth_required(
            "Authentication required. Please visit the URL to log in:"
        ));
        assert!(looks_like_auth_required(
            "Error: Please sign in to view available models."
        ));
    }

    #[test]
    fn oauth_mentions_in_model_content_are_not_auth_failures() {
        assert!(!looks_like_auth_required(
            r#"{"summary":"Extracted OAuth project configuration details."}"#
        ));
    }

    #[tokio::test]
    async fn fake_cli_plain_stdout_response() {
        let dir = tempfile::tempdir().expect("tempdir");
        let script = dir.path().join("agy-fake");
        fs::write(&script, "#!/bin/sh\nprintf 'fake-response\\n'\n").expect("write fake agy");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script).expect("metadata").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script, perms).expect("chmod");
        }

        let client = AntigravityClient::new(None)
            .with_bin(&script)
            .with_timeout_ms(1_000);
        let response = client.call("ignored", None).await.expect("agy response");

        assert_eq!(response.response, "fake-response");
        assert!(response.session_id.is_empty());
        assert!(response.stream_events.is_none());
    }
}
