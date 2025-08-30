//! Knowledge graph tool handlers for creating and searching entities/relationships

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;

impl SurrealMindServer {
    /// Handle the knowledgegraph_create tool call
    pub async fn handle_knowledgegraph_create(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;

        let kind_s: String = args
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("entity")
            .to_string();
        let data = args.get("data").cloned().unwrap_or(serde_json::json!({}));

        let id: String = match kind_s.as_str() {
            "entity" => {
                let name_s: String = data
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let created_raw: Vec<surrealdb::sql::Value> = self
                    .db
                    .query("CREATE kg_entities SET created_at = time::now(), name = $name, data = $data RETURN AFTER;")
                    .bind(("name", name_s.clone()))
                    .bind(("data", data.clone()))
                    .await?
                    .take(0)?;
                let created: Vec<serde_json::Value> = created_raw
                    .into_iter()
                    .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
                    .collect();
                created
                    .first()
                    .and_then(|v| v.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            }
            "relationship" => {
                let src_s: String = data
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let dst_s: String = data
                    .get("target")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let rel_kind_s: String = data
                    .get("rel_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("related_to")
                    .to_string();
                let created_raw: Vec<surrealdb::sql::Value> = self
                    .db
                    .query("CREATE kg_edges SET created_at = time::now(), source = $src, target = $dst, rel_type = $kind, data = $data RETURN AFTER;")
                    .bind(("src", src_s))
                    .bind(("dst", dst_s))
                    .bind(("kind", rel_kind_s))
                    .bind(("data", data.clone()))
                    .await?
                    .take(0)?;
                let created: Vec<serde_json::Value> = created_raw
                    .into_iter()
                    .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
                    .collect();
                created
                    .first()
                    .and_then(|v| v.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            }
            _ => {
                return Err(SurrealMindError::Validation {
                    message: format!("Unsupported KG kind: {}", kind_s),
                });
            }
        };

        let result = json!({
            "kind": kind_s,
            "id": id,
            "created": true
        });

        Ok(CallToolResult::structured(result))
    }

    /// Handle the knowledgegraph_search tool call
    pub async fn handle_knowledgegraph_search(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;

        let target_s: String = args
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("mixed")
            .to_string();
        let top_k = args.get("top_k").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
        let name_like_s: String = args
            .get("query")
            .and_then(|q| q.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let mut items: Vec<serde_json::Value> = Vec::new();
        if target_s == "entity" || target_s == "mixed" {
            let sql = if name_like_s.is_empty() {
                format!(
                    "SELECT id, name, data, created_at FROM kg_entities LIMIT {}",
                    top_k
                )
            } else {
                format!(
                    "SELECT id, name, data, created_at FROM kg_entities WHERE string::lower(name) CONTAINS string::lower($name) LIMIT {}",
                    top_k
                )
            };
            let mut q = self.db.query(sql);
            if !name_like_s.is_empty() {
                q = q.bind(("name", name_like_s.clone()));
            }
            let rows_raw: Vec<surrealdb::sql::Value> = q.await?.take(0)?;
            let rows: Vec<serde_json::Value> = rows_raw
                .into_iter()
                .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
                .collect();
            items.extend(rows);
        }
        if target_s == "relationship" || target_s == "mixed" {
            let sql = format!(
                "SELECT id, source, target, rel_type, data, created_at FROM kg_edges LIMIT {}",
                top_k
            );
            let rows_raw: Vec<surrealdb::sql::Value> = self.db.query(sql).await?.take(0)?;
            let rows: Vec<serde_json::Value> = rows_raw
                .into_iter()
                .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
                .collect();
            items.extend(rows);
        }

        let result = json!({
            "items": items
        });
        Ok(CallToolResult::structured(result))
    }
}
