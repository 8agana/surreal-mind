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
    #[serde(skip_serializing_if = "Option::is_none")]
    submode: Option<String>,
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
    pub submode: Option<String>,
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
        let offset = params.offset.unwrap_or(0);
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

        // Fetch only the fields we need for search, extracting id as string
        let take = limit_default;

        // Use meta::id() to get just the record ID without the table prefix
        let sql = format!(
            "SELECT meta::id(id) as id, content, embedding, significance, submode FROM thoughts LIMIT {}",
            take
        );

        // Execute and deserialize to simpler struct
        let mut response = self.db.query(sql).await?;

        #[derive(Debug, Deserialize)]
        struct SimpleRow {
            // Now id will be a plain string from meta::id()
            id: String,
            content: String,
            embedding: Vec<f32>,
            #[serde(default)]
            significance: f32,
            #[serde(skip_serializing_if = "Option::is_none")]
            submode: Option<String>,
        }

        let rows: Vec<SimpleRow> = response.take(0)?;

        tracing::debug!(
            "search_thoughts: Retrieved {} thoughts, query embedding dims: {}",
            rows.len(),
            q_emb.len()
        );

        // Compute cosine similarity locally
        fn cosine(a: &[f32], b: &[f32]) -> f32 {
            if a.is_empty() || b.is_empty() || a.len() != b.len() {
                return 0.0;
            }
            let mut dot = 0.0f32;
            let mut na = 0.0f32;
            let mut nb = 0.0f32;
            for i in 0..a.len() {
                dot += a[i] * b[i];
                na += a[i] * a[i];
                nb += b[i] * b[i];
            }
            if na == 0.0 || nb == 0.0 {
                0.0
            } else {
                dot / (na.sqrt() * nb.sqrt())
            }
        }

        let mut matches: Vec<(f32, SimpleRow)> = Vec::new();
        let mut skipped_mismatched = 0;
        let mut below_threshold = 0;

        for row in rows.into_iter() {
            if row.embedding.len() == q_emb.len() {
                let sim = cosine(&q_emb, &row.embedding);

                // DEEP DEBUG: Log first few dimensions of embeddings for debugging
                tracing::debug!(
                    "EMBEDDING DEBUG: id={}, sim={:.6}, query_first_5=[{:.6}, {:.6}, {:.6}, {:.6}, {:.6}], stored_first_5=[{:.6}, {:.6}, {:.6}, {:.6}, {:.6}]",
                    row.id,
                    sim,
                    q_emb.first().unwrap_or(&0.0),
                    q_emb.get(1).unwrap_or(&0.0),
                    q_emb.get(2).unwrap_or(&0.0),
                    q_emb.get(3).unwrap_or(&0.0),
                    q_emb.get(4).unwrap_or(&0.0),
                    row.embedding.first().unwrap_or(&0.0),
                    row.embedding.get(1).unwrap_or(&0.0),
                    row.embedding.get(2).unwrap_or(&0.0),
                    row.embedding.get(3).unwrap_or(&0.0),
                    row.embedding.get(4).unwrap_or(&0.0)
                );

                // Debug logging for high similarity scores
                if sim >= 0.3 {
                    tracing::info!(
                        "Found potential match: sim={:.4}, id={}, content_preview={}...",
                        sim,
                        row.id,
                        &row.content.chars().take(50).collect::<String>()
                    );
                }

                if sim >= sim_thresh {
                    matches.push((sim, row));
                } else {
                    below_threshold += 1;
                }
            } else {
                skipped_mismatched += 1;
                tracing::warn!(
                    "Dimension mismatch: query={}, thought={}, id={}",
                    q_emb.len(),
                    row.embedding.len(),
                    row.id
                );
            }
        }

        if skipped_mismatched > 0 {
            tracing::warn!(
                "Skipped {} thoughts with mismatched embedding dimensions",
                skipped_mismatched
            );
        }
        tracing::debug!(
            "search_thoughts: {} matches above threshold {}, {} below threshold",
            matches.len(),
            sim_thresh,
            below_threshold
        );

        // Sort by similarity desc and apply offset/top_k
        matches.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let total = matches.len();
        let end = (offset + top_k).min(total);
        let sliced = if offset < total {
            &matches[offset..end]
        } else {
            &[]
        };

        let results: Vec<serde_json::Value> = sliced
            .iter()
            .map(|(sim, row)| {
                json!({
                    "id": row.id,
                    "content": row.content,
                    "similarity": sim,
                    "submode": row.submode,
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
