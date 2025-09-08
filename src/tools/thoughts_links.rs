//! thoughts.links tool for retrieving thought link relationships

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct ThoughtsLinksParams {
    pub thought_id: String,
}

impl SurrealMindServer {
    /// Handle the thoughts.links tool call
    pub async fn handle_thoughts_links(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: ThoughtsLinksParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::Serialization {
                message: format!("Invalid parameters: {}", e),
            })?;

        // Query for the thought's link fields
        let sql = "SELECT previous_thought_id, revises_thought, branch_from \
                   FROM thoughts \
                   WHERE meta::id(id) = $thought_id \
                   LIMIT 1";

        let mut resp = self
            .db
            .query(sql)
            .bind(("thought_id", params.thought_id.clone()))
            .await?;

        #[derive(Debug, Deserialize)]
        struct LinksRow {
            #[serde(default)]
            previous_thought_id: Option<String>,
            #[serde(default)]
            revises_thought: Option<String>,
            #[serde(default)]
            branch_from: Option<String>,
        }

        let rows: Vec<LinksRow> = resp.take(0)?;
        
        if let Some(row) = rows.into_iter().next() {
            // Thought found, return its links
            Ok(CallToolResult::structured(json!({
                "thought_id": params.thought_id,
                "links": {
                    "previous_thought": row.previous_thought_id,
                    "revises_thought": row.revises_thought,
                    "branch_from": row.branch_from,
                }
            })))
        } else {
            // Thought not found
            Ok(CallToolResult::structured(json!({
                "thought_id": params.thought_id,
                "error": "Thought not found",
                "links": null
            })))
        }
    }
}