//! cancel_agent_job tool handler to cancel running/queued jobs

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::Deserialize;
use serde_json::{Value, json};
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Client as WsClient;

/// Parameters for the cancel_agent_job tool
#[derive(Debug, Deserialize)]
pub struct CancelAgentJobParams {
    pub job_id: String,
}

#[derive(Debug, Deserialize)]
struct JobStatusRow {
    status: String,
}

impl SurrealMindServer {
    /// Handle the cancel_agent_job tool call
    pub async fn handle_cancel_agent_job(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: CancelAgentJobParams =
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

        let result = cancel_job(self.db.as_ref(), job_id).await?;

        Ok(CallToolResult::structured(json!(result)))
    }
}

async fn cancel_job(db: &Surreal<WsClient>, job_id: String) -> Result<Value> {
    // First check current status
    let check_sql = "SELECT status FROM agent_jobs WHERE job_id = $job_id LIMIT 1;";
    let mut response = db.query(check_sql).bind(("job_id", job_id.clone())).await?;
    let rows: Vec<JobStatusRow> = response.take(0)?;

    let current_status = rows.first().ok_or_else(|| SurrealMindError::Mcp {
        message: format!("Job not found: {}", job_id),
    })?;

    // Don't cancel already completed/failed jobs
    if current_status.status == "completed" || current_status.status == "failed" {
        return Err(SurrealMindError::InvalidParams {
            message: format!("Cannot cancel job in '{}' status", current_status.status),
        });
    }
    if current_status.status == "cancelled" {
        return Ok(json!({
            "job_id": job_id,
            "previous_status": current_status.status,
            "new_status": "cancelled",
            "message": "Job already cancelled."
        }));
    }

    // Update status to cancelled
    let cancel_sql = "UPDATE agent_jobs SET status = 'cancelled', completed_at = time::now(), duration_ms = 0 WHERE job_id = $job_id;";
    db.query(cancel_sql)
        .bind(("job_id", job_id.clone()))
        .await?;

    // Note: Task abort is handled via the JoinHandle in the spawning code
    // The database update serves as the signal for cancellation

    Ok(json!({
        "job_id": job_id,
        "previous_status": current_status.status,
        "new_status": "cancelled",
        "message": "Job cancellation requested. Task will be terminated if still running."
    }))
}
