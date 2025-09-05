//! search_thoughts tool handler for semantic search with graph expansion

use crate::error::{Result, SurrealMindError};
use crate::server::{DateRangeParam, SurrealMindServer};
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Minimal struct for search results to avoid serialization issues
#[derive(Debug, Deserialize, Serialize)]
struct SearchRow {
    #[serde(deserialize_with = "crate::server::deserialize_thing_to_string")]
    id: String,
    content: String,
    created_at: surrealdb::sql::Datetime,
    embedding: Vec<f32>,
    #[serde(default)]
    significance: f32,
    #[serde(default)]
    access_count: u32,
    last_accessed: Option<surrealdb::sql::Datetime>,
}

/// Parameters for the search_thoughts tool
#[derive(Debug, serde::Deserialize)]
pub struct SearchThoughtsParams {
    pub content: String,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_usize_forgiving"
    )]
    pub top_k: Option<usize>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_usize_forgiving"
    )]
    pub offset: Option<usize>,
    #[serde(default)]
    pub sim_thresh: Option<f32>,
    #[serde(default)]
    pub min_significance: Option<f32>,
    #[serde(default)]
    pub date_range: Option<DateRangeParam>,
    #[serde(default)]
    pub expand_graph: Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_u8_forgiving"
    )]
    pub graph_depth: Option<u8>,
    #[serde(default)]
    pub graph_boost: Option<f32>,
    #[serde(default)]
    pub min_edge_strength: Option<f32>,
    #[serde(default)]
    pub sort_by: Option<String>,
}

impl SurrealMindServer {
    /// Handle the search_thoughts tool call
    pub async fn handle_search_thoughts(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: SearchThoughtsParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::Serialization {
                message: format!("Invalid parameters: {}", e),
            })?;

        // Config
        let top_k_default: usize = std::env::var("SURR_TOP_K")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&n| (1..=50).contains(&n))
            .unwrap_or(5);
        let sim_thresh_default: f32 = std::env::var("SURR_SIM_THRESH")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.3); // Lowered from 0.5 for better retrieval
        let limit_default: usize = std::env::var("SURR_DB_LIMIT")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&n| n > 0)
            .unwrap_or(500);

        let top_k = params.top_k.unwrap_or(top_k_default).clamp(1, 50);
        let offset = params.offset.unwrap_or(0).max(0);
        let sim_thresh = params
            .sim_thresh
            .unwrap_or(sim_thresh_default)
            .clamp(0.0, 1.0);

        // Compute query embedding
        let q_emb = self.embedder.embed(&params.content).await.map_err(|e| {
            SurrealMindError::Embedding {
                message: e.to_string(),
            }
        })?;

        // Let the database perform the vector search
        let q_dim = q_emb.len() as i64;
        let sql = "SELECT \
                        meta::id(id) as id, \
                        content, \
                        significance, \
                        vector::similarity::cosine(embedding, $q_emb) AS similarity \
                   FROM thoughts \
                   WHERE embedding_dim = $dim AND vector::similarity::cosine(embedding, $q_emb) > $sim_thresh \
                   ORDER BY similarity DESC \
                   LIMIT $limit START $start";

        // Execute and deserialize to simpler struct
        let mut response = self
            .db
            .query(sql)
            .bind(("q_emb", q_emb))
            .bind(("dim", q_dim))
            .bind(("sim_thresh", sim_thresh))
            .bind(("limit", top_k as i64))
            .bind(("start", offset as i64))
            .await?;

        #[derive(Debug, Deserialize)]
        struct SearchResultRow {
            id: String,
            content: String,
            #[serde(default)]
            significance: f32,
            similarity: f32,
        }

        let results: Vec<SearchResultRow> = response.take(0)?;
        let total = results.len();

        tracing::info!(
            "search: found {} matches (sim_thresh={:.3})",
            results.len(),
            sim_thresh,
        );

        let results: Vec<serde_json::Value> = results
            .into_iter()
            .map(|row| {
                json!({
                    "id": row.id,
                    "content": row.content,
                    "similarity": row.similarity,
                    "significance": row.significance
                })
            })
            .collect();

        let result = json!({
            "total": total,
            "offset": offset,
            "top_k": top_k,
            "results": results
        });

        Ok(CallToolResult::structured(result))
    }
}
