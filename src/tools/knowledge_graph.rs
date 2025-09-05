//! Knowledge graph tool handlers for creating and searching entities/relationships

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;
// use std::str::FromStr; // no longer needed
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
                                entity.as_object_mut().unwrap().insert(
                                    "canonical_suggestions".to_string(),
                                    json!({"suggestions": suggestions_json}),
                                );
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
                                    .bind(("name", name))
                                    .bind(("etype", etype))
                                    .bind(("data", data))
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
                                    .query("UPDATE type::thing($tb, $id) SET status = 'aliased', reviewed_at = time::now(), feedback = $fb, promoted_id = $pid RETURN meta::id(id) as id")
                                    .bind(("tb", "kg_entity_candidates"))
                                    .bind(("id", id_s.clone()))
                                    .bind(("fb", feedback_s.clone()))
                                    .bind(("pid", final_id.clone()))
                                    .await?;

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

    // Note: resolve_entity_id_str is defined once in kg_create.rs and reused here.

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

        let mut candidates: Vec<(String, String, f32)> = rows
            .into_iter()
            .filter_map(|row| {
                let id = row.get("id")?.as_str()?.to_string();
                let entity_name = row.get("name")?.as_str()?.to_string();
                let similarity = Self::calculate_name_similarity(&normalized, &entity_name);
                if similarity > 0.6 {
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
