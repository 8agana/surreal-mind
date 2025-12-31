//! list_agent_jobs tool handler to list and filter async jobs

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::Deserialize;
use serde_json::{Value, json};
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Client as WsClient;

/// Parameters for the list_agent_jobs tool
#[derive(Debug, Deserialize)]
pub struct ListAgentJobsParams {
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub status_filter: Option<String>,
    #[serde(default)]
    pub tool_name: Option<String>,
}

fn default_limit() -> u32 {
    20
}

#[derive(Debug, Deserialize)]
struct JobSummary {
    job_id: String,
    status: String,
    tool_name: String,
    created_at: String,
    completed_at: Option<String>,
    duration_ms: Option<i64>,
}

impl SurrealMindServer {
    /// Handle the list_agent_jobs tool call
    pub async fn handle_list_agent_jobs(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: ListAgentJobsParams =
            serde_json::from_value(Value::Object(args)).map_err(|e| {
                SurrealMindError::InvalidParams {
                    message: format!("Invalid parameters: {}", e),
                }
            })?;

        // Validate limit
        let limit = if params.limit == 0 || params.limit > 100 {
            20
        } else {
            params.limit
        };

        let jobs = fetch_jobs(
            self.db.as_ref(),
            limit,
            params.status_filter.as_deref(),
            params.tool_name.as_deref(),
        )
        .await?;

        Ok(CallToolResult::structured(json!({
            "jobs": jobs,
            "total": jobs.len()
        })))
    }
}

async fn fetch_jobs(
    db: &Surreal<WsClient>,
    limit: u32,
    status_filter: Option<&str>,
    tool_name_filter: Option<&str>,
) -> Result<Vec<Value>> {
    let mut sql =
        "SELECT job_id, status, tool_name, created_at, completed_at, duration_ms FROM agent_jobs"
            .to_string();
    let mut conditions = Vec::new();

    if let Some(status) = status_filter {
        conditions.push(format!("status = '{}'", status));
    }
    if let Some(tool) = tool_name_filter {
        conditions.push(format!("tool_name = '{}'", tool));
    }

    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }

    sql.push_str(" ORDER BY created_at DESC");
    sql.push_str(&format!(" LIMIT {}", limit));

    let mut response = db.query(&sql).await?;
    let rows: Vec<JobSummary> = response.take(0)?;

    Ok(rows
        .into_iter()
        .map(|job| {
            json!({
                "job_id": job.job_id,
                "status": job.status,
                "tool_name": job.tool_name,
                "created_at": job.created_at,
                "completed_at": job.completed_at,
                "duration_ms": job.duration_ms
            })
        })
        .collect())
}
