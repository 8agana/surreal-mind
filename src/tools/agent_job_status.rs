//! agent_job_status tool handler to query async job status

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use serde::Deserialize;
use serde_json::{Value, json};
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Client as WsClient;

/// Parameters for the agent_job_status tool
#[derive(Debug, Deserialize)]
pub struct AgentJobStatusParams {
    pub job_id: String,
}

impl SurrealMindServer {
    /// Handle the agent_job_status tool call
    pub async fn handle_agent_job_status(
        &self,
        request: CallToolRequestParams,
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
    // Query all job fields that exist in the schema.
    // For exchange_id (Record<agent_exchanges>), use IF THEN ELSE to safely convert to string
    // when the value exists, or return null when it's NONE.
    let sql = "SELECT
            meta::id(id) as id,
            job_id,
            status,
            type::string(created_at) as ts_created,
            IF started_at != NONE THEN type::string(started_at) ELSE null END as ts_started,
            IF completed_at != NONE THEN type::string(completed_at) ELSE null END as ts_completed,
            duration_ms,
            error,
            session_id,
            IF exchange_id != NONE THEN type::string(exchange_id) ELSE null END as exchange_id,
            metadata,
            prompt,
            task_name,
            model_override,
            cwd,
            timeout_ms
        FROM agent_jobs WHERE job_id = $job_id LIMIT 1;";

    let mut response = match db.query(sql).bind(("job_id", job_id.clone())).await {
        Ok(resp) => resp,
        Err(err) => {
            tracing::error!("agent_job_status query failed: {}", err);
            return Err(err.into());
        }
    };

    let raw_rows: Vec<serde_json::Value> = match response.take(0) {
        Ok(rows) => rows,
        Err(err) => {
            tracing::error!("agent_job_status take failed: {}", err);
            return Err(err.into());
        }
    };

    let row = raw_rows.first().ok_or_else(|| SurrealMindError::Mcp {
        message: format!("Job not found: {}", job_id),
    })?;

    let job_id_val = row
        .get("job_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if job_id_val.is_empty() {
        return Err(SurrealMindError::Mcp {
            message: format!("Job not found: {}", job_id),
        });
    }

    let mut metadata: Option<Value> = row.get("metadata").cloned();
    let mut response = extract_response_from_metadata(&metadata);

    let exchange_id = row
        .get("exchange_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if response.is_none()
        && let Some(ref eid) = exchange_id
        && let Some((exchange_response, exchange_metadata)) =
            fetch_exchange_response(db, eid.as_str()).await?
    {
        response = exchange_response;
        if metadata_is_empty(&metadata) {
            metadata = exchange_metadata;
        }
    }

    Ok(json!({
        "job_id": job_id_val,
        "status": row.get("status").and_then(|v| v.as_str()).unwrap_or(""),
        "created_at": row.get("ts_created"),
        "started_at": row.get("ts_started"),
        "completed_at": row.get("ts_completed"),
        "duration_ms": row.get("duration_ms"),
        "error": row.get("error"),
        "session_id": row.get("session_id"),
        "exchange_id": exchange_id,
        "metadata": metadata,
        "response": response,
        "prompt": row.get("prompt"),
        "task_name": row.get("task_name"),
        "model_override": row.get("model_override"),
        "cwd": row.get("cwd"),
        "timeout_ms": row.get("timeout_ms")
    }))
}

fn extract_response_from_metadata(metadata: &Option<Value>) -> Option<String> {
    match metadata {
        Some(Value::Object(map)) => map
            .get("response")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string()),
        Some(Value::Null) => None,
        _ => None,
    }
}

fn metadata_is_empty(metadata: &Option<Value>) -> bool {
    match metadata {
        None => true,
        Some(Value::Null) => true,
        Some(Value::Object(map)) => map.is_empty(),
        _ => false,
    }
}

async fn fetch_exchange_response(
    db: &Surreal<WsClient>,
    exchange_id: &str,
) -> Result<Option<(Option<String>, Option<Value>)>> {
    let sql = "SELECT response, metadata FROM agent_exchanges WHERE id = type::record($exchange_id) LIMIT 1;";
    let mut response = db
        .query(sql)
        .bind(("exchange_id", exchange_id.to_string()))
        .await?;
    let rows: Vec<serde_json::Value> = response.take(0)?;
    Ok(rows.first().map(|row| {
        let resp = row
            .get("response")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let meta = row.get("metadata").cloned();
        (resp, meta)
    }))
}
