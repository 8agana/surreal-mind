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
use surrealdb::sql::Number as SurrealNumber;
use surrealdb::sql::Value as SqlValue;
use surrealdb::Value as SurrealValue;

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

        let gemini = match model_override {
            Some(custom) => GeminiClient::with_timeout_ms(custom, gemini_timeout_ms()),
            None => GeminiClient::new(),
        };
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

        let exchange_id = fetch_last_exchange_id(self.db.as_ref(), task_name.clone())
            .await?
            .ok_or_else(|| SurrealMindError::Internal {
                message: "missing exchange_id after persisted call".into(),
            })?;

        Ok(CallToolResult::structured(json!({
            "response": response.response,
            "session_id": response.session_id,
            "exchange_id": exchange_id
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
    let sql =
        "SELECT last_agent_session_id FROM tool_sessions WHERE tool_name = $tool LIMIT 1;";
    let rows: SurrealValue = db
        .query(sql)
        .bind(("tool", tool_name))
        .await?
        .take::<surrealdb::Value>(0)?;
    let rows = to_json_value(rows)?;
    Ok(first_row(&rows)
        .and_then(|v| v.get("last_agent_session_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string()))
}

async fn fetch_last_exchange_id(
    db: &Surreal<WsClient>,
    tool_name: String,
) -> Result<Option<String>> {
    let sql = "SELECT last_exchange_id FROM tool_sessions WHERE tool_name = $tool LIMIT 1;";
    let rows: SurrealValue = db
        .query(sql)
        .bind(("tool", tool_name))
        .await?
        .take::<surrealdb::Value>(0)?;
    let rows = to_json_value(rows)?;
    Ok(first_row(&rows)
        .and_then(|v| v.get("last_exchange_id"))
        .and_then(parse_record_id))
}

fn map_agent_error(err: AgentError) -> SurrealMindError {
    match err {
        AgentError::Timeout { timeout_ms } => SurrealMindError::Timeout {
            operation: "gemini".to_string(),
            timeout_ms,
        },
        AgentError::CliError(message) => SurrealMindError::Internal {
            message: format!("gemini cli error: {}", message),
        },
        AgentError::ParseError(message) => SurrealMindError::Serialization {
            message: format!("gemini parse error: {}", message),
        },
        AgentError::StdinError(message) => SurrealMindError::Internal {
            message: format!("gemini stdin error: {}", message),
        },
        AgentError::NotFound => SurrealMindError::Internal {
            message: "gemini cli not found".to_string(),
        },
    }
}

fn parse_record_id(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }

    let obj = value.as_object()?;
    if let Some(record) = obj.get("$record").and_then(|v| v.as_str()) {
        return Some(record.to_string());
    }
    if let Some(thing) = obj.get("$thing") {
        return parse_record_id(thing);
    }
    if let (Some(tb), Some(id_val)) = (obj.get("tb"), obj.get("id")) {
        let table = tb.as_str()?;
        let id = parse_record_id_value(id_val)?;
        return Some(format!("{}:{}", table, id));
    }

    obj.get("id").and_then(parse_record_id)
}

fn to_json_value(value: SurrealValue) -> Result<Value> {
    Ok(to_json_value_inner(value.into()))
}

fn to_json_value_inner(value: SqlValue) -> Value {
    match value {
        SqlValue::None | SqlValue::Null => Value::Null,
        SqlValue::Bool(value) => Value::Bool(value),
        SqlValue::Number(number) => number_to_json(number),
        SqlValue::Strand(value) => Value::String(value.to_string()),
        SqlValue::Duration(value) => Value::String(value.to_string()),
        SqlValue::Datetime(value) => Value::String(value.to_string()),
        SqlValue::Uuid(value) => Value::String(value.to_string()),
        SqlValue::Array(values) => {
            Value::Array(values.into_iter().map(to_json_value_inner).collect())
        }
        SqlValue::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, to_json_value_inner(value)))
                .collect(),
        ),
        SqlValue::Geometry(value) => Value::String(value.to_string()),
        SqlValue::Bytes(value) => Value::String(value.to_string()),
        SqlValue::Thing(value) => Value::String(value.to_string()),
        other => Value::String(other.to_string()),
    }
}

fn number_to_json(number: SurrealNumber) -> Value {
    match number {
        SurrealNumber::Int(value) => Value::Number(serde_json::Number::from(value)),
        SurrealNumber::Float(value) => serde_json::Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(value.to_string())),
        SurrealNumber::Decimal(value) => Value::String(value.to_string()),
        _ => Value::String(number.to_string()),
    }
}

fn first_row(value: &Value) -> Option<&Value> {
    match value {
        Value::Array(values) => values.first(),
        Value::Null => None,
        other => Some(other),
    }
}

fn parse_record_id_value(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    if let Some(n) = value.as_i64() {
        return Some(n.to_string());
    }
    if let Some(n) = value.as_u64() {
        return Some(n.to_string());
    }
    if let Some(uuid) = value.get("$uuid").and_then(|v| v.as_str()) {
        return Some(uuid.to_string());
    }
    if let Some(ulid) = value.get("$ulid").and_then(|v| v.as_str()) {
        return Some(ulid.to_string());
    }
    None
}
