//! Journal tool handler for research thread management
//!
//! Provides structured research threading on top of the existing KG.
//! Research threads are entities (entity_type: "research_thread"),
//! journal entries are observations linked to those threads.

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use serde_json::json;

#[derive(Debug, serde::Deserialize)]
pub struct JournalParams {
    pub mode: String,
    pub thread: Option<String>,
    pub content: Option<String>,
    pub observation_type: Option<String>,
    pub author: Option<String>,
    pub tags: Option<Vec<String>>,
    pub confidence: Option<f64>,
    pub thread_status: Option<String>,
    pub author_filter: Option<String>,
    pub type_filter: Option<String>,
    pub status_filter: Option<String>,
    pub limit: Option<u32>,
}

const VALID_OBS_TYPES: [&str; 6] = [
    "question",
    "hypothesis",
    "evidence",
    "reflection",
    "dead_end",
    "follow_up",
];
const VALID_AUTHORS: [&str; 4] = ["cc", "gem", "vibe", "dt"];
const VALID_STATUSES: [&str; 4] = ["open", "pursuing", "resolved", "abandoned"];

impl SurrealMindServer {
    /// Handle the journal tool call — routes to mode-specific handlers
    pub async fn handle_journal(&self, request: CallToolRequestParams) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: JournalParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::InvalidParams {
                message: format!("Invalid parameters: {}", e),
            })?;

        match params.mode.as_str() {
            "write" => self.journal_write(&params).await,
            "read" => self.journal_read(&params).await,
            "threads" => self.journal_threads(&params).await,
            "status" => self.journal_status(&params).await,
            _ => Err(SurrealMindError::Validation {
                message: format!(
                    "Unsupported mode: {}. Use 'write', 'read', 'threads', or 'status'.",
                    params.mode
                ),
            }),
        }
    }

    /// Write mode: create/find a thread and add a journal entry as an observation
    async fn journal_write(&self, params: &JournalParams) -> Result<CallToolResult> {
        let thread_name = params
            .thread
            .as_deref()
            .ok_or_else(|| SurrealMindError::Validation {
                message: "thread is required for write mode".into(),
            })?;
        let content = params
            .content
            .as_deref()
            .ok_or_else(|| SurrealMindError::Validation {
                message: "content is required for write mode".into(),
            })?;
        let obs_type =
            params
                .observation_type
                .as_deref()
                .ok_or_else(|| SurrealMindError::Validation {
                    message: "observation_type is required for write mode".into(),
                })?;

        if !VALID_OBS_TYPES.contains(&obs_type) {
            return Err(SurrealMindError::Validation {
                message: format!(
                    "Invalid observation_type: {}. Must be one of: {:?}",
                    obs_type, VALID_OBS_TYPES
                ),
            });
        }

        let author = params.author.as_deref().unwrap_or("cc");
        if !VALID_AUTHORS.contains(&author) {
            return Err(SurrealMindError::Validation {
                message: format!(
                    "Invalid author: {}. Must be one of: {:?}",
                    author, VALID_AUTHORS
                ),
            });
        }

        let confidence = params.confidence.unwrap_or(0.5).clamp(0.0, 1.0);
        let tags = params.tags.clone().unwrap_or_default();

        // 1. Find or create thread entity
        let existing: Vec<serde_json::Value> = self
            .db
            .query("SELECT meta::id(id) as id, thread_status, type::string(created_at) as created_at FROM kg_entities WHERE name = $name AND data.entity_type = 'research_thread' LIMIT 1")
            .bind(("name", thread_name.to_string()))
            .await?
            .take(0)?;

        let (thread_id, thread_created) = if let Some(row) = existing.first() {
            let id = row
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            (id, false)
        } else {
            // Create new thread entity
            let created: Vec<serde_json::Value> = self
                .db
                .query(
                    "CREATE kg_entities SET \
                     created_at = time::now(), \
                     name = $name, \
                     entity_type = 'research_thread', \
                     thread_status = 'open', \
                     data = { entity_type: 'research_thread', description: $name } \
                     RETURN meta::id(id) as id",
                )
                .bind(("name", thread_name.to_string()))
                .await?
                .take(0)?;
            let id = created
                .first()
                .and_then(|v| v.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Auto-embed the new thread entity
            if !id.is_empty() {
                let data = json!({"entity_type": "research_thread", "description": thread_name});
                if let Err(e) = self
                    .ensure_kg_embedding("kg_entities", &id, thread_name, &data)
                    .await
                {
                    tracing::warn!("journal: failed to embed thread entity {}: {}", id, e);
                }
            }

            (id, true)
        };

        // 2. Create observation (journal entry) on the thread
        let thread_ref = format!("kg_entities:{}", thread_id);
        let obs_data = json!({
            "observation_type": obs_type,
            "author": author,
            "thread_id": thread_ref,
            "description": content,
        });

        let created_obs: Vec<serde_json::Value> = self
            .db
            .query(
                "CREATE kg_observations SET \
                 created_at = time::now(), \
                 name = $name, \
                 data = $data, \
                 confidence = $conf, \
                 tags = $tags, \
                 author = $author, \
                 observation_type = $obs_type \
                 RETURN meta::id(id) as id, type::string(created_at) as created_at",
            )
            .bind(("name", thread_name.to_string()))
            .bind(("data", obs_data.clone()))
            .bind(("conf", confidence))
            .bind(("tags", tags.clone()))
            .bind(("author", author.to_string()))
            .bind(("obs_type", obs_type.to_string()))
            .await?
            .take(0)?;

        let obs_id = created_obs
            .first()
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Auto-embed the observation
        if !obs_id.is_empty() {
            let embed_name = format!("{} - {}", thread_name, obs_type);
            if let Err(e) = self
                .ensure_kg_embedding("kg_observations", &obs_id, &embed_name, &obs_data)
                .await
            {
                tracing::warn!("journal: failed to embed observation {}: {}", obs_id, e);
            }
        }

        // 3. Create edge linking observation to thread for graph traversal
        self.db
            .query(
                "CREATE kg_edges SET \
                 created_at = time::now(), \
                 source = type::record('kg_observations', $obs_id), \
                 target = type::record('kg_entities', $thread_id), \
                 rel_type = 'journal_entry_of', \
                 data = {} \
                 RETURN NONE",
            )
            .bind(("obs_id", obs_id.clone()))
            .bind(("thread_id", thread_id.clone()))
            .await?;

        let response = json!({
            "success": true,
            "thread": {
                "id": thread_ref,
                "name": thread_name,
                "created": thread_created,
            },
            "entry": {
                "id": format!("kg_observations:{}", obs_id),
                "observation_type": obs_type,
                "author": author,
                "content": content,
                "confidence": confidence,
                "tags": tags,
            }
        });

        Ok(CallToolResult::structured(response))
    }

    /// Read mode: return observations for a thread, chronologically
    async fn journal_read(&self, params: &JournalParams) -> Result<CallToolResult> {
        let thread_name = params
            .thread
            .as_deref()
            .ok_or_else(|| SurrealMindError::Validation {
                message: "thread is required for read mode".into(),
            })?;
        let limit = params.limit.unwrap_or(20).min(100);

        // Find thread entity
        let thread: Vec<serde_json::Value> = self
            .db
            .query(
                "SELECT meta::id(id) as id, name, thread_status, type::string(created_at) as created_at \
                 FROM kg_entities WHERE name = $name AND data.entity_type = 'research_thread' LIMIT 1",
            )
            .bind(("name", thread_name.to_string()))
            .await?
            .take(0)?;

        let thread_row = thread.first().ok_or_else(|| SurrealMindError::Validation {
            message: format!("Thread not found: {}", thread_name),
        })?;

        // Fetch observations — no ORDER BY in SQL (surrealdb crate bug: ORDER BY created_at
        // causes empty results). Sort in Rust instead.
        let mut sql = String::from(
            "SELECT meta::id(id) as id, name, data, confidence, tags, author, \
             observation_type, type::string(created_at) as created_at \
             FROM kg_observations WHERE name = $name",
        );

        if params.author_filter.is_some() {
            sql.push_str(" AND author = $author");
        }
        if params.type_filter.is_some() {
            sql.push_str(" AND observation_type = $type_filter");
        }

        // No ORDER BY — will sort in Rust. Use generous limit.
        sql.push_str(&format!(" LIMIT {}", limit));

        let mut q = self.db.query(&sql).bind(("name", thread_name.to_string()));
        if let Some(ref author) = params.author_filter {
            q = q.bind(("author", author.clone()));
        }
        if let Some(ref type_f) = params.type_filter {
            q = q.bind(("type_filter", type_f.clone()));
        }

        let mut entries: Vec<serde_json::Value> = q.await?.take(0)?;

        // Sort by created_at ASC in Rust
        entries.sort_by(|a, b| {
            let a_ts = a.get("created_at").and_then(|v| v.as_str()).unwrap_or("");
            let b_ts = b.get("created_at").and_then(|v| v.as_str()).unwrap_or("");
            a_ts.cmp(b_ts)
        });

        let response = json!({
            "thread": thread_row,
            "entries": entries,
            "count": entries.len(),
        });

        Ok(CallToolResult::structured(response))
    }

    /// Threads mode: dashboard view of all research threads
    async fn journal_threads(&self, params: &JournalParams) -> Result<CallToolResult> {
        let mut sql = String::from(
            "SELECT meta::id(id) as id, name, thread_status, type::string(created_at) as created_at \
             FROM kg_entities WHERE data.entity_type = 'research_thread'",
        );

        if params.status_filter.is_some() {
            sql.push_str(" AND thread_status = $status");
        }

        sql.push_str(" ORDER BY created_at DESC");

        let mut q = self.db.query(&sql);
        if let Some(ref status) = params.status_filter {
            q = q.bind(("status", status.clone()));
        }

        let threads: Vec<serde_json::Value> = q.await?.take(0)?;

        // Enrich each thread with entry count and last activity
        // Use name-based lookup to avoid record-link type coercion issues
        let mut enriched_threads = Vec::new();
        for thread in &threads {
            let tname = thread.get("name").and_then(|v| v.as_str()).unwrap_or("");

            let count_result: Option<i64> = self
                .db
                .query("RETURN count((SELECT id FROM kg_observations WHERE name = $tname AND observation_type IS NOT NONE))")
                .bind(("tname", tname.to_string()))
                .await?
                .take(0)?;

            let last_entry: Vec<serde_json::Value> = self
                .db
                .query(
                    "SELECT author, observation_type, type::string(created_at) as created_at \
                     FROM kg_observations WHERE name = $tname AND observation_type IS NOT NONE \
                     ORDER BY created_at DESC LIMIT 1",
                )
                .bind(("tname", tname.to_string()))
                .await?
                .take(0)?;

            let mut enriched = thread.clone();
            if let Some(obj) = enriched.as_object_mut() {
                obj.insert("entry_count".to_string(), json!(count_result.unwrap_or(0)));
                obj.insert(
                    "last_activity".to_string(),
                    last_entry.first().cloned().unwrap_or(json!(null)),
                );
            }
            enriched_threads.push(enriched);
        }

        let response = json!({
            "threads": enriched_threads,
            "total": enriched_threads.len(),
        });

        Ok(CallToolResult::structured(response))
    }

    /// Status mode: update a thread's status
    async fn journal_status(&self, params: &JournalParams) -> Result<CallToolResult> {
        let thread_name = params
            .thread
            .as_deref()
            .ok_or_else(|| SurrealMindError::Validation {
                message: "thread is required for status mode".into(),
            })?;
        let new_status =
            params
                .thread_status
                .as_deref()
                .ok_or_else(|| SurrealMindError::Validation {
                    message: "thread_status is required for status mode".into(),
                })?;

        if !VALID_STATUSES.contains(&new_status) {
            return Err(SurrealMindError::Validation {
                message: format!(
                    "Invalid thread_status: {}. Must be one of: {:?}",
                    new_status, VALID_STATUSES
                ),
            });
        }

        // Find thread
        let thread: Vec<serde_json::Value> = self
            .db
            .query(
                "SELECT meta::id(id) as id, name, thread_status, type::string(created_at) as created_at \
                 FROM kg_entities WHERE name = $name AND data.entity_type = 'research_thread' LIMIT 1",
            )
            .bind(("name", thread_name.to_string()))
            .await?
            .take(0)?;

        let thread_row = thread.first().ok_or_else(|| SurrealMindError::Validation {
            message: format!("Thread not found: {}", thread_name),
        })?;
        let thread_id = thread_row.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let current_status = thread_row
            .get("thread_status")
            .and_then(|v| v.as_str())
            .unwrap_or("open");

        // Validate transition
        if current_status == "abandoned" && new_status == "resolved" {
            return Err(SurrealMindError::Validation {
                message: "Cannot resolve an abandoned thread. Reopen it first by setting status to 'open' or 'pursuing'.".into(),
            });
        }

        // Update status
        self.db
            .query(
                "UPDATE kg_entities SET thread_status = $status \
                 WHERE id = type::record('kg_entities', $id) RETURN NONE",
            )
            .bind(("id", thread_id.to_string()))
            .bind(("status", new_status.to_string()))
            .await?;

        let response = json!({
            "success": true,
            "thread": {
                "id": format!("kg_entities:{}", thread_id),
                "name": thread_name,
                "previous_status": current_status,
                "new_status": new_status,
            }
        });

        Ok(CallToolResult::structured(response))
    }
}
