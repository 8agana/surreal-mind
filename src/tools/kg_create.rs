//! knowledgegraph_create tool handler
use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;
use std::str::FromStr;

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
                    let mut sql =
                        "SELECT meta::id(id) as id FROM kg_entities WHERE name = $name".to_string();
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

                tracing::debug!(
                    "Attempting to create relationship: src: '{}', dst: '{}', kind: '{}'",
                    src_s,
                    dst_s,
                    rel_kind_s
                );

                // 1. Resolve entity refs to bare IDs (strings)
                let src_id = self.resolve_entity_id_str(&src_s).await?;
                let dst_id = self.resolve_entity_id_str(&dst_s).await?;

                if src_id.is_none() || dst_id.is_none() {
                    return Err(SurrealMindError::Validation {
                        message: format!(
                            "Could not resolve one or both entities for relationship: src: '{}', dst: '{}'",
                            src_s, dst_s
                        ),
                    });
                }

                let src_bare = src_id.unwrap();
                let dst_bare = dst_id.unwrap();

                // Build Thing records for bind parameters
                let src_thing =
                    surrealdb::sql::Thing::from_str(&format!("kg_entities:{}", src_bare)).ok();
                let dst_thing =
                    surrealdb::sql::Thing::from_str(&format!("kg_entities:{}", dst_bare)).ok();
                if src_thing.is_none() || dst_thing.is_none() {
                    return Err(SurrealMindError::Validation {
                        message: "Failed to construct record links for relationship".into(),
                    });
                }
                let src_thing = src_thing.unwrap();
                let dst_thing = dst_thing.unwrap();

                // Upsert: check for existing triplet using resolved Things in kg_edges
                if upsert {
                    let found: Vec<serde_json::Value> = self
                        .db
                        .query("SELECT meta::id(id) as id FROM kg_edges WHERE source = $src AND target = $dst AND rel_type = $kind LIMIT 1")
                        .bind(("src", src_thing.clone()))
                        .bind(("dst", dst_thing.clone()))
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

                // 2. Use RELATE with bound Things; persist source/target automatically
                let created_raw: Vec<serde_json::Value> = self
                    .db
                    .query("RELATE $src->kg_edges->$dst SET rel_type = $kind, source = $src, target = $dst, created_at = time::now(), data = $data RETURN meta::id(id) as id;")
                    .bind(("src", src_thing))
                    .bind(("dst", dst_thing))
                    .bind(("kind", rel_kind_s))
                    .bind(("data", data.clone()))
                    .await?
                    .take(0)?;

                let maybe_id = created_raw
                    .first()
                    .and_then(|v| v.get("id"))
                    .and_then(|v| v.as_str());

                if let Some(id) = maybe_id {
                    id.to_string()
                } else {
                    return Err(SurrealMindError::Mcp {
                        message: "Failed to create relationship edge in kg_edges".into(),
                    });
                }
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
                        .query("SELECT meta::id(id) as id FROM kg_observations WHERE name = $name AND source_thought_id = $src LIMIT 1")
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

    /// Helper: resolve an entity ref to its bare meta id string (no Thing types used in Rust).
    /// Accepts full Thing strings, bare IDs, or names.
    pub async fn resolve_entity_id_str(&self, entity: &str) -> Result<Option<String>> {
        // Full Thing? parse and return the inner id as string
        if let Ok(thing) = surrealdb::sql::Thing::from_str(entity) {
            if !thing.tb.is_empty() {
                return Ok(Some(thing.id.to_string()));
            }
        }
        // Try match by exact meta id or constructed thing, else by name
        let mut q = self
            .db
            .query(
                "SELECT meta::id(id) as id FROM kg_entities \
                 WHERE meta::id(id) = $id OR id = type::thing('kg_entities', $id) OR name = $name \
                 LIMIT 1",
            )
            .bind(("id", entity.to_string()))
            .bind(("name", entity.to_string()))
            .await?;
        let rows: Vec<serde_json::Value> = q.take(0)?;
        let id = rows
            .first()
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Ok(id)
    }
}
