//! agent_job_status tool handler to query async job status

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
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
    // Query all job fields. NONE values deserialize naturally to Option::None
    let sql = "SELECT
            job_id,
            status,
            created_at,
            started_at,
            completed_at,
            duration_ms,
            error,
            session_id,
            exchange_id,
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
            tracing::error!("agent_job_status query failed: {}", err);
            return Err(err.into());
        }
    };

    // Define a struct to deserialize the job row
    // Option<T> fields allow NONE values to deserialize as None, fixing the issue
    // where <string> casting would fail on NONE values for running jobs
    #[derive(Deserialize)]
    struct JobRow {
        job_id: String,
        status: Option<String>,
        created_at: Option<String>,
        started_at: Option<String>,
        completed_at: Option<String>,
        duration_ms: Option<i64>,
        error: Option<String>,
        session_id: Option<String>,
        exchange_id: Option<String>,
        metadata: Option<serde_json::Value>,
        prompt: Option<String>,
        task_name: Option<String>,
        model_override: Option<String>,
        cwd: Option<String>,
        timeout_ms: Option<i64>,
        tool_timeout_ms: Option<i64>,
        expose_stream: Option<bool>,
    }

    let rows: Vec<JobRow> = match response.take(0) {
        Ok(rows) => rows,
        Err(err) => {
            tracing::error!("agent_job_status take failed: {}", err);
            return Err(err.into());
        }
    };

    let row = rows.first().ok_or_else(|| SurrealMindError::Mcp {
        message: format!("Job not found: {}", job_id),
    })?;

    if row.job_id.is_empty() {
        return Err(SurrealMindError::Mcp {
            message: format!("Job not found: {}", job_id),
        });
    }

    Ok(json!({
        "job_id": row.job_id,
        "status": row.status.clone().unwrap_or_default(),
        "created_at": row.created_at,
        "started_at": row.started_at,
        "completed_at": row.completed_at,
        "duration_ms": row.duration_ms,
        "error": row.error,
        "session_id": row.session_id,
        "exchange_id": row.exchange_id,
        "metadata": row.metadata,
        "prompt": row.prompt,
        "task_name": row.task_name,
        "model_override": row.model_override,
        "cwd": row.cwd,
        "timeout_ms": row.timeout_ms,
        "tool_timeout_ms": row.tool_timeout_ms,
        "expose_stream": row.expose_stream
    }))
}
