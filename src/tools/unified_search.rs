//! Unified search over memories (default) and optional thoughts

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
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
    // Continuity field filters (Phase B+)
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub chain_id: Option<String>,
    #[serde(default)]
    pub confidence_gte: Option<f32>,
    #[serde(default)]
    pub confidence_lte: Option<f32>,
    #[serde(default)]
    pub order: Option<String>, // "created_at_asc" | "created_at_desc"
}

#[derive(Debug, Serialize)]
struct ThoughtOut {
    id: String,
    content: String,
    similarity: f32,
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
    let sim_thresh = params.sim_thresh.unwrap_or(0.3).clamp(0.0, 1.0);

    // Build a simple name-like predicate from query if available
    let mut name_like: Option<String> = None;
    if let Some(q) = &params.query {
        if let Some(n) = q.get("name").and_then(|v| v.as_str()) {
            if !n.is_empty() {
                name_like = Some(n.to_string());
            }
        }
    }

    // 1) Memories search: entities/relationships/observations as requested
    let mut items: Vec<serde_json::Value> = Vec::new();
    if target == "entity" || target == "mixed" {
        let sql = if let Some(ref _nl) = name_like {
            format!(
                "SELECT meta::id(id) as id, name, data, created_at FROM kg_entities WHERE name ~ $name LIMIT {}",
                top_k_mem
            )
        } else {
            format!(
                "SELECT meta::id(id) as id, name, data, created_at FROM kg_entities LIMIT {}",
                top_k_mem
            )
        };
        let mut q = server.db.query(sql);
        if let Some(ref nl) = name_like {
            q = q.bind(("name", nl.clone()));
        }
        let rows: Vec<serde_json::Value> = q.await?.take(0)?;
        items.extend(rows);
    }
    if target == "relationship" || target == "mixed" {
        let sql = format!(
            "SELECT meta::id(id) as id,
                    (IF type::is::record(source) THEN meta::id(source) ELSE string::concat(source) END) as source_id,
                    (IF type::is::record(target) THEN meta::id(target) ELSE string::concat(target) END) as target_id,
                    rel_type, data, created_at
             FROM kg_edges LIMIT {}",
            top_k_mem
        );
        let rows: Vec<serde_json::Value> = server.db.query(sql).await?.take(0)?;
        items.extend(rows);
    }
    if target == "observation" || target == "mixed" {
        let sql = if let Some(ref _nl) = name_like {
            format!(
                "SELECT meta::id(id) as id, name, data, created_at FROM kg_observations WHERE name ~ $name LIMIT {}",
                top_k_mem
            )
        } else {
            format!(
                "SELECT meta::id(id) as id, name, data, created_at FROM kg_observations LIMIT {}",
                top_k_mem
            )
        };
        let mut q = server.db.query(sql);
        if let Some(ref nl) = name_like {
            q = q.bind(("name", nl.clone()));
        }
        let rows: Vec<serde_json::Value> = q.await?.take(0)?;
        items.extend(rows);
    }

    let mut out = serde_json::Map::new();
    out.insert("memories".into(), json!({"items": items}));

    // 2) Thoughts search (optional)
    if include_thoughts {
        // Decide query text for thoughts
        let mut content = params.thoughts_content.unwrap_or_default();
        if content.is_empty() {
            if let Some(ref nl) = name_like {
                content = nl.clone();
            }
        }
        if !content.is_empty() {
            let q_emb =
                server
                    .embedder
                    .embed(&content)
                    .await
                    .map_err(|e| SurrealMindError::Embedding {
                        message: e.to_string(),
                    })?;
            let q_dim = q_emb.len() as i64;
            
            // Build WHERE clause with continuity filters
            let mut where_clauses = vec![
                "embedding_dim = $dim".to_string(),
                "embedding IS NOT NULL".to_string(),
                "vector::similarity::cosine(embedding, $q) > $sim".to_string(),
            ];
            
            // Add continuity field filters if present
            if params.session_id.is_some() {
                where_clauses.push("session_id = $session_id".to_string());
            }
            if params.chain_id.is_some() {
                where_clauses.push("chain_id = $chain_id".to_string());
            }
            
            // Add confidence bounds (clamp to [0,1])
            if let Some(conf_gte) = params.confidence_gte {
                let clamped = conf_gte.clamp(0.0, 1.0);
                where_clauses.push(format!("confidence >= {}", clamped));
            }
            if let Some(conf_lte) = params.confidence_lte {
                let clamped = conf_lte.clamp(0.0, 1.0);
                where_clauses.push(format!("confidence <= {}", clamped));
            }
            
            let where_clause = where_clauses.join(" AND ");
            
            // Determine ordering: if session/chain present and no explicit order, use ASC
            let order_by = if let Some(ref order) = params.order {
                match order.as_str() {
                    "created_at_asc" => "ORDER BY created_at ASC",
                    "created_at_desc" => "ORDER BY created_at DESC",
                    _ => "ORDER BY similarity DESC", // fallback to similarity
                }
            } else if params.session_id.is_some() || params.chain_id.is_some() {
                "ORDER BY created_at ASC" // Natural thread view for session/chain
            } else {
                "ORDER BY similarity DESC" // Default for general search
            };
            
            let sql = format!(
                "SELECT meta::id(id) as id, content, significance, \
                 vector::similarity::cosine(embedding, $q) AS similarity \
                 FROM thoughts WHERE {} {} LIMIT $k",
                where_clause, order_by
            );
            
            let mut query = server.db.query(&sql)
                .bind(("q", q_emb))
                .bind(("dim", q_dim))
                .bind(("sim", sim_thresh))
                .bind(("k", top_k_th as i64));
            
            // Bind optional parameters
            if let Some(ref sid) = params.session_id {
                query = query.bind(("session_id", sid.clone()));
            }
            if let Some(ref cid) = params.chain_id {
                query = query.bind(("chain_id", cid.clone()));
            }
            
            let mut resp = query.await?;
            #[derive(Debug, Deserialize)]
            struct Row {
                id: String,
                content: String,
                #[serde(default)]
                significance: f32,
                similarity: f32,
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
    }

    Ok(CallToolResult::structured(serde_json::Value::Object(out)))
}
