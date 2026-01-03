use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub session_id: String,
    pub response: String,
    pub exchange_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_events: Option<Vec<crate::clients::gemini::GeminiStreamEvent>>,
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("agent timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
    #[error("cli error: {0}")]
    CliError(String),
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("stdin error: {0}")]
    StdinError(String),
    #[error("cli executable not found")]
    NotFound,
}

#[async_trait]
pub trait CognitiveAgent: Send + Sync {
    async fn call(
        &self,
        prompt: &str,
        session_id: Option<&str>,
    ) -> Result<AgentResponse, AgentError>;
}
