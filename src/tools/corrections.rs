//! corrections tool: list correction_events with optional target filter

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;

#[derive(Debug, serde::Deserialize)]
pub struct CorrectionsParams {
    pub target_id: Option<String>,
    #[serde(default = "CorrectionsParams::default_limit")]
    pub limit: i64,
}

impl CorrectionsParams {
    fn default_limit() -> i64 {
        10
    }
}

impl SurrealMindServer {
    pub async fn handle_corrections(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: CorrectionsParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::InvalidParams {
                message: format!("Invalid parameters: {}", e),
            })?;

        let mut q = String::from(
            "SELECT meta::id(id) as id, target_id, target_table, timestamp, reasoning, sources, initiated_by, corrects_previous, spawned_by, previous_state, new_state \
             FROM correction_events",
        );

        if params.target_id.is_some() {
            q.push_str(" WHERE target_id = $target_id");
        }
        q.push_str(" ORDER BY timestamp DESC LIMIT $limit");

        let mut query = self.db.query(q).bind(("limit", params.limit));
        if let Some(tid) = params.target_id.clone() {
            query = query.bind(("target_id", tid));
        }

        let rows: Vec<serde_json::Value> = query.await?.take(0)?;

        let response = json!({
            "success": true,
            "count": rows.len(),
            "events": rows
        });

        Ok(CallToolResult::structured(response))
    }
}
