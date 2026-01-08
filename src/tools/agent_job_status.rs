//! agent_job_status tool handler to query async job status

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::Deserialize;
use serde_json::{Value, json};
use surrealdb::engine::remote::ws::Client as WsClient;
use surrealdb::sql::Value as SqlValue;
use surrealdb::Surreal;

/// Parameters for the agent_job_status tool
#[derive(Debug, Deserialize)]
pub struct AgentJobStatusParams {
    pub job_id: String,
}


impl SurrealMindServer {
    /// Handle the agent_job_status tool call
    pub async fn handle_agent_job_status(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: AgentJobStatusParams =
            serde_json::from_value(Value::Object(args)).map_err(|e| {
                SurrealMindError::InvalidParams {
                    message: format!("Invalid parameters: {}", e),
                }
            })?;

        let job_id = params.job_id.trim().to_string();
        if job_id.is_empty() {
            return Err(SurrealMindError::InvalidParams {
                message: "job_id cannot be empty".into(),
            });
        }

        let job = fetch_job_status(self.db.as_ref(), job_id).await?;

        Ok(CallToolResult::structured(json!(job)))
    }
}

async fn fetch_job_status(db: &Surreal<WsClient>, job_id: String) -> Result<Value> {
    // Query all job fields, using explicit casting to avoid SurrealDB enum serialization issues
    let sql =
        "SELECT
            job_id,
            status,
            created_at,
            started_at,
            completed_at,
            duration_ms,
            error,
            session_id,
            exchange_id as exchange_id_raw,
            metadata,
            prompt,
            task_name,
            model_override,
            cwd,
            timeout_ms,
            tool_timeout_ms,
            expose_stream
        FROM agent_jobs WHERE job_id = $job_id LIMIT 1;";

    let mut response = match db.query(sql).bind(("job_id", job_id.clone())).await {
        Ok(resp) => resp,
        Err(err) => {
            tracing::error!("call_status query failed: {}", err);
            return Err(err.into());
        }
    };

    // Use raw SQL values to handle complex types safely
    let rows: Vec<SqlValue> = match response.take(0) {
        Ok(rows) => rows,
        Err(err) => {
            tracing::error!("call_status take failed: {}", err);
            return Err(err.into());
        }
    };

    let row = rows.first().ok_or_else(|| SurrealMindError::Mcp {
        message: format!("Job not found: {}", job_id),
    })?;

    let obj = match row {
        SqlValue::Object(obj) => obj,
        _ => {
            return Err(SurrealMindError::Mcp {
                message: "Unexpected job status response shape".into(),
            })
        }
    };

    let job_id = val_to_string(obj.get("job_id")).unwrap_or_default();
    if job_id.is_empty() {
        return Err(SurrealMindError::Mcp {
            message: format!("Job not found: {}", job_id),
        });
    }

    // Safely extract exchange_id - it might be a Record type which can't deserialize to JSON
    let exchange_id = match obj.get("exchange_id_raw") {
        Some(SqlValue::None) | Some(SqlValue::Null) => None,
        Some(SqlValue::Strand(s)) => Some(s.0.clone()),
        Some(other) => Some(other.to_string()), // Fallback for Record types
        None => None,
    };

    Ok(json!({
        "job_id": job_id,
        "status": val_to_string(obj.get("status")).unwrap_or_default(),
        "created_at": val_to_string(obj.get("created_at")),
        "started_at": val_to_string(obj.get("started_at")),
        "completed_at": val_to_string(obj.get("completed_at")),
        "duration_ms": val_to_i64(obj.get("duration_ms")),
        "error": val_to_string(obj.get("error")),
        "session_id": val_to_string(obj.get("session_id")),
        "exchange_id": exchange_id,
        "metadata": obj.get("metadata").cloned(),
        "prompt": val_to_string(obj.get("prompt")),
        "task_name": val_to_string(obj.get("task_name")),
        "model_override": val_to_string(obj.get("model_override")),
        "cwd": val_to_string(obj.get("cwd")),
        "timeout_ms": val_to_i64(obj.get("timeout_ms")),
        "tool_timeout_ms": val_to_i64(obj.get("tool_timeout_ms")),
        "expose_stream": val_to_bool(obj.get("expose_stream"))
    }))
}

fn val_to_string(value: Option<&SqlValue>) -> Option<String> {
    let value = value?;
    match value {
        SqlValue::None | SqlValue::Null => None,
        SqlValue::Strand(s) => Some(s.0.clone()),
        other => Some(other.to_string()),
    }
}

fn val_to_i64(value: Option<&SqlValue>) -> Option<i64> {
    val_to_string(value).and_then(|s| s.parse::<i64>().ok())
}

fn val_to_bool(value: Option<&SqlValue>) -> Option<bool> {
    let value = value?;
    match value {
        SqlValue::Bool(b) => Some(*b),
        SqlValue::Strand(s) => s.0.parse::<bool>().ok(),
        other => other.to_string().parse::<bool>().ok(),
    }
}
