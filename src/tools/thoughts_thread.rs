//! thoughts.thread tool for retrieving ordered thought threads by session

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct ThoughtsThreadParams {
    pub session_id: String,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_usize_forgiving"
    )]
    pub limit: Option<usize>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_usize_forgiving"
    )]
    pub offset: Option<usize>,
    #[serde(default)]
    pub order: Option<String>, // "asc" | "desc"
}

#[derive(Debug, Serialize, Deserialize)]
struct ThreadRow {
    #[serde(deserialize_with = "crate::server::deserialize_thing_to_string")]
    id: String,
    created_at: surrealdb::sql::Datetime,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chain_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_thought_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    revises_thought: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence: Option<f32>,
}

impl SurrealMindServer {
    /// Handle the thoughts.thread tool call
    pub async fn handle_thoughts_thread(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: ThoughtsThreadParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::Serialization {
                message: format!("Invalid parameters: {}", e),
            })?;

        let limit = params.limit.unwrap_or(100).clamp(1, 500);
        let offset = params.offset.unwrap_or(0);
        
        // Default to ascending for thread view
        let order_dir = match params.order.as_deref() {
            Some("desc") => "DESC",
            _ => "ASC", // Default to ASC for natural thread order
        };

        let sql = format!(
            "SELECT meta::id(id) as id, created_at, content, session_id, chain_id, \
             previous_thought_id, revises_thought, branch_from, confidence \
             FROM thoughts \
             WHERE session_id = $session_id \
             ORDER BY created_at {} \
             LIMIT $limit START $offset",
            order_dir
        );

        let mut resp = self
            .db
            .query(&sql)
            .bind(("session_id", params.session_id.clone()))
            .bind(("limit", limit))
            .bind(("offset", offset))
            .await?;

        let rows: Vec<ThreadRow> = resp.take(0)?;

        Ok(CallToolResult::structured(json!({
            "session_id": params.session_id,
            "total": rows.len(),
            "limit": limit,
            "offset": offset,
            "order": order_dir.to_lowercase(),
            "thoughts": rows
        })))
    }
}