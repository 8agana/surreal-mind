//! rethink tool handler for revision and correction operations

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;

/// Parameters for the rethink tool
#[derive(Debug, serde::Deserialize)]
pub struct RethinkParams {
    pub target_id: String,
    pub mode: String,
    pub mark_type: Option<String>,
    pub marked_for: Option<String>,
    pub note: Option<String>,
    pub reasoning: Option<String>,
    pub sources: Option<Vec<String>>,
    pub cascade: Option<bool>,
}

impl SurrealMindServer {
    /// Handle the rethink tool call (mark + correct modes)
    pub async fn handle_rethink(&self, request: CallToolRequestParam) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: RethinkParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::InvalidParams {
                message: format!("Invalid parameters: {}", e),
            })?;

        let mode = params.mode.as_str();
        if mode != "mark" && mode != "correct" {
            return Err(SurrealMindError::Validation {
                message: format!(
                    "Unsupported mode: {}. Use 'mark' or 'correct'.",
                    params.mode
                ),
            });
        }

        // Mode-specific validation
        if mode == "mark" {
            let valid_types = ["correction", "research", "enrich", "expand"];
            let mark_type =
                params
                    .mark_type
                    .as_deref()
                    .ok_or_else(|| SurrealMindError::Validation {
                        message: "mark_type is required for mark mode".into(),
                    })?;
            if !valid_types.contains(&mark_type) {
                return Err(SurrealMindError::Validation {
                    message: format!(
                        "Invalid mark_type: {}. Must be one of: {:?}",
                        mark_type, valid_types
                    ),
                });
            }

            let valid_targets = ["cc", "sam", "gemini", "dt", "gem"];
            let marked_for =
                params
                    .marked_for
                    .as_deref()
                    .ok_or_else(|| SurrealMindError::Validation {
                        message: "marked_for is required for mark mode".into(),
                    })?;
            if !valid_targets.contains(&marked_for) {
                return Err(SurrealMindError::Validation {
                    message: format!(
                        "Invalid marked_for: {}. Must be one of: {:?}",
                        marked_for, valid_targets
                    ),
                });
            }

            if params.note.as_deref().unwrap_or("").is_empty() {
                return Err(SurrealMindError::Validation {
                    message: "note is required for mark mode".into(),
                });
            }
        } else if mode == "correct" {
            if params.reasoning.as_deref().unwrap_or("").is_empty() {
                return Err(SurrealMindError::Validation {
                    message: "reasoning is required for correct mode".into(),
                });
            }
            let sources = params
                .sources
                .as_ref()
                .ok_or_else(|| SurrealMindError::Validation {
                    message: "sources is required for correct mode".into(),
                })?;
            if sources.is_empty() {
                return Err(SurrealMindError::Validation {
                    message: "sources cannot be empty".into(),
                });
            }
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

        // Extract ID part from "table:id" format and clone it for binding
        let id_part = parts[1].to_string();

        // Check if the record exists using type::thing(), but avoid deserializing record IDs.
        // Use a scalar RETURN count(...) so the SDK deserializes directly into i64 instead of an object.
        let count: Option<i64> = self
            .db
            .query(format!(
                "RETURN count((SELECT * FROM {} WHERE id = type::thing('{}', $id)))",
                table_name, table_name
            ))
            .bind(("id", id_part.clone()))
            .await?
            .take(0)?;

        if count.unwrap_or(0) == 0 {
            return Err(SurrealMindError::Validation {
                message: format!("Record not found: {}", params.target_id),
            });
        }

        if mode == "mark" {
            // Update the record with mark fields
            // Use RETURN NONE to avoid SurrealDB SDK datetime serialization issues
            self.db
                .query(format!(
                    "UPDATE {} SET marked_for = $marked_for, mark_type = $mark_type, mark_note = $note, marked_at = time::now(), marked_by = $marked_by WHERE id = type::thing('{}', $id) RETURN NONE",
                    table_name, table_name
                ))
                .bind(("id", id_part))
                .bind(("marked_for", params.marked_for.clone().unwrap()))
                .bind(("mark_type", params.mark_type.clone().unwrap()))
                .bind(("note", params.note.clone().unwrap()))
                .bind(("marked_by", "cc"))
                .await?;

            let response = json!({
                "success": true,
                "marked": {
                    "id": params.target_id,
                    "type": params.mark_type,
                    "for": params.marked_for,
                    "note": params.note,
                    "marked_by": "cc"
                }
            });

            return Ok(CallToolResult::structured(response));
        }

        // --- correct mode ---
        // 1) Fetch previous state
        let previous: Vec<serde_json::Value> = self
            .db
            .query(format!(
                "SELECT meta::id(id) as id, meta::tb(id) as table, * FROM {} WHERE id = type::thing('{}', $id) LIMIT 1",
                table_name, table_name
            ))
            .bind(("id", id_part.clone()))
            .await?
            .take(0)?;

        let previous_state =
            previous
                .first()
                .cloned()
                .ok_or_else(|| SurrealMindError::Validation {
                    message: format!("Record not found: {}", params.target_id),
                })?;

        // For now, new_state is identical (no field edits supplied); this establishes provenance and clears the mark.
        let new_state = previous_state.clone();

        // 2) Create CorrectionEvent with provenance
        let correction: Vec<serde_json::Value> = self
            .db
            .query(
                "CREATE correction_events SET \
                 target_id = $target_id, \
                 target_table = $target_table, \
                 previous_state = $previous_state, \
                 new_state = $new_state, \
                 initiated_by = $initiated_by, \
                 reasoning = $reasoning, \
                 sources = $sources, \
                 verification_status = 'auto_applied', \
                 corrects_previous = NONE, \
                 spawned_by = NONE \
                 RETURN AFTER",
            )
            .bind(("target_id", params.target_id.clone()))
            .bind(("target_table", table_name.to_string()))
            .bind(("previous_state", previous_state.clone()))
            .bind(("new_state", new_state.clone()))
            .bind(("initiated_by", "cc"))
            .bind(("reasoning", params.reasoning.clone().unwrap()))
            .bind(("sources", params.sources.clone().unwrap()))
            .await?
            .take(0)?;

        let correction_event = correction.first().cloned().unwrap_or_else(|| json!({}));

        // 3) Clear mark fields on target
        self.db
            .query(format!(
                "UPDATE {} SET marked_for = NONE, mark_type = NONE, mark_note = NONE, marked_at = NONE, marked_by = NONE WHERE id = type::thing('{}', $id) RETURN NONE",
                table_name, table_name
            ))
            .bind(("id", id_part.clone()))
            .await?;

        // 4) Optional cascade: flag derivatives for review (simple heuristic on source_thought_ids)
        let mut derivatives_flagged = 0_i64;
        if params.cascade.unwrap_or(false) && table_name == "thoughts" {
            let cascade_note = format!("Cascade from correction of {}", params.target_id);
            let mark_query = "UPDATE kg_entities SET marked_for = 'cc', mark_type = 'correction', mark_note = $note, marked_at = time::now(), marked_by = 'cc' WHERE array::contains(source_thought_ids, $thought_id) RETURN NONE";
            let _ = self
                .db
                .query(mark_query)
                .bind(("note", cascade_note.clone()))
                .bind(("thought_id", params.target_id.clone()))
                .await?;

            let obs_mark_query = "UPDATE kg_observations SET marked_for = 'cc', mark_type = 'correction', mark_note = $note, marked_at = time::now(), marked_by = 'cc' WHERE array::contains(source_thought_ids, $thought_id) RETURN NONE";
            let _ = self
                .db
                .query(obs_mark_query)
                .bind(("note", cascade_note))
                .bind(("thought_id", params.target_id.clone()))
                .await?;

            // Count how many were marked
            let cascade_count: Option<i64> = self
                .db
                .query("RETURN count((SELECT id FROM kg_entities WHERE marked_for = 'cc' AND mark_note CONTAINS 'Cascade from correction of')) + count((SELECT id FROM kg_observations WHERE marked_for = 'cc' AND mark_note CONTAINS 'Cascade from correction of'))")
                .await?
                .take(0)?;
            derivatives_flagged = cascade_count.unwrap_or(0);
        }

        let response = json!({
            "success": true,
            "correction": {
                "id": correction_event.get("id").cloned().unwrap_or(json!(null)),
                "target_id": params.target_id,
                "previous_state": previous_state,
                "new_state": new_state,
                "reasoning": params.reasoning,
                "sources": params.sources,
                "initiated_by": "cc"
            },
            "derivatives_flagged": derivatives_flagged
        });

        Ok(CallToolResult::structured(response))
    }
}
