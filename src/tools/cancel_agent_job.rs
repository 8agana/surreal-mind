//! cancel_agent_job tool handler to cancel running/queued jobs

use crate::error::{Result, SurrealMindError};
use crate::registry;
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

    // Attempt immediate abort via registry (delegates and other registered jobs)
    let was_running = registry::abort_job(&job_id);

    let message = if was_running {
        "Job found and aborted immediately."
    } else {
        "Job status marked cancelled. If running via polling-based worker, will terminate on next check."
    };

    Ok(json!({
        "job_id": job_id,
        "previous_status": current_status.status,
        "new_status": "cancelled",
        "was_running_in_registry": was_running,
        "message": message
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cancel_idempotent() {
        // Calling cancel twice should be safe and idempotent
        // This test verifies the function doesn't panic and returns sensible responses

        // Verify that calling cancel on a non-existent job is safe
        // (In real usage, the job must exist in DB first)
        let job_id = "test-job-456";
        let was_running_1 = registry::abort_job(job_id);
        let was_running_2 = registry::abort_job(job_id);

        // First should return false (not registered)
        // Second should also return false (already removed)
        assert!(!was_running_1);
        assert!(!was_running_2);
    }

    #[tokio::test]
    async fn test_cancel_registered_job() {
        let job_id = "test-job-cancel-001";

        // Register a dummy job
        let handle = tokio::spawn(async { std::future::pending::<()>().await });
        registry::register_job(job_id.to_string(), handle);

        // Verify it's in the registry
        assert_eq!(registry::registry_size(), 1);

        // Call cancel (the part that looks up in registry)
        let was_aborted = registry::abort_job(job_id);

        // Should have been found and aborted
        assert!(was_aborted);
        assert_eq!(registry::registry_size(), 0);
    }

    #[tokio::test]
    async fn test_cancel_idempotent_registry() {
        let job_id = "test-job-idempotent-001";

        // Register a job
        let handle = tokio::spawn(async {});
        registry::register_job(job_id.to_string(), handle);

        // First abort - should succeed
        let first = registry::abort_job(job_id);
        assert!(first);

        // Second abort - should fail safely (job already removed)
        let second = registry::abort_job(job_id);
        assert!(!second);

        // Both cases should not panic - this verifies idempotence
    }
}
