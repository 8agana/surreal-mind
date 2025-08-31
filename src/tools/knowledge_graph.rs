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
        let mut data = args.get("data").cloned().unwrap_or(serde_json::json!({}));
        // Normalize entity_type alias: if data.type provided, copy to data.entity_type
        let alias_type = data
            .get("type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let has_entity_type = data.get("entity_type").is_some();
        if !has_entity_type {
            if let (Some(t), Some(obj)) = (alias_type, data.as_object_mut()) {
                obj.insert("entity_type".to_string(), serde_json::Value::String(t));
            }
        }
        // Determine upsert behavior (default true)
        let upsert = args.get("upsert").and_then(|v| v.as_bool()).unwrap_or(true);

        let id: String = match kind_s.as_str() {
            "entity" => {
                let name_s: String = data
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let entity_type_s: Option<String> = data
                    .get("entity_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                // Try upsert: find existing by name + entity_type when available
                if upsert {
                    let mut sql = "SELECT meta::id(id) as id FROM kg_entities WHERE string::lower(name) = string::lower($name)".to_string();
                    if entity_type_s.is_some() {
                        sql.push_str(" AND data.entity_type = $etype");
                    }
                    sql.push_str(" LIMIT 1");
                    let mut q = self.db.query(sql).bind(("name", name_s.clone()));
                    if let Some(ref et) = entity_type_s {
                        q = q.bind(("etype", et.clone()));
                    }
                    let found: Vec<serde_json::Value> = q.await?.take(0)?;
                    if let Some(idv) = found
                        .first()
                        .and_then(|v| v.get("id"))
                        .and_then(|v| v.as_str())
                    {
                        let result = json!({"kind": kind_s, "id": idv, "created": false});
                        return Ok(CallToolResult::structured(result));
                    }
                }

                // Create new entity; also store entity_type as top-level convenience if present
                let created_raw: Vec<serde_json::Value> = self
                    .db
                    .query("CREATE kg_entities SET created_at = time::now(), name = $name, entity_type = $etype, data = $data RETURN meta::id(id) as id, name, data, created_at;")
                    .bind(("name", name_s.clone()))
                    .bind(("etype", entity_type_s.clone().unwrap_or_default()))
                    .bind(("data", data.clone()))
                    .await?
                    .take(0)?;
                created_raw
                    .first()
                    .and_then(|v| v.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            }
            "relationship" => {
                // Accept both {source,target,rel_type} and {from_id,to_id,relationship_type}
                let src_s: String = data
                    .get("source")
                    .or_else(|| data.get("from_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let dst_s: String = data
                    .get("target")
                    .or_else(|| data.get("to_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let rel_kind_s: String = data
                    .get("rel_type")
                    .or_else(|| data.get("relationship_type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("related_to")
                    .to_string();
                // Upsert: check for existing triplet
                if upsert {
                    let found: Vec<serde_json::Value> = self
                        .db
                        .query("SELECT meta::id(id) as id FROM kg_edges WHERE source = $src AND target = $dst AND rel_type = $kind LIMIT 1")
                        .bind(("src", src_s.clone()))
                        .bind(("dst", dst_s.clone()))
                        .bind(("kind", rel_kind_s.clone()))
                        .await?
                        .take(0)?;
                    if let Some(idv) = found
                        .first()
                        .and_then(|v| v.get("id"))
                        .and_then(|v| v.as_str())
                    {
                        let result = json!({"kind": kind_s, "id": idv, "created": false});
                        return Ok(CallToolResult::structured(result));
                    }
                }

                let created_raw: Vec<serde_json::Value> = self
                    .db
                    .query("CREATE kg_edges SET created_at = time::now(), source = $src, target = $dst, rel_type = $kind, data = $data RETURN meta::id(id) as id, source, target, rel_type, data, created_at;")
                    .bind(("src", src_s))
                    .bind(("dst", dst_s))
                    .bind(("kind", rel_kind_s))
                    .bind(("data", data.clone()))
                    .await?
                    .take(0)?;
                created_raw
                    .first()
                    .and_then(|v| v.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            }
            "observation" => {
                let name_s: String = data
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let source_thought_id_s: String = args
                    .get("source_thought_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let confidence_f: f64 = args
                    .get("confidence")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0f64);
                // Upsert: name + source_thought_id
                if upsert && !name_s.is_empty() && !source_thought_id_s.is_empty() {
                    let found: Vec<serde_json::Value> = self
                        .db
                        .query("SELECT meta::id(id) as id FROM kg_observations WHERE string::lower(name) = string::lower($name) AND source_thought_id = $src LIMIT 1")
                        .bind(("name", name_s.clone()))
                        .bind(("src", source_thought_id_s.clone()))
                        .await?
                        .take(0)?;
                    if let Some(idv) = found
                        .first()
                        .and_then(|v| v.get("id"))
                        .and_then(|v| v.as_str())
                    {
                        let result = json!({"kind": kind_s, "id": idv, "created": false});
                        return Ok(CallToolResult::structured(result));
                    }
                }
                let created_raw: Vec<serde_json::Value> = self
                    .db
                    .query("CREATE kg_observations SET created_at = time::now(), name = $name, data = $data, source_thought_id = $src, confidence = $conf RETURN meta::id(id) as id, name, data, created_at;")
                    .bind(("name", name_s.clone()))
                    .bind(("data", data.clone()))
                    .bind(("src", source_thought_id_s))
                    .bind(("conf", confidence_f))
                    .await?
                    .take(0)?;
                created_raw
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
        // Accept flexible top_k (int, float, string)
        let top_k = match args.get("top_k") {
            Some(v) => match v {
                serde_json::Value::Number(n) => {
                    if let Some(u) = n.as_u64() {
                        u as usize
                    } else if let Some(f) = n.as_f64() {
                        f.round().clamp(1.0, 50.0) as usize
                    } else {
                        10
                    }
                }
                serde_json::Value::String(s) => s
                    .parse::<f64>()
                    .map(|f| f.round().clamp(1.0, 50.0) as usize)
                    .unwrap_or(10),
                _ => 10,
            },
            None => 10,
        };
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
                    "SELECT meta::id(id) as id, name, data, created_at FROM kg_entities LIMIT {}",
                    top_k
                )
            } else {
                format!(
                    "SELECT meta::id(id) as id, name, data, created_at FROM kg_entities WHERE string::lower(name) CONTAINS string::lower($name) LIMIT {}",
                    top_k
                )
            };
            let mut q = self.db.query(sql);
            if !name_like_s.is_empty() {
                q = q.bind(("name", name_like_s.clone()));
            }
            let rows: Vec<serde_json::Value> = q.await?.take(0)?;
            items.extend(rows);
        }
        if target_s == "relationship" || target_s == "mixed" {
            let sql = format!(
                "SELECT meta::id(id) as id, source, target, rel_type, data, created_at FROM kg_edges LIMIT {}",
                top_k
            );
            let rows: Vec<serde_json::Value> = self.db.query(sql).await?.take(0)?;
            items.extend(rows);
        }
        if target_s == "observation" || target_s == "mixed" {
            let sql = if name_like_s.is_empty() {
                format!(
                    "SELECT meta::id(id) as id, name, data, created_at FROM kg_observations LIMIT {}",
                    top_k
                )
            } else {
                format!(
                    "SELECT meta::id(id) as id, name, data, created_at FROM kg_observations WHERE string::lower(name) CONTAINS string::lower($name) LIMIT {}",
                    top_k
                )
            };
            let mut q = self.db.query(sql);
            if !name_like_s.is_empty() {
                q = q.bind(("name", name_like_s.clone()));
            }
            let rows: Vec<serde_json::Value> = q.await?.take(0)?;
            items.extend(rows);
        }

        let result = json!({
            "items": items
        });
        Ok(CallToolResult::structured(result))
    }
}
