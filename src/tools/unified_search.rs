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

    // Debug logging for chain_id search
    if let Some(ref cid) = params.chain_id {
        tracing::info!("🔍 Unified search requested with chain_id: {}", cid);
    }

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
    if let Some(q) = &params.query
        && let Some(n) = q.get("name").and_then(|v| v.as_str())
        && !n.is_empty()
    {
        name_like = Some(n.to_string());
    }

    // Determine content for embedding
    let mut content = params.thoughts_content.clone().unwrap_or_default();
    if content.is_empty()
        && let Some(qjson) = &params.query
        && let Some(text) = qjson.get("text").and_then(|v| v.as_str())
        && !text.is_empty()
    {
        content = text.to_string();
    }
    if content.is_empty()
        && let Some(ref nl) = name_like
    {
        content = nl.clone();
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

    // Helper for chain_id predicate (used in multiple places)
    let chain_filter_sql = |field_prefix: &str| -> String {
        format!(
            "({prefix}source_thought_id IN (SELECT value id FROM thoughts WHERE chain_id = $cid) \
            OR {prefix}staged_by_thought IN (SELECT value id FROM thoughts WHERE chain_id = $cid) \
            OR source_thought_ids CONTAINSANY (SELECT value id FROM thoughts WHERE chain_id = $cid))",
            prefix = field_prefix
        )
    };

    if target == "entity" || target == "mixed" {
        // Flag to track if we found anything via semantic search
        let mut found_semantic = false;

        if let Some(ref q_emb_val) = q_emb {
            // Semantic search using embeddings
            let q_dim = q_emb_val.len() as i64;
            // UPDATED: Order by similarity DESC (not created_at) for semantic search
            let mut sql = "SELECT meta::id(id) as id, name, data, created_at, vector::similarity::cosine(embedding, $q) AS similarity
                 FROM kg_entities WHERE embedding_dim = $dim AND embedding IS NOT NULL".to_string();

            if params.chain_id.is_some() {
                sql.push_str(" AND ");
                sql.push_str(&chain_filter_sql("data."));
            }

            sql.push_str(&format!(
                " ORDER BY similarity DESC LIMIT {}",
                top_k_mem * 3
            ));

            #[derive(Debug, serde::Deserialize)]
            struct EntityRow {
                id: String,
                name: String,
                data: serde_json::Value,
                created_at: serde_json::Value,
                similarity: Option<f32>,
            }

            let mut query = server
                .db
                .query(sql)
                .bind(("dim", q_dim))
                .bind(("q", q_emb_val.clone()));
            if let Some(ref cid) = params.chain_id {
                query = query.bind(("cid", cid.clone()));
            }
            let rows: Vec<EntityRow> = query.await?.take(0)?;

            let mut scored_entities: Vec<serde_json::Value> = Vec::new();
            for row in rows {
                let similarity = row.similarity;
                if let Some(sim) = similarity
                    && sim >= sim_thresh
                {
                    let entity_json = json!({
                        "id": row.id,
                        "kind": "entity",
                        "name": row.name,
                        "data": row.data,
                        "created_at": row.created_at,
                        "similarity": sim
                    });
                    scored_entities.push(entity_json);
                }
            }

            if !scored_entities.is_empty() {
                // Sort by similarity descending before truncating
                sort_by_similarity(&mut scored_entities);
                scored_entities.truncate(top_k_mem);
                items.extend(scored_entities);
                found_semantic = true;
            }
        }

        // Fallback or Non-Semantic Search
        if !found_semantic {
            if let Some(ref nl) = name_like {
                // Fallback to name pattern matching when no embedding available
                let mut sql =
                    "SELECT meta::id(id) as id, name, data, created_at FROM kg_entities WHERE name ~ $name"
                        .to_string();
                if params.chain_id.is_some() {
                    sql.push_str(" AND ");
                    sql.push_str(&chain_filter_sql("data."));
                }
                sql.push_str(&format!(" LIMIT {}", top_k_mem));
                let mut query = server.db.query(sql).bind(("name", nl.clone()));
                if let Some(ref cid) = params.chain_id {
                    query = query.bind(("cid", cid.clone()));
                }
                let rows: Vec<serde_json::Value> = query.await?.take(0)?;

                // Inject kind field
                items.extend(rows.into_iter().map(|mut v| {
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("kind".to_string(), json!("entity"));
                        obj.insert("similarity".to_string(), json!(0.0));
                    }
                    v
                }));
            } else {
                // Fallback to recent items when no query or embedding
                let mut sql = "SELECT meta::id(id) as id, name, data, created_at FROM kg_entities"
                    .to_string();
                if params.chain_id.is_some() {
                    sql.push_str(" WHERE ");
                    sql.push_str(&chain_filter_sql("data."));
                }
                sql.push_str(&format!(" ORDER BY created_at DESC LIMIT {}", top_k_mem));
                let mut query = server.db.query(sql);
                #[allow(unused_variables)]
                if let Some(ref cid) = params.chain_id {
                    query = query.bind(("cid", cid.clone()));
                }
                let rows: Vec<serde_json::Value> = query.await?.take(0)?;

                // Inject kind field
                items.extend(rows.into_iter().map(|mut v| {
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("kind".to_string(), json!("entity"));
                        obj.insert("similarity".to_string(), json!(0.0));
                    }
                    v
                }));
            }
        }
    }
    if target == "relationship" || target == "mixed" {
        let mut sql = "SELECT meta::id(id) as id,
                    (IF type::is::record(source) THEN meta::id(source) ELSE string::concat(source) END) as source_id,
                    (IF type::is::record(target) THEN meta::id(target) ELSE string::concat(target) END) as target_id,
                    rel_type, data, created_at
             FROM kg_edges".to_string();
        if params.chain_id.is_some() {
            sql.push_str(" WHERE ");
            sql.push_str(&chain_filter_sql("data."));
        }
        sql.push_str(&format!(" ORDER BY created_at DESC LIMIT {}", top_k_mem));
        let mut query = server.db.query(sql);
        #[allow(unused_variables)]
        if let Some(ref cid) = params.chain_id {
            query = query.bind(("cid", cid.clone()));
        }
        let rows: Vec<serde_json::Value> = query.await?.take(0)?;

        items.extend(rows.into_iter().map(|mut v| {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("kind".to_string(), json!("relationship"));
            }
            v
        }));
    }
    if target == "observation" || target == "mixed" {
        let mut found_semantic_obs = false;

        if let Some(ref q_emb_val) = q_emb {
            // Semantic search using embeddings
            let q_dim = q_emb_val.len() as i64;
            let mut sql = "SELECT meta::id(id) as id, name, data, created_at, vector::similarity::cosine(embedding, $q) AS similarity
                 FROM kg_observations WHERE embedding_dim = $dim AND embedding IS NOT NULL".to_string();

            if params.chain_id.is_some() {
                sql.push_str(" AND ");
                sql.push_str(&chain_filter_sql("")); // root level source_thought_id
            }

            sql.push_str(&format!(
                " ORDER BY similarity DESC LIMIT {}",
                top_k_mem * 3
            ));

            #[derive(Debug, serde::Deserialize)]
            struct ObservationRow {
                id: String,
                name: String,
                data: serde_json::Value,
                created_at: serde_json::Value,
                similarity: Option<f32>,
            }

            let mut query = server
                .db
                .query(sql)
                .bind(("dim", q_dim))
                .bind(("q", q_emb_val.clone()));
            if let Some(ref cid) = params.chain_id {
                query = query.bind(("cid", cid.clone()));
            }
            let rows: Vec<ObservationRow> = query.await?.take(0)?;

            let mut scored_observations: Vec<serde_json::Value> = Vec::new();
            for row in rows {
                let similarity = row.similarity;
                if let Some(sim) = similarity
                    && sim >= sim_thresh
                {
                    let observation_json = json!({
                        "id": row.id,
                        "kind": "observation",
                        "name": row.name,
                        "data": row.data,
                        "created_at": row.created_at,
                        "similarity": sim
                    });
                    scored_observations.push(observation_json);
                }
            }

            if !scored_observations.is_empty() {
                // Sort by similarity descending before truncating
                sort_by_similarity(&mut scored_observations);
                scored_observations.truncate(top_k_mem);
                items.extend(scored_observations);
                found_semantic_obs = true;
            }
        }

        if !found_semantic_obs {
            if let Some(ref nl) = name_like {
                // Fallback to name pattern matching when no embedding available
                let mut sql = "SELECT meta::id(id) as id, name, data, created_at FROM kg_observations WHERE name ~ $name".to_string();
                if params.chain_id.is_some() {
                    sql.push_str(" AND ");
                    sql.push_str(&chain_filter_sql(""));
                }
                sql.push_str(&format!(" LIMIT {}", top_k_mem));
                let mut query = server.db.query(sql).bind(("name", nl.clone()));
                if let Some(ref cid) = params.chain_id {
                    query = query.bind(("cid", cid.clone()));
                }
                let rows: Vec<serde_json::Value> = query.await?.take(0)?;

                items.extend(rows.into_iter().map(|mut v| {
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("kind".to_string(), json!("observation"));
                        obj.insert("similarity".to_string(), json!(0.0));
                    }
                    v
                }));
            } else {
                // Fallback to recent items
                let mut sql =
                    "SELECT meta::id(id) as id, name, data, created_at FROM kg_observations"
                        .to_string();
                if params.chain_id.is_some() {
                    sql.push_str(" WHERE ");
                    sql.push_str(&chain_filter_sql(""));
                }
                sql.push_str(&format!(" ORDER BY created_at DESC LIMIT {}", top_k_mem));
                let mut query = server.db.query(sql);
                #[allow(unused_variables)]
                if let Some(ref cid) = params.chain_id {
                    query = query.bind(("cid", cid.clone()));
                }
                let rows: Vec<serde_json::Value> = query.await?.take(0)?;

                items.extend(rows.into_iter().map(|mut v| {
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("kind".to_string(), json!("observation"));
                        obj.insert("similarity".to_string(), json!(0.0));
                    }
                    v
                }));
            }
        }
    }

    let mut out = serde_json::Map::new();
    out.insert("memories".into(), json!({"items": items}));
    tracing::debug!("🔍 Unified search found {} memory items", items.len());

    // 2) Thoughts search (optional)
    if include_thoughts {
        // Decide query text for thoughts
        let mut content = params.thoughts_content.clone().unwrap_or_default();
        if content.is_empty()
            && let Some(qjson) = &params.query
            && let Some(text) = qjson.get("text").and_then(|v| v.as_str())
            && !text.is_empty()
        {
            content = text.to_string();
        }
        if content.is_empty()
            && let Some(ref nl) = name_like
        {
            content = nl.clone();
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

        // Debug logging for thoughts search
        tracing::debug!(
            "🔍 Thoughts search with chain_id present: {}",
            params.chain_id.is_some()
        );

        // Build WHERE clauses - only require embeddings if doing semantic search
        let mut where_clauses = vec![];
        if q_emb.is_some() {
            where_clauses.push("embedding_dim = $dim AND embedding IS NOT NULL".to_string());
        }
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
        let where_sql = if where_clauses.is_empty() {
            "true".to_string()
        } else {
            where_clauses.join(" AND ")
        };
        let sql = format!(
            "SELECT {} FROM thoughts WHERE {} ORDER BY {} LIMIT $k",
            select_fields, where_sql, order_by
        );

        // Debug the thoughts query
        tracing::info!("🔍 Thoughts SQL: {}", sql);
        tracing::info!("🔍 Thoughts binds: {:?}", binds);
        tracing::info!(
            "🔍 Thoughts has_query: {}, chain_id: {:?}",
            has_query,
            params.chain_id
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

/// Helper function to sort entities by similarity (used by both production and tests)
fn sort_by_similarity(entities: &mut [serde_json::Value]) {
    entities.sort_by(|a, b| {
        let sim_a = a.get("similarity").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let sim_b = b.get("similarity").and_then(|v| v.as_f64()).unwrap_or(0.0);
        sim_b
            .partial_cmp(&sim_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_similarity_ordering_keeps_high_similarity_old_items() {
        // Create test entities with varying similarities and ages
        let mut scored_entities = vec![
            json!({
                "id": "old-high-sim",
                "name": "Old but high similarity",
                "created_at": "2023-01-01T00:00:00Z",
                "similarity": 0.95
            }),
            json!({
                "id": "new-low-sim",
                "name": "New but low similarity",
                "created_at": "2025-01-01T00:00:00Z",
                "similarity": 0.60
            }),
            json!({
                "id": "mid-mid-sim",
                "name": "Middle age and similarity",
                "created_at": "2024-06-01T00:00:00Z",
                "similarity": 0.75
            }),
            json!({
                "id": "newer-high-sim",
                "name": "Newer high similarity",
                "created_at": "2025-06-01T00:00:00Z",
                "similarity": 0.92
            }),
            json!({
                "id": "oldest-med-sim",
                "name": "Oldest medium similarity",
                "created_at": "2022-01-01T00:00:00Z",
                "similarity": 0.70
            }),
        ];

        // Use the actual production sorting function
        sort_by_similarity(&mut scored_entities);

        // Truncate to top 3
        scored_entities.truncate(3);

        // Verify the top 3 are the highest similarity ones regardless of age
        let ids: Vec<&str> = scored_entities
            .iter()
            .map(|e| e.get("id").unwrap().as_str().unwrap())
            .collect();

        assert_eq!(ids[0], "old-high-sim"); // 0.95 - oldest but highest similarity
        assert_eq!(ids[1], "newer-high-sim"); // 0.92 - newer, second highest
        assert_eq!(ids[2], "mid-mid-sim"); // 0.75 - middle age, third highest

        // Verify that new-low-sim (0.60) and oldest-med-sim (0.70) were dropped
        assert_eq!(scored_entities.len(), 3);
    }
}
