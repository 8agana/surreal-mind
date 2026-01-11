//! rethink tool handler for revision and correction operations

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;
use surrealdb::sql::Datetime;

/// Parameters for the rethink tool
#[derive(Debug, serde::Deserialize)]
pub struct RethinkParams {
    pub target_id: String,
    pub mode: String,
    pub mark_type: String,
    pub marked_for: String,
    pub note: String,
}

impl SurrealMindServer {
    /// Handle the rethink tool call (currently only mark mode supported)
    pub async fn handle_rethink(&self, request: CallToolRequestParam) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: RethinkParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::InvalidParams {
                message: format!("Invalid parameters: {}", e),
            })?;

        // Validate mode
        if params.mode != "mark" {
            return Err(SurrealMindError::Validation {
                message: format!(
                    "Unsupported mode: {}. Only 'mark' is currently supported.",
                    params.mode
                ),
            });
        }

        // Validate mark_type
        let valid_types = ["correction", "research", "enrich", "expand"];
        if !valid_types.contains(&params.mark_type.as_str()) {
            return Err(SurrealMindError::Validation {
                message: format!(
                    "Invalid mark_type: {}. Must be one of: {:?}",
                    params.mark_type, valid_types
                ),
            });
        }

        // Validate marked_for
        let valid_targets = ["cc", "sam", "gemini", "dt", "gem"];
        if !valid_targets.contains(&params.marked_for.as_str()) {
            return Err(SurrealMindError::Validation {
                message: format!(
                    "Invalid marked_for: {}. Must be one of: {:?}",
                    params.marked_for, valid_targets
                ),
            });
        }

        // Validate target_id format (should be table:id)
        if !params.target_id.contains(':') {
            return Err(SurrealMindError::Validation {
                message: "Invalid target_id format. Expected table:id".into(),
            });
        }

        let parts: Vec<&str> = params.target_id.split(':').collect();
        if parts.len() != 2 {
            return Err(SurrealMindError::Validation {
                message: "Invalid target_id format. Expected table:id".into(),
            });
        }

        let table = parts[0];
        let valid_tables = ["thoughts", "kg_entities", "kg_observations"];

        // Allow entity: and observation: prefixes for kg_entities and kg_observations
        let table_name = match table {
            "entity" => "kg_entities",
            "observation" => "kg_observations",
            "thoughts" => "thoughts",
            _ => {
                if !valid_tables.contains(&table) {
                    return Err(SurrealMindError::Validation {
                        message: format!(
                            "Invalid table: {}. Must be one of: {:?}",
                            table,
                            ["thoughts", "entity", "observation"]
                        ),
                    });
                }
                table
            }
        };

        // Check if the record exists
        let exists: Option<serde_json::Value> = self
            .db
            .query(format!("SELECT id FROM {} WHERE id = $id", table_name))
            .bind(("id", params.target_id.clone()))
            .await?
            .take(0)?;

        if exists.is_none() {
            return Err(SurrealMindError::Validation {
                message: format!("Record not found: {}", params.target_id),
            });
        }

        // Update the record with mark fields
        let marked_at = Datetime::default(); // current time
        let marked_by = "cc"; // hardcoded for now

        self.db
            .query(format!(
                "UPDATE {} SET marked_for = $marked_for, mark_type = $mark_type, mark_note = $note, marked_at = $marked_at, marked_by = $marked_by WHERE id = $id",
                table_name
            ))
            .bind(("id", params.target_id.clone()))
            .bind(("marked_for", params.marked_for.clone()))
            .bind(("mark_type", params.mark_type.clone()))
            .bind(("note", params.note.clone()))
            .bind(("marked_at", marked_at.clone()))
            .bind(("marked_by", marked_by))
            .await?;

        let response = json!({
            "success": true,
            "marked": {
                "id": params.target_id,
                "type": params.mark_type,
                "for": params.marked_for,
                "note": params.note,
                "marked_at": marked_at,
                "marked_by": marked_by
            }
        });

        Ok(CallToolResult::structured(response))
    }
}
