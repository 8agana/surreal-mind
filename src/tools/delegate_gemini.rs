//! delegate_gemini tool handler to call Gemini CLI with persistence

use crate::clients::{AgentError, GeminiClient, PersistedAgent};
use crate::clients::traits::CognitiveAgent;
use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::Deserialize;
use serde_json::{json, Value};
use surrealdb::engine::remote::ws::Client as WsClient;
use surrealdb::Surreal;

const DEFAULT_MODEL: &str = "gemini-2.5-pro";
const DEFAULT_TIMEOUT_MS: u64 = 60_000;

/// Parameters for the delegate_gemini tool
#[derive(Debug, Deserialize)]
pub struct DelegateGeminiParams {
    pub prompt: String,
    #[serde(default)]
    pub task_name: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// Working directory for the Gemini CLI subprocess
    #[serde(default)]
    pub cwd: Option<String>,
    /// Timeout in milliseconds (overrides GEMINI_TIMEOUT_MS env var)
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct SessionResult {
    #[serde(default)]
    last_agent_session_id: Option<String>,
}

impl SurrealMindServer {
    /// Handle the delegate_gemini tool call
    pub async fn handle_delegate_gemini(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: DelegateGeminiParams =
            serde_json::from_value(Value::Object(args)).map_err(|e| SurrealMindError::InvalidParams {
                message: format!("Invalid parameters: {}", e),
            })?;

        let prompt = params.prompt.trim().to_string();
        if prompt.is_empty() {
            return Err(SurrealMindError::InvalidParams {
                message: "prompt cannot be empty".into(),
            });
        }

        let task_name = normalize_optional_string(params.task_name)
            .unwrap_or_else(|| "delegate_gemini".to_string());
        let model_override = normalize_optional_string(params.model);
        let model = model_override
            .clone()
            .unwrap_or_else(|| default_model_name());

        let resume_session = fetch_last_session_id(self.db.as_ref(), task_name.clone()).await?;
        let cwd = normalize_optional_string(params.cwd);
        let timeout = params.timeout_ms.unwrap_or_else(gemini_timeout_ms);

        let mut gemini = match model_override {
            Some(custom) => GeminiClient::with_timeout_ms(custom, timeout),
            None => GeminiClient::with_timeout_ms(default_model_name(), timeout),
        };
        if let Some(ref dir) = cwd {
            gemini = gemini.with_cwd(dir);
        }
        let agent = PersistedAgent::new(
            gemini,
            self.db.clone(),
            "gemini",
            model.clone(),
            task_name.clone(),
        );

        let response = agent
            .call(&prompt, resume_session.as_deref())
            .await
            .map_err(map_agent_error)?;

        Ok(CallToolResult::structured(json!({
            "response": response.response,
            "session_id": response.session_id,
            "exchange_id": response.exchange_id
        })))
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn default_model_name() -> String {
    std::env::var("GEMINI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string())
}

fn gemini_timeout_ms() -> u64 {
    std::env::var("GEMINI_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TIMEOUT_MS)
}

async fn fetch_last_session_id(
    db: &Surreal<WsClient>,
    tool_name: String,
) -> Result<Option<String>> {
    let sql = "SELECT last_agent_session_id FROM tool_sessions WHERE tool_name = $tool LIMIT 1;";
    let rows: Vec<SessionResult> = db
        .query(sql)
        .bind(("tool", tool_name))
        .await?
        .take::<Vec<SessionResult>>(0)?;
    Ok(rows
        .first()
        .and_then(|row| row.last_agent_session_id.clone()))
}

fn map_agent_error(err: AgentError) -> SurrealMindError {
    match err {
        AgentError::Timeout { timeout_ms } => SurrealMindError::Timeout {
            operation: "delegate_gemini".to_string(),
            timeout_ms,
        },
        AgentError::CliError(message) => SurrealMindError::Internal {
            message: format!("delegate_gemini failed: {}", message),
        },
        AgentError::ParseError(message) => SurrealMindError::Serialization {
            message: format!("delegate_gemini parse error: {}", message),
        },
        AgentError::StdinError(message) => SurrealMindError::Internal {
            message: format!("delegate_gemini stdin error: {}", message),
        },
        AgentError::NotFound => SurrealMindError::Internal {
            message: "delegate_gemini failed: gemini cli not found".to_string(),
        },
    }
}
