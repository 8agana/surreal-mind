//! Knowledge graph tool handlers for creating and searching entities/relationships

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;
use std::str::FromStr;
use strsim;

impl SurrealMindServer {
    /// Handle knowledgegraph_moderate: unified review + decide interface
    pub async fn handle_knowledgegraph_moderate(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;

        let action_s = args
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("review");
        let target_s = args
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("mixed")
            .to_string();
        let status_s = args
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("pending")
            .to_string();
        let min_conf = args.get("min_conf").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let limit = match args.get("limit") {
            Some(v) => match v {
                serde_json::Value::Number(n) => n.as_u64().unwrap_or(50) as usize,
                serde_json::Value::String(s) => s.parse::<usize>().unwrap_or(50),
                _ => 50,
            },
            None => 50,
        };
        let offset = match args.get("offset") {
            Some(v) => match v {
                serde_json::Value::Number(n) => n.as_u64().unwrap_or(0) as usize,
                serde_json::Value::String(s) => s.parse::<usize>().unwrap_or(0),
                _ => 0,
            },
            None => 0,
        };
        let _cursor = args
            .get("cursor")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let dry_run = args
            .get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut out = serde_json::Map::new();

        // Optional review phase
        if action_s == "review" || action_s == "review_and_decide" {
            let mut items: Vec<serde_json::Value> = Vec::new();

            if target_s == "entity" || target_s == "mixed" {
                let sql = format!(
                    "SELECT meta::id(id) as id, name, entity_type, data, confidence, status, created_at FROM kg_entity_candidates WHERE status = $status AND confidence >= $minc ORDER BY created_at DESC LIMIT {} START {}",
                    limit, offset
                );
                let mut entity_rows: Vec<serde_json::Value> = self
                    .db
                    .query(sql)
                    .bind(("status", status_s.clone()))
                    .bind(("minc", min_conf))
                    .await?
                    .take(0)?;

                // Add canonical suggestions for entity candidates
                for entity in &mut entity_rows {
                    if let Some(name) = entity.get("name").and_then(|v| v.as_str()) {
                        if let Some(entity_type) =
                            entity.get("entity_type").and_then(|v| v.as_str())
                        {
                            let suggestions =
                                self.find_similar_entities(name, entity_type, 3).await?;
                            if !suggestions.is_empty() {
                                let suggestions_json = suggestions
                                    .iter()
                                    .map(|(id, name, score)| {
                                        json!({
                                            "id": id,
                                            "name": name,
                                            "similarity": score
                                        })
                                    })
                                    .collect::<Vec<_>>();
                                if let Some(obj) = entity.as_object_mut() {
                                    obj.insert(
                                        "canonical_suggestions".to_string(),
                                        json!({"suggestions": suggestions_json}),
                                    );
                                }
                            }
                        }
                    }
                }

                items.extend(entity_rows);
            }
            if target_s == "relationship" || target_s == "mixed" {
                let sql = format!(
                    "SELECT meta::id(id) as id,
                            source_name,
                            target_name,
                            (IF type::is::record(source_id) THEN meta::id(source_id) ELSE string::concat(source_id) END) AS source_id,
                            (IF type::is::record(target_id) THEN meta::id(target_id) ELSE string::concat(target_id) END) AS target_id,
                            rel_type, data, confidence, status, created_at
                     FROM kg_edge_candidates WHERE status = $status AND confidence >= $minc ORDER BY created_at DESC LIMIT {} START {}",
                    limit, offset
                );
                let rows: Vec<serde_json::Value> = self
                    .db
                    .query(sql)
                    .bind(("status", status_s.clone()))
                    .bind(("minc", min_conf))
                    .await?
                    .take(0)?;
                items.extend(rows);
            }

            out.insert(
                "review".to_string(),
                serde_json::Value::Object(serde_json::Map::from_iter(vec![(
                    "items".to_string(),
                    serde_json::Value::Array(items),
                )])),
            );
        }

        // Optional decide phase
        if action_s == "decide" || action_s == "review_and_decide" {
            let items: Vec<serde_json::Value> = args
                .get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            if !items.is_empty() && !dry_run {
                let mut results: Vec<serde_json::Value> = Vec::new();
                for item in items {
                    let id_s = item
                        .get("id")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| SurrealMindError::Validation {
                            message: "Missing id in decision".into(),
                        })?
                        .to_string();
                    let kind_s = item
                        .get("kind")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let decision_s = item
                        .get("decision")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let feedback_s = item
                        .get("feedback")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let canonical_id_opt = item
                        .get("canonical_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    match (kind_s.as_str(), decision_s.as_str()) {
                        ("entity", "approve") => {
                            let row: Option<serde_json::Value> = self
                                .db
                                .query("SELECT meta::id(id) as id, name, entity_type, data, confidence FROM type::thing($tb, $id) LIMIT 1")
                                .bind(("tb", "kg_entity_candidates"))
                                .bind(("id", id_s.clone()))
                                .await?
                                .take(0)?;
                            if let Some(candidate) = row {
                                let name = candidate
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let etype = candidate
                                    .get("entity_type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let data = candidate.get("data").cloned().unwrap_or(json!({}));

                                let found: Vec<serde_json::Value> = self
                                    .db
                                    .query("SELECT meta::id(id) as id FROM kg_entities WHERE name = $name AND data.entity_type = $etype LIMIT 1")
                                    .bind(("name", name.clone()))
                                    .bind(("etype", etype.clone()))
                                    .await?
                                    .take(0)?;
                                let final_id: String = if let Some(first) = found.first() {
                                    first
                                        .get("id")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string()
                                } else {
                                    let created: Vec<serde_json::Value> = self
                                        .db
                                        .query("CREATE kg_entities SET created_at = time::now(), name = $name, entity_type = $etype, data = $data RETURN meta::id(id) as id")
                                        .bind(("name", name.clone()))
                                        .bind(("etype", etype.clone()))
                                        .bind(("data", data.clone()))
                                        .await?
                                        .take(0)?;
                                    created
                                        .first()
                                        .and_then(|v| v.get("id"))
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string()
                                };
                                let _ = self
                                    .db
                                    .query("UPDATE type::thing($tb, $id) SET status = 'approved', reviewed_at = time::now(), feedback = $fb, promoted_id = $pid RETURN meta::id(id) as id")
                                    .bind(("tb", "kg_entity_candidates"))
                                    .bind(("id", id_s.clone()))
                                    .bind(("fb", feedback_s.clone()))
                                    .bind(("pid", final_id.clone()))
                                    .await?;
                                results.push(json!({"id": id_s, "kind": "entity", "decision": "approved", "promoted_id": final_id}));
                            }
                        }
                        ("entity", "reject") => {
                            let _ = self
                                .db
                                .query("UPDATE type::thing($tb, $id) SET status = 'rejected', reviewed_at = time::now(), feedback = $fb RETURN meta::id(id) as id")
                                .bind(("tb", "kg_entity_candidates"))
                                .bind(("id", id_s.clone()))
                                .bind(("fb", feedback_s.clone()))
                                .await?;
                            results.push(
                                json!({"id": id_s, "kind": "entity", "decision": "rejected"}),
                            );
                        }
                        ("relationship", "approve") => {
                            let row: Option<serde_json::Value> = self
                                .db
                                .query("SELECT meta::id(id) as id,
                                               source_name,
                                               target_name,
                                               (IF type::is::record(source_id) THEN meta::id(source_id) ELSE string::concat(source_id) END) AS source_id,
                                               (IF type::is::record(target_id) THEN meta::id(target_id) ELSE string::concat(target_id) END) AS target_id,
                                               rel_type, data, confidence
                                        FROM type::thing($tb, $id) LIMIT 1")
                                .bind(("tb", "kg_edge_candidates"))
                                .bind(("id", id_s.clone()))
                                .await?
                                .take(0)?;
                            if let Some(candidate) = row {
                                let rel_type = candidate
                                    .get("rel_type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("related_to")
                                    .to_string();
                                let data = candidate.get("data").cloned().unwrap_or(json!({}));
                                let src_id_val =
                                    candidate.get("source_id").and_then(|v| v.as_str());
                                let dst_id_val =
                                    candidate.get("target_id").and_then(|v| v.as_str());
                                let src_name =
                                    candidate.get("source_name").and_then(|v| v.as_str());
                                let dst_name =
                                    candidate.get("target_name").and_then(|v| v.as_str());

                                let src_bare_opt = if let Some(cid) = canonical_id_opt.as_deref() {
                                    self.resolve_entity_id_str(cid).await?
                                } else if let Some(idstr) = src_id_val {
                                    self.resolve_entity_id_str(idstr).await?
                                } else if let Some(name) = src_name {
                                    self.resolve_entity_id_str(name).await?
                                } else {
                                    None
                                };

                                let dst_bare_opt = if let Some(idstr) = dst_id_val {
                                    self.resolve_entity_id_str(idstr).await?
                                } else if let Some(name) = dst_name {
                                    self.resolve_entity_id_str(name).await?
                                } else {
                                    None
                                };

                                if let (Some(src_bare), Some(dst_bare)) =
                                    (src_bare_opt, dst_bare_opt)
                                {
                                    let found: Vec<serde_json::Value> = self
                                        .db
                                        .query("SELECT meta::id(id) as id FROM kg_edges WHERE source = type::thing('kg_entities', $src) AND target = type::thing('kg_entities', $dst) AND rel_type = $kind LIMIT 1")
                                        .bind(("src", src_bare.clone()))
                                        .bind(("dst", dst_bare.clone()))
                                        .bind(("kind", rel_type.clone()))
                                        .await?
                                        .take(0)?;
                                    let final_id: String = if let Some(first) = found.first() {
                                        first
                                            .get("id")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string()
                                    } else {
                                        let created: Vec<serde_json::Value> = self
                                            .db
                                            .query("CREATE kg_edges SET created_at = time::now(), source = type::thing('kg_entities', $source), target = type::thing('kg_entities', $target), rel_type = $rel_type, data = $data RETURN meta::id(id) as id")
                                            .bind(("source", src_bare))
                                            .bind(("target", dst_bare))
                                            .bind(("rel_type", rel_type.clone()))
                                            .bind(("data", data.clone()))
                                            .await?
                                            .take(0)?;
                                        created
                                            .first()
                                            .and_then(|v| v.get("id"))
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string()
                                    };

                                    let _ = self
                                        .db
                                        .query("UPDATE type::thing($tb, $id) SET status = 'approved', reviewed_at = time::now(), feedback = $fb, promoted_id = $pid RETURN meta::id(id) as id")
                                        .bind(("tb", "kg_edge_candidates"))
                                        .bind(("id", id_s.clone()))
                                        .bind(("fb", feedback_s.clone()))
                                        .bind(("pid", final_id.clone()))
                                        .await?;
                                    results.push(json!({"id": id_s, "kind": "relationship", "decision": "approved", "promoted_id": final_id}));
                                } else {
                                    return Err(SurrealMindError::Validation {
                                        message: format!(
                                            "Could not resolve entity IDs for candidate edge {}",
                                            id_s
                                        ),
                                    });
                                }
                            }
                        }
                        ("entity", "alias") => {
                            // Handle entity alias decision
                            let canonical_id =
                                canonical_id_opt.ok_or_else(|| SurrealMindError::Validation {
                                    message: "canonical_id required for alias decision".into(),
                                })?;

                            // Validate that the canonical entity exists
                            let canonical_exists: Vec<serde_json::Value> = self
                                .db
                                .query("SELECT meta::id(id) as id FROM kg_entities WHERE meta::id(id) = $cid LIMIT 1")
                                .bind(("cid", canonical_id.clone()))
                                .await?
                                .take(0)?;

                            if canonical_exists.is_empty() {
                                return Err(SurrealMindError::Validation {
                                    message: format!("Canonical entity {} not found", canonical_id),
                                });
                            }

                            // Create the candidate as an alias entity
                            let row: Option<serde_json::Value> = self
                                .db
                                .query("SELECT meta::id(id) as id, name, entity_type, data, confidence FROM type::thing($tb, $id) LIMIT 1")
                                .bind(("tb", "kg_entity_candidates"))
                                .bind(("id", id_s.clone()))
                                .await?
                                .take(0)?;

                            if let Some(candidate) = row {
                                let name = candidate
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let etype = candidate
                                    .get("entity_type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let data = candidate.get("data").cloned().unwrap_or(json!({}));

                                // Create the alias entity
                                let created: Vec<serde_json::Value> = self
                                    .db
                                    .query("CREATE kg_entities SET created_at = time::now(), name = $name, entity_type = $etype, data = { ...$data, is_alias: true, canonical_id: $canonical_id }, embedding = $embedding RETURN meta::id(id) as id")
                                    .bind(("name", name.clone()))
                                    .bind(("etype", etype))
                                    .bind(("data", data.clone()))
                                    .bind(("canonical_id", canonical_id.clone()))
                                    .bind(("embedding", candidate.get("embedding").cloned().unwrap_or(json!(null))))
                                    .await?
                                    .take(0)?;

                                let final_id = created
                                    .first()
                                    .and_then(|v| v.get("id"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                // Mark candidate as aliased
                                let _ = self
                                    .db
                                    .query("UPDATE type::thing($tb, $id) SET status = 'approved', reviewed_at = time::now(), feedback = $fb, promoted_id = $pid RETURN meta::id(id) as id")
                                    .bind(("tb", "kg_observation_candidates"))
                                    .bind(("id", id_s.clone()))
                                    .bind(("fb", feedback_s.clone()))
                                    .bind(("pid", final_id.clone()))
                                    .await?;
                                // Embed new aliased entity (always new)
                                if let Err(e) = self
                                    .ensure_kg_embedding("kg_entities", &final_id, &name, &data)
                                    .await
                                {
                                    tracing::warn!(
                                        "kg_embedding: failed to auto-embed moderated entity {}: {}",
                                        final_id,
                                        e
                                    );
                                }
                                results.push(json!({"id": id_s, "kind": "entity", "decision": "aliased", "promoted_id": final_id}));
                            }
                        }
                        ("relationship", "reject") => {
                            let _ = self
                                .db
                                .query("UPDATE type::thing($tb, $id) SET status = 'rejected', reviewed_at = time::now(), feedback = $fb RETURN meta::id(id) as id")
                                .bind(("tb", "kg_edge_candidates"))
                                .bind(("id", id_s.clone()))
                                .bind(("fb", feedback_s.clone()))
                                .await?;
                            results.push(
                                json!({"id": id_s, "kind": "relationship", "decision": "rejected"}),
                            );
                        }
                        _ => {
                            return Err(SurrealMindError::Validation {
                                message: format!(
                                    "Unsupported decision: kind='{}' decision='{}'",
                                    kind_s, decision_s
                                ),
                            });
                        }
                    }
                }
                out.insert("results".to_string(), serde_json::Value::Array(results));
            } else if dry_run && !items.is_empty() {
                out.insert("results".to_string(), serde_json::Value::Array(vec![]));
            }
        }

        Ok(CallToolResult::structured(serde_json::Value::Object(out)))
    }
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

                // 1. Resolve entity refs to bare IDs (strings)
                let src_id = self.resolve_entity_id_str(&src_s).await?;
                let dst_id = self.resolve_entity_id_str(&dst_s).await?;

                let (src_bare, dst_bare) = match (src_id, dst_id) {
                    (Some(s), Some(d)) => (s, d),
                    _ => {
                        return Err(SurrealMindError::Validation {
                            message: format!(
                                "Could not resolve one or both entities for relationship: src: '{}', dst: '{}'",
                                src_s, dst_s
                            ),
                        });
                    }
                };
                // Build Thing records for bind parameters
                let src_thing =
                    surrealdb::sql::Thing::from_str(&format!("kg_entities:{}", src_bare)).map_err(
                        |_| SurrealMindError::Validation {
                            message: format!(
                                "Failed to construct record link for source entity: {}",
                                src_bare
                            ),
                        },
                    )?;
                let dst_thing =
                    surrealdb::sql::Thing::from_str(&format!("kg_entities:{}", dst_bare)).map_err(
                        |_| SurrealMindError::Validation {
                            message: format!(
                                "Failed to construct record link for destination entity: {}",
                                dst_bare
                            ),
                        },
                    )?;

                let existing_rel: Vec<serde_json::Value> = self
                    .db
                    .query("SELECT meta::id(id) as id FROM kg_edges WHERE source = $src AND target = $dst AND rel_type = $rel LIMIT 1")
                    .bind(("src", src_thing.clone()))
                    .bind(("dst", dst_thing.clone()))
                    .bind(("rel", rel_kind_s.clone()))
                    .await?
                    .take(0)?;

                if let Some(rel_row) = existing_rel.first() {
                    if let Some(rel_id) = rel_row.get("id").and_then(|v| v.as_str()) {
                        let result = json!({"kind": kind_s, "id": rel_id, "created": false});
                        return Ok(CallToolResult::structured(result));
                    }
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
                    if let Some(obs_row) = existing_obs.first() {
                        if let Some(obs_id) = obs_row.get("id").and_then(|v| v.as_str()) {
                            let result = json!({"kind": kind_s, "id": obs_id, "created": false});
                            return Ok(CallToolResult::structured(result));
                        }
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
        } else if kind_s == "observation" {
            if let Err(e) = self
                .ensure_kg_embedding("kg_observations", &id, &name, &data)
                .await
            {
                tracing::warn!(
                    "kg_embedding: failed to auto-embed created observation {}: {}",
                    id,
                    e
                );
            }
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

    /// Helper to ensure KG entities/observations have embedding vectors
    async fn ensure_kg_embedding(
        &self,
        table: &str,
        id: &str,
        name: &str,
        data: &serde_json::Value,
    ) -> Result<()> {
        let (provider, model, dim) = self.get_embedding_metadata();

        // Build embedding text based on table and data
        let mut text = name.to_string();
        if table == "kg_entities" {
            if let Some(entity_type) = data.get("entity_type").and_then(|v| v.as_str()) {
                text.push_str(&format!(" ({})", entity_type));
            }
        } else if table == "kg_observations" {
            if let Some(description) = data.get("description").and_then(|v| v.as_str()) {
                text.push_str(&format!(" - {}", description));
            }
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

    /// Handle the knowledgegraph_review tool call
    pub async fn handle_knowledgegraph_review(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;

        let target_s = args
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("mixed")
            .to_string();
        let status_s = args
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("pending")
            .to_string();
        let min_conf = args.get("min_conf").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let limit = match args.get("limit") {
            Some(v) => match v {
                serde_json::Value::Number(n) => n.as_u64().unwrap_or(50) as usize,
                serde_json::Value::String(s) => s.parse::<usize>().unwrap_or(50),
                _ => 50,
            },
            None => 50,
        };
        let offset = match args.get("offset") {
            Some(v) => match v {
                serde_json::Value::Number(n) => n.as_u64().unwrap_or(0) as usize,
                serde_json::Value::String(s) => s.parse::<usize>().unwrap_or(0),
                _ => 0,
            },
            None => 0,
        };

        let mut items: Vec<serde_json::Value> = Vec::new();

        if target_s == "entity" || target_s == "mixed" {
            let sql = format!(
                "SELECT meta::id(id) as id, name, entity_type, data, confidence, source_thought_id, status, created_at FROM kg_entity_candidates WHERE status = $status AND confidence >= $min_conf ORDER BY created_at ASC LIMIT {} START {}",
                limit, offset
            );
            let rows: Vec<serde_json::Value> = self
                .db
                .query(sql)
                .bind(("status", status_s.clone()))
                .bind(("min_conf", min_conf))
                .await?
                .take(0)?;
            items.extend(rows);
        }

        if target_s == "relationship" || target_s == "mixed" {
            let sql = format!(
                "SELECT meta::id(id) as id, source_name, target_name, source_id, target_id, rel_type, data, confidence, source_thought_id, status, created_at FROM kg_edge_candidates WHERE status = $status AND confidence >= $min_conf ORDER BY created_at ASC LIMIT {} START {}",
                limit, offset
            );
            let rows: Vec<serde_json::Value> = self
                .db
                .query(sql)
                .bind(("status", status_s))
                .bind(("min_conf", min_conf))
                .await?
                .take(0)?;
            items.extend(rows);
        }

        Ok(CallToolResult::structured(json!({"items": items})))
    }

    /// Handle the knowledgegraph_decide tool call
    pub async fn handle_knowledgegraph_decide(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;

        let items: Vec<serde_json::Value> = args
            .get("items")
            .and_then(|v| v.as_array())
            .cloned()
            .ok_or_else(|| SurrealMindError::Validation {
            message: "'items' must be an array".into(),
        })?;

        let mut results: Vec<serde_json::Value> = Vec::new();

        for item in items {
            let id_s = item
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| SurrealMindError::Validation {
                    message: "Missing id in decision".into(),
                })?
                .to_string();
            let kind_s = item
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let decision_s = item
                .get("decision")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let feedback_s = item
                .get("feedback")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let canonical_id_opt = item
                .get("canonical_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            match (kind_s.as_str(), decision_s.as_str()) {
                ("entity", "approve") => {
                    let row: Option<serde_json::Value> = self
                        .db
                        .query("SELECT * FROM type::thing($tb, $id) LIMIT 1")
                        .bind(("tb", "kg_entity_candidates"))
                        .bind(("id", id_s.clone()))
                        .await?
                        .take(0)?;
                    if let Some(candidate) = row {
                        let name = candidate
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let etype = candidate
                            .get("entity_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let data = candidate.get("data").cloned().unwrap_or(json!({}));

                        // Upsert entity (same pattern as create)
                        let found: Vec<serde_json::Value> = self
                            .db
                            .query("SELECT meta::id(id) as id FROM kg_entities WHERE name = $name AND data.entity_type = $etype LIMIT 1")
                            .bind(("name", name.clone()))
                            .bind(("etype", etype.clone()))
                            .await?
                            .take(0)?;
                        let final_id: String = if let Some(first) = found.first() {
                            first
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string()
                        } else {
                            let created: Vec<serde_json::Value> = self
                                .db
                                .query("CREATE kg_entities SET created_at = time::now(), name = $name, entity_type = $etype, data = $data RETURN meta::id(id) as id")
                                .bind(("name", name.clone()))
                                .bind(("etype", etype.clone()))
                                .bind(("data", data.clone()))
                                .await?
                                .take(0)?;
                            created
                                .first()
                                .and_then(|v| v.get("id"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string()
                        };
                        // Mark candidate approved
                        let _ = self
                            .db
                            .query("UPDATE type::thing($tb, $id) SET status = 'approved', reviewed_at = time::now(), feedback = $fb, promoted_id = $pid RETURN meta::id(id) as id")
                            .bind(("tb", "kg_observation_candidates"))
                            .bind(("id", id_s.clone()))
                            .bind(("fb", feedback_s.clone()))
                            .bind(("pid", final_id.clone()))
                            .await?;
                        // Embed new observation (check for existing not implemented in decide, assume new)
                        if let Err(e) = self
                            .ensure_kg_embedding("kg_observations", &final_id, &name, &data)
                            .await
                        {
                            tracing::warn!(
                                "kg_embedding: failed to auto-embed moderated observation {}: {}",
                                final_id,
                                e
                            );
                        }
                        results.push(json!({"id": id_s, "kind": "observation", "decision": "approved", "promoted_id": final_id}));
                    }
                }
                ("entity", "reject") => {
                    let _ = self
                        .db
                        .query("UPDATE type::thing($tb, $id) SET status = 'rejected', reviewed_at = time::now(), feedback = $fb RETURN meta::id(id) as id")
                        .bind(("tb", "kg_entity_candidates"))
                        .bind(("id", id_s.clone()))
                        .bind(("fb", feedback_s.clone()))
                        .await?;
                    results.push(json!({"id": id_s, "kind": "entity", "decision": "rejected"}));
                }
                ("relationship", "approve") => {
                    let row: Option<serde_json::Value> = self
                        .db
                        .query("SELECT * FROM type::thing($tb, $id) LIMIT 1")
                        .bind(("tb", "kg_edge_candidates"))
                        .bind(("id", id_s.clone()))
                        .await?
                        .take(0)?;
                    if let Some(candidate) = row {
                        let rel_type = candidate
                            .get("rel_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("related_to")
                            .to_string();
                        let data = candidate.get("data").cloned().unwrap_or(json!({}));
                        // Resolve source/target ids
                        let src_id_val = candidate.get("source_id").and_then(|v| v.as_str());
                        let dst_id_val = candidate.get("target_id").and_then(|v| v.as_str());
                        let src_name = candidate.get("source_name").and_then(|v| v.as_str());
                        let dst_name = candidate.get("target_name").and_then(|v| v.as_str());

                        let src_bare_opt = if let Some(cid) = canonical_id_opt.as_deref() {
                            self.resolve_entity_id_str(cid).await?
                        } else if let Some(idstr) = src_id_val {
                            self.resolve_entity_id_str(idstr).await?
                        } else if let Some(name) = src_name {
                            self.resolve_entity_id_str(name).await?
                        } else {
                            None
                        };

                        let dst_bare_opt = if let Some(idstr) = dst_id_val {
                            self.resolve_entity_id_str(idstr).await?
                        } else if let Some(name) = dst_name {
                            self.resolve_entity_id_str(name).await?
                        } else {
                            None
                        };

                        if let (Some(src_bare), Some(dst_bare)) = (src_bare_opt, dst_bare_opt) {
                            // Upsert or create edge
                            let found: Vec<serde_json::Value> = self
                                .db
                                .query("SELECT meta::id(id) as id FROM kg_edges WHERE source = type::thing('kg_entities', $src) AND target = type::thing('kg_entities', $dst) AND rel_type = $kind LIMIT 1")
                                .bind(("src", src_bare.clone()))
                                .bind(("dst", dst_bare.clone()))
                                .bind(("kind", rel_type.clone()))
                                .await?
                                .take(0)?;
                            let final_id: String = if let Some(first) = found.first() {
                                first
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string()
                            } else {
                                let created: Vec<serde_json::Value> = self
                                    .db
                                    .query("CREATE kg_edges SET created_at = time::now(), source = type::thing('kg_entities', $source), target = type::thing('kg_entities', $target), rel_type = $rel_type, data = $data RETURN meta::id(id) as id")
                                    .bind(("source", src_bare))
                                    .bind(("target", dst_bare))
                                    .bind(("rel_type", rel_type.clone()))
                                    .bind(("data", data.clone()))
                                    .await?
                                    .take(0)?;
                                created
                                    .first()
                                    .and_then(|v| v.get("id"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string()
                            };

                            // Mark candidate approved
                            let _ = self
                                .db
                                .query("UPDATE type::thing($tb, $id) SET status = 'approved', reviewed_at = time::now(), feedback = $fb, promoted_id = $pid RETURN meta::id(id) as id")
                                .bind(("tb", "kg_edge_candidates"))
                                .bind(("id", id_s.clone()))
                                .bind(("fb", feedback_s.clone()))
                                .bind(("pid", final_id.clone()))
                                .await?;
                            results.push(json!({"id": id_s, "kind": "relationship", "decision": "approved", "promoted_id": final_id}));
                        } else {
                            return Err(SurrealMindError::Validation {
                                message: format!(
                                    "Could not resolve entity IDs for candidate edge {}",
                                    id_s
                                ),
                            });
                        }
                    }
                }
                ("relationship", "reject") => {
                    let _ = self
                        .db
                        .query("UPDATE type::thing($tb, $id) SET status = 'rejected', reviewed_at = time::now(), feedback = $fb RETURN meta::id(id) as id")
                        .bind(("tb", "kg_edge_candidates"))
                        .bind(("id", id_s.clone()))
                        .bind(("fb", feedback_s.clone()))
                        .await?;
                    results
                        .push(json!({"id": id_s, "kind": "relationship", "decision": "rejected"}));
                }
                _ => {
                    return Err(SurrealMindError::Validation {
                        message: format!(
                            "Unsupported decision: kind='{}' decision='{}'",
                            kind_s, decision_s
                        ),
                    });
                }
            }
        }

        Ok(CallToolResult::structured(json!({"results": results})))
    }

    /// Helper: resolve an entity ref to its bare meta id string (no Thing types used in Rust).
    /// Accepts full Thing strings, bare IDs, or names.
    async fn resolve_entity_id_str(&self, entity: &str) -> Result<Option<String>> {
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

    /// Normalize entity name for similarity comparison
    fn normalize_entity_name(name: &str) -> String {
        name.to_lowercase()
            .replace("'s", "")
            .replace("'", "")
            .replace("-", " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Calculate similarity score between two entity names
    fn calculate_name_similarity(name1: &str, name2: &str) -> f32 {
        let norm1 = Self::normalize_entity_name(name1);
        let norm2 = Self::normalize_entity_name(name2);
        strsim::jaro_winkler(&norm1, &norm2) as f32
    }

    /// Find similar entities for canonical suggestions
    async fn find_similar_entities(
        &self,
        name: &str,
        entity_type: &str,
        limit: usize,
    ) -> Result<Vec<(String, String, f32)>> {
        let normalized = Self::normalize_entity_name(name);

        let query = "
            SELECT meta::id(id) as id, name
            FROM kg_entities
            WHERE entity_type = $entity_type
            AND (data.is_alias IS NONE OR data.is_alias = false)
            LIMIT 20
        ";

        let rows: Vec<serde_json::Value> = self
            .db
            .query(query)
            .bind(("entity_type", entity_type.to_string()))
            .await?
            .take(0)?;

        let threshold = self.config.retrieval.kg_moderation_threshold;
        let mut candidates: Vec<(String, String, f32)> = rows
            .into_iter()
            .filter_map(|row| {
                let id = row.get("id")?.as_str()?.to_string();
                let entity_name = row.get("name")?.as_str()?.to_string();
                let similarity = Self::calculate_name_similarity(&normalized, &entity_name);
                if similarity > threshold {
                    // Only include reasonably similar matches
                    Some((id, entity_name, similarity))
                } else {
                    None
                }
            })
            .collect();

        candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(limit);

        Ok(candidates)
    }
}
