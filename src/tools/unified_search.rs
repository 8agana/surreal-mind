//! Unified search over memories (default) and optional thoughts

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use chrono::NaiveDate;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct UnifiedSearchParams {
    #[serde(default)]
    pub query: Option<serde_json::Value>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub include_thoughts: Option<bool>,
    #[serde(default)]
    pub thoughts_content: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_usize_forgiving"
    )]
    pub top_k_memories: Option<usize>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_usize_forgiving"
    )]
    pub top_k_thoughts: Option<usize>,
    #[serde(default)]
    pub sim_thresh: Option<f32>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub chain_id: Option<String>,
    #[serde(default)]
    pub previous_thought_id: Option<String>,
    #[serde(default)]
    pub revises_thought: Option<String>,
    #[serde(default)]
    pub branch_from: Option<String>,
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(default)]
    pub confidence_gte: Option<f32>,
    #[serde(default)]
    pub confidence_lte: Option<f32>,
    #[serde(default)]
    pub date_from: Option<String>,
    #[serde(default)]
    pub date_to: Option<String>,
    #[serde(default)]
    pub order: Option<String>,
}

#[derive(Debug, Serialize)]
struct ThoughtOut {
    id: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    similarity: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    significance: Option<f32>,
}

impl SurrealMindServer {
    /// LegacyMind unified search handler (current DB)
    pub async fn handle_unified_search(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        unified_search_inner(self, request).await
    }
}

