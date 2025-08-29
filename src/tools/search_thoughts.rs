//! search_thoughts tool handler for semantic search with graph expansion

use crate::error::{Result, SurrealMindError};
use crate::server::{DateRangeParam, SurrealMindServer};
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;

/// Parameters for the search_thoughts tool
#[derive(Debug, serde::Deserialize)]
pub struct SearchThoughtsParams {
    pub content: String,
    #[serde(default)]
    pub top_k: Option<usize>,
    #[serde(default)]
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
    #[serde(default)]
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

        // For now, return a simple search result
        // TODO: Implement full search_thoughts logic
        let result = json!({
            "total": 0,
            "offset": params.offset.unwrap_or(0),
            "top_k": params.top_k.unwrap_or(10),
            "results": []
        });

        Ok(CallToolResult::structured(result))
    }
}
