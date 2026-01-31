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
        if !has_entity_type && let (Some(t), Some(obj)) = (alias_type, data.as_object_mut()) {
            obj.insert("entity_type".to_string(), serde_json::Value::String(t));
        }
        // Determine upsert behavior (default true)
        let upsert = args.get("upsert").and_then(|v| v.as_bool()).unwrap_or(true);

        #[allow(unused_assignments)]
        let mut id: String = "".to_string();
        #[allow(unused_assignments)]
        let mut name: String = "".to_string();

        match kind_s.as_str() {
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
                let entity_id = created_raw
                    .first()
                    .and_then(|v| v.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                id = entity_id;
                name = name_s;
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

                // 1. Resolve KG refs to concrete Things
                let src_thing = self.resolve_kg_item(&src_s).await?;
                let dst_thing = self.resolve_kg_item(&dst_s).await?;

                let (src_thing, dst_thing) = match (src_thing, dst_thing) {
                    (Some(s), Some(d)) => (s, d),
                    _ => {
                        return Err(SurrealMindError::Validation {
                            message: format!(
                                "Could not resolve one or both KG items for relationship: src: '{}', dst: '{}'",
                                src_s, dst_s
                            ),
                        });
                    }
                };

                let existing_rel: Vec<serde_json::Value> = self
                    .db
                    .query("SELECT meta::id(id) as id FROM kg_edges WHERE source = $src AND target = $dst AND rel_type = $rel LIMIT 1")
                    .bind(("src", src_thing.clone()))
                    .bind(("dst", dst_thing.clone()))
                    .bind(("rel", rel_kind_s.clone()))
                    .await?
                    .take(0)?;

                if let Some(rel_row) = existing_rel.first()
                    && let Some(rel_id) = rel_row.get("id").and_then(|v| v.as_str())
                {
                    let result = json!({"kind": kind_s, "id": rel_id, "created": false});
                    return Ok(CallToolResult::structured(result));
                }

                // 3. Create new relationship
                let created_rel: Vec<serde_json::Value> = self
                    .db
                    .query("CREATE kg_edges SET created_at = time::now(), source = $src, target = $dst, rel_type = $rel, data = $data RETURN meta::id(id) as id, rel_type, created_at;")
                    .bind(("src", src_thing))
                    .bind(("dst", dst_thing))
                    .bind(("rel", rel_kind_s))
                    .bind(("data", data.clone()))
                    .await?
                    .take(0)?;
                let rel_id = created_rel
                    .first()
                    .and_then(|v| v.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                // Relationships don't need embedding
                id = rel_id;
                name = "".to_string();
            }
            "observation" => {
                let source_thought_id_s: String = data
                    .get("source_thought_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let confidence_f = data
                    .get("confidence")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.5)
                    .clamp(0.0, 1.0);
                let name_s: String = data
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                // Upsert: if name and source_thought_id match, update instead of create
                if upsert {
                    let existing_obs: Vec<serde_json::Value> = self
                        .db
                        .query("SELECT meta::id(id) as id FROM kg_observations WHERE name = $name AND data.source_thought_id = $src LIMIT 1")
                        .bind(("name", name_s.clone()))
                        .bind(("src", source_thought_id_s.clone()))
                        .await?
                        .take(0)?;
                    if let Some(obs_row) = existing_obs.first()
                        && let Some(obs_id) = obs_row.get("id").and_then(|v| v.as_str())
                    {
                        let result = json!({"kind": kind_s, "id": obs_id, "created": false});
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
                let obs_id = created_raw
                    .first()
                    .and_then(|v| v.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                id = obs_id;
                name = name_s;
            }
            _ => {
                return Err(SurrealMindError::Validation {
                    message: format!("Unsupported KG kind: {}", kind_s),
                });
            }
        }

        // Auto-embed newly created entities/observations
        if kind_s == "entity" {
            if let Err(e) = self
                .ensure_kg_embedding("kg_entities", &id, &name, &data)
                .await
            {
                tracing::warn!(
                    "kg_embedding: failed to auto-embed created entity {}: {}",
                    id,
                    e
                );
            }
        } else if kind_s == "observation"
            && let Err(e) = self
                .ensure_kg_embedding("kg_observations", &id, &name, &data)
                .await
        {
            tracing::warn!(
                "kg_embedding: failed to auto-embed created observation {}: {}",
                id,
                e
            );
        }

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
                    "SELECT meta::id(id) as id, name, data, created_at FROM kg_entities WHERE name ~ $name LIMIT {}",
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
            // Project to string IDs safely; handle legacy rows with non-record source/target
            let sql = format!(
                "SELECT meta::id(id) as id,
                        (IF type::is::record(source) THEN meta::id(source) ELSE string::concat(source) END) as source_id,
                        (IF type::is::record(target) THEN meta::id(target) ELSE string::concat(target) END) as target_id,
                        rel_type, data, created_at
                 FROM kg_edges LIMIT {}",
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
                    "SELECT meta::id(id) as id, name, data, created_at FROM kg_observations WHERE name ~ $name LIMIT {}",
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

    /// Helper: ensure a KG record has an up-to-date embedding
    async fn ensure_kg_embedding(
        &self,
        table: &str,
        id: &str,
        name: &str,
        data: &serde_json::Value,
    ) -> Result<()> {
        let provider = self.config.system.embedding_provider.clone();
        let model = self.config.system.embedding_model.clone();
        let dim = self.config.system.embedding_dimensions;

        let mut text = name.to_string();
        if table == "kg_entities" {
            if let Some(entity_type) = data.get("entity_type").and_then(|v| v.as_str()) {
                text.push_str(&format!(" ({})", entity_type));
            }
        } else if table == "kg_observations"
            && let Some(description) = data.get("description").and_then(|v| v.as_str())
        {
            text.push_str(&format!(" - {}", description));
        }

        // Generate embedding
        let embedding = self.embedder.embed(&text).await?;

        // Update record with embedding metadata
        self.db
            .query(
                "UPDATE type::thing($tb, $id) SET embedding = $emb, embedding_provider = $prov, embedding_model = $model, embedding_dim = $dim, embedded_at = time::now()",
            )
            .bind(("tb", table.to_string()))
            .bind(("id", id.to_string()))
            .bind(("emb", embedding))
            .bind(("prov", provider))
            .bind(("model", model))
            .bind(("dim", dim))
            .await?;
        Ok(())
    }

    /// Helper: resolve an entity ref to its bare meta id string (no Thing types used in Rust).
    /// Accepts full Thing strings, bare IDs, or names.
    /// Helper: resolve a KG reference (ID string, Thing, or Name) to a concrete Thing.
    /// Searches across kg_entities, kg_observations, and thoughts.
    async fn resolve_kg_item(&self, entity: &str) -> Result<Option<surrealdb::sql::Thing>> {
        use std::str::FromStr;
        // 1. If it's already a valid Thing string (table:id)
        if let Ok(thing) = surrealdb::sql::Thing::from_str(entity)
            && !thing.tb.is_empty()
        {
            return Ok(Some(thing));
        }

        // 2. Try to find by bare ID or Name in kg_entities
        let entities: Vec<serde_json::Value> = self.db.query(
            "SELECT id FROM kg_entities WHERE meta::id(id) = $val OR name = $val LIMIT 1"
        ).bind(("val", entity.to_string())).await?.take(0)?;
        if let Some(row) = entities.first() && let Some(id_val) = row.get("id") {
             if let Ok(thing) = surrealdb::sql::Thing::from_str(&id_val.to_string().replace("\"", "")) {
                 return Ok(Some(thing));
             }
        }

        // 3. Try to find by bare ID or Name in kg_observations
        let observations: Vec<serde_json::Value> = self.db.query(
            "SELECT id FROM kg_observations WHERE meta::id(id) = $val OR name = $val LIMIT 1"
        ).bind(("val", entity.to_string())).await?.take(0)?;
        if let Some(row) = observations.first() && let Some(id_val) = row.get("id") {
             if let Ok(thing) = surrealdb::sql::Thing::from_str(&id_val.to_string().replace("\"", "")) {
                 return Ok(Some(thing));
             }
        }

        // 4. Try to find by bare ID in thoughts
        let thoughts: Vec<serde_json::Value> = self.db.query(
            "SELECT id FROM thoughts WHERE meta::id(id) = $val LIMIT 1"
        ).bind(("val", entity.to_string())).await?.take(0)?;
        if let Some(row) = thoughts.first() && let Some(id_val) = row.get("id") {
             if let Ok(thing) = surrealdb::sql::Thing::from_str(&id_val.to_string().replace("\"", "")) {
                 return Ok(Some(thing));
             }
        }

        Ok(None)
    }
}