pub async fn unified_search_inner(
    server: &SurrealMindServer,
    request: CallToolRequestParam,
) -> Result<CallToolResult> {
    let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
        message: "Missing parameters".into(),
    })?;
    let params: UnifiedSearchParams = serde_json::from_value(serde_json::Value::Object(args))
        .map_err(|e| SurrealMindError::Serialization {
            message: format!("Invalid parameters: {}", e),
        })?;

    let target = params.target.unwrap_or_else(|| "mixed".to_string());
    let include_thoughts = params.include_thoughts.unwrap_or(false);
    let top_k_mem = params.top_k_memories.unwrap_or(10).clamp(1, 50);
    let top_k_th = params.top_k_thoughts.unwrap_or(5).clamp(1, 50);
    let sim_thresh = params.sim_thresh.unwrap_or(0.0).clamp(0.0, 1.0);

    // Clamp confidence bounds to [0,1]
    let confidence_gte = params.confidence_gte.map(|v| v.clamp(0.0, 1.0));
    let confidence_lte = params.confidence_lte.map(|v| v.clamp(0.0, 1.0));

    // Parse date bounds
    let date_from_bound = params
        .date_from
        .as_ref()
        .map(|d| format!("{}T00:00:00Z", d));
    let date_to_bound = params.date_to.as_ref().map(|d| format!("{}T23:59:59Z", d));

    // Validate date range if both provided
    if let (Some(df), Some(dt)) = (&params.date_from, &params.date_to) {
        let from_date = NaiveDate::parse_from_str(df, "%Y-%m-%d").map_err(|_| {
            SurrealMindError::Serialization {
                message: "Invalid date_from format (expected YYYY-MM-DD)".into(),
            }
        })?;
        let to_date = NaiveDate::parse_from_str(dt, "%Y-%m-%d").map_err(|_| {
            SurrealMindError::Serialization {
                message: "Invalid date_to format (expected YYYY-MM-DD)".into(),
            }
        })?;
        if from_date > to_date {
            return Err(SurrealMindError::Serialization {
                message: "date_from cannot be after date_to".into(),
            });
        }
    }

    // Build a simple name-like predicate from query if available
    let mut name_like: Option<String> = None;
    if let Some(q) = &params.query {
        if let Some(n) = q.get("name").and_then(|v| v.as_str()) {
            if !n.is_empty() {
                name_like = Some(n.to_string());
            }
        }
    }

    // Determine content for embedding
    let mut content = params.thoughts_content.clone().unwrap_or_default();
    if content.is_empty() {
        if let Some(qjson) = &params.query {
            if let Some(text) = qjson.get("text").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    content = text.to_string();
                }
            }
        }
    }
    if content.is_empty() {
        if let Some(ref nl) = name_like {
            content = nl.clone();
        }
    }
    let has_query = !content.is_empty();
    let q_emb = if has_query {
        match server.embedder.embed(&content).await {
            Ok(emb) => Some(emb),
            Err(e) => {
                tracing::warn!(
                    "Embedding failed for query {} : {}, falling back to non-semantic search",
                    content,
                    e
                );
                None
            }
        }
    } else {
        None
    };
    // 1) Memories search: entities/relationships/observations as requested
    let mut items: Vec<serde_json::Value> = Vec::new();
    if target == "entity" || target == "mixed" {
        if let Some(ref q_emb_val) = q_emb {
            // Semantic search using embeddings
            let q_dim = q_emb_val.len() as i64;
            let sql = format!(
                "SELECT meta::id(id) as id, name, data, created_at, vector::similarity::cosine(embedding, $q) AS similarity
                 FROM kg_entities WHERE embedding_dim = $dim AND embedding IS NOT NULL ORDER BY created_at DESC
                 LIMIT {}",
                top_k_mem * 3 // Get more candidates for similarity filtering
            );

            #[derive(Debug, serde::Deserialize)]
            struct EntityRow {
                id: String,
                name: String,
                data: serde_json::Value,
                created_at: serde_json::Value,
                similarity: Option<f32>,
            }

            let rows: Vec<EntityRow> = server
                .db
                .query(sql)
                .bind(("dim", q_dim))
                .bind(("q", q_emb_val.clone()))
                .await?
                .take(0)?;

            let mut scored_entities: Vec<serde_json::Value> = Vec::new();
            for row in rows {
                let similarity = row.similarity;
                if let Some(sim) = similarity {
                    if sim >= sim_thresh {
                        let entity_json = json!({"id": row.id, "name": row.name, "data": row.data, "created_at": row.created_at, "similarity": sim});
                        scored_entities.push(entity_json);
                    }
                }
            }
            scored_entities.truncate(top_k_mem);
            items.extend(scored_entities);
        }
    } else if let Some(ref nl) = name_like {
        // Fallback to name pattern matching when no embedding available
        let sql = format!(
            "SELECT meta::id(id) as id, name, data, created_at FROM kg_entities WHERE name ~ $name LIMIT {}",
            top_k_mem
        );
        let rows: Vec<serde_json::Value> = server
            .db
            .query(sql)
            .bind(("name", nl.clone()))
            .await?
            .take(0)?;
        items.extend(rows);
    } else {
        // Fallback to recent items when no query or embedding
        let sql = format!(
            "SELECT meta::id(id) as id, name, data, created_at FROM kg_entities LIMIT {}",
            top_k_mem
        );
        let rows: Vec<serde_json::Value> = server.db.query(sql).await?.take(0)?;
        items.extend(rows);
    }
    if target == "relationship" || target == "mixed" {
        let sql = format!(
            "SELECT meta::id(id) as id,
                    (IF type::is::record(source) THEN meta::id(source) ELSE string::concat(source) END) as source_id,
                    (IF type::is::record(target) THEN meta::id(target) ELSE string::concat(target) END) as target_id,
                    rel_type, data, created_at
             FROM kg_edges ORDER BY created_at DESC LIMIT {}",
            top_k_mem
        );
        let rows: Vec<serde_json::Value> = server.db.query(sql).await?.take(0)?;
        items.extend(rows);
    }
    if target == "observation" || target == "mixed" {
        if let Some(ref q_emb_val) = q_emb {
            // Semantic search using embeddings
            let q_dim = q_emb_val.len() as i64;
            let sql = format!(
                "SELECT meta::id(id) as id, name, data, created_at, vector::similarity::cosine(embedding, $q) AS similarity
                 FROM kg_observations WHERE embedding_dim = $dim AND embedding IS NOT NULL ORDER BY created_at DESC
                 LIMIT {}",
                top_k_mem * 3 // Get more candidates for similarity filtering
            );

            #[derive(Debug, serde::Deserialize)]
            struct ObservationRow {
                id: String,
                name: String,
                data: serde_json::Value,
                created_at: serde_json::Value,
                similarity: Option<f32>,
            }

            let rows: Vec<ObservationRow> = server
                .db
                .query(sql)
                .bind(("dim", q_dim))
                .bind(("q", q_emb_val.clone()))
                .await?
                .take(0)?;

            let mut scored_observations: Vec<serde_json::Value> = Vec::new();
            for row in rows {
                let similarity = row.similarity;
                if let Some(sim) = similarity {
                    if sim >= sim_thresh {
                        let observation_json = json!({ "id": row.id, "name": row.name, "data": row.data, "created_at": row.created_at, "similarity": sim });
                        scored_observations.push(observation_json);
                    }
                }
            }

            scored_observations.truncate(top_k_mem);
            items.extend(scored_observations);
        } else if let Some(ref nl) = name_like {
            // Fallback to name pattern matching when no embedding available
            let sql = format!(
                "SELECT meta::id(id) as id, name, data, created_at FROM kg_observations WHERE name ~ $name LIMIT {}",
                top_k_mem
            );
            let rows: Vec<serde_json::Value> = server
                .db
                .query(sql)
                .bind(("name", nl.clone()))
                .await?
                .take(0)?;
            items.extend(rows);
        } else {
            // Fallback to recent items when no query or embedding
            let sql = format!(
                "SELECT meta::id(id) as id, name, data, created_at FROM kg_observations LIMIT {}",
                top_k_mem
            );
            let rows: Vec<serde_json::Value> = server.db.query(sql).await?.take(0)?;
            items.extend(rows);
        }
    }

    let mut out = serde_json::Map::new();
    out.insert("memories".into(), json!({"items": items}));

    // 2) Thoughts search (optional)
    if include_thoughts {
        // Decide query text for thoughts
        let mut content = params.thoughts_content.clone().unwrap_or_default();
        if content.is_empty() {
            // Prefer explicit text from query if available (common client pattern)
            if let Some(qjson) = &params.query {
                if let Some(text) = qjson.get("text").and_then(|v| v.as_str()) {
                    if !text.is_empty() {
                        content = text.to_string();
                    }
                }
            }
        }
        if content.is_empty() {
            if let Some(ref nl) = name_like {
                content = nl.clone();
            }
        }
        let has_query = !content.is_empty();
        let q_emb = if has_query {
            Some(server.embedder.embed(&content).await.map_err(|e| {
                SurrealMindError::Embedding {
                    message: e.to_string(),
                }
            })?)
        } else {
            None
        };

        // Build WHERE clauses
        let mut where_clauses = vec!["embedding_dim = $dim AND embedding IS NOT NULL".to_string()];
        let mut binds = serde_json::Map::new();

        if let Some(sid) = &params.session_id {
            where_clauses.push("session_id = $sid".to_string());
            binds.insert("sid".to_string(), json!(sid));
        }
        if let Some(cid) = &params.chain_id {
            where_clauses.push("chain_id = $cid".to_string());
            binds.insert("cid".to_string(), json!(cid));
        }
        if let Some(prev) = &params.previous_thought_id {
            where_clauses.push("((type::is::record(previous_thought_id) AND meta::id(previous_thought_id) = $prev) OR previous_thought_id = $prev)".to_string());
            binds.insert("prev".to_string(), json!(prev));
        }
        if let Some(rev) = &params.revises_thought {
            where_clauses.push("((type::is::record(revises_thought) AND meta::id(revises_thought) = $rev) OR revises_thought = $rev)".to_string());
            binds.insert("rev".to_string(), json!(rev));
        }
        if let Some(br) = &params.branch_from {
            where_clauses.push("((type::is::record(branch_from) AND meta::id(branch_from) = $br) OR branch_from = $br)".to_string());
            binds.insert("br".to_string(), json!(br));
        }
        if let Some(origin) = &params.origin {
            where_clauses.push("origin = $origin".to_string());
            binds.insert("origin".to_string(), json!(origin));
        }
        if let Some(cgte) = confidence_gte {
            where_clauses.push("confidence IS NOT NULL AND confidence >= $cgte".to_string());
            binds.insert("cgte".to_string(), json!(cgte));
        }
        if let Some(clte) = confidence_lte {
            where_clauses.push("confidence IS NOT NULL AND confidence <= $clte".to_string());
            binds.insert("clte".to_string(), json!(clte));
        }
        if let Some(df) = &date_from_bound {
            where_clauses.push("created_at >= $from_date".to_string());
            binds.insert("from_date".to_string(), json!(df));
        }
        if let Some(dt) = &date_to_bound {
            where_clauses.push("created_at <= $to_date".to_string());
            binds.insert("to_date".to_string(), json!(dt));
        }

        // Add similarity filter if query present
        if q_emb.is_some() {
            where_clauses.push("vector::similarity::cosine(embedding, $q) > $sim".to_string());
        }

        // Build ORDER BY
        let has_continuity = params.session_id.is_some() || params.chain_id.is_some();
        let order_by = if has_continuity && params.order.is_none() {
            if q_emb.is_some() {
                "created_at ASC, similarity DESC"
            } else {
                "created_at ASC"
            }
        } else if let Some(order) = &params.order {
            match order.as_str() {
                "created_at_asc" => "created_at ASC",
                "created_at_desc" => "created_at DESC",
                _ => "similarity DESC", // fallback
            }
        } else if q_emb.is_some() {
            "similarity DESC"
        } else {
            "created_at DESC" // fallback if no query and no order
        };

        // Build SELECT
        let select_fields = if q_emb.is_some() {
            // Include created_at in projection to satisfy SurrealDB 2.x ORDER BY requirements
            "meta::id(id) as id, content, significance, created_at, vector::similarity::cosine(embedding, $q) AS similarity"
        } else {
            // Always project created_at if used for ordering
            "meta::id(id) as id, content, significance, created_at"
        };
        let sql = format!(
            "SELECT {} FROM thoughts WHERE {} ORDER BY {} LIMIT $k",
            select_fields,
            where_clauses.join(" AND "),
            order_by
        );

        let mut query = server.db.query(sql).bind(("k", top_k_th as i64));
        if let Some(ref q_emb_val) = q_emb {
            query = query.bind(("q", q_emb_val.clone()));
            query = query.bind(("sim", sim_thresh));
        }
        let q_dim = if let Some(ref q_emb_val) = q_emb {
            q_emb_val.len() as i64
        } else {
            server.embedder.dimensions() as i64
        };
        query = query.bind(("dim", q_dim));
        for (k, v) in binds {
            query = query.bind((k, v));
        }
        let mut resp = query.await?;

        #[derive(Debug, Deserialize)]
        struct Row {
            id: String,
            content: String,
            #[serde(default)]
            significance: f32,
            #[serde(default)]
            similarity: Option<f32>,
        }
        let rows: Vec<Row> = resp.take(0)?;
        let results: Vec<ThoughtOut> = rows
            .into_iter()
            .map(|r| ThoughtOut {
                id: r.id,
                content: r.content,
                similarity: r.similarity,
                significance: Some(r.significance),
            })
            .collect();
        out.insert(
            "thoughts".into(),
            json!({
                "total": results.len(),
                "top_k": top_k_th,
                "results": results
            }),
        );
    }

    Ok(CallToolResult::structured(serde_json::Value::Object(out)))
}
