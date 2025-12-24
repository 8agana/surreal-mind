//! Server module containing the SurrealMindServer implementation

use crate::embeddings::Embedder;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Client;
use tokio::sync::RwLock;

// Submodules
pub mod db;
pub mod router;
pub mod schema;

/// Custom deserializer for SurrealDB Thing to String
pub fn deserialize_thing_to_string<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let value = serde_json::Value::deserialize(deserializer)?;

    // Handle both String and Thing types
    match value {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Object(obj) => {
            // Extract id from Thing object
            if let Some(id) = obj.get("id") {
                if let Some(id_str) = id.as_str() {
                    Ok(id_str.to_string())
                } else if let Some(id_obj) = id.as_object() {
                    // Handle nested id object
                    if let Some(inner_id) = id_obj.get("String")
                        && let Some(s) = inner_id.as_str()
                    {
                        return Ok(s.to_string());
                    }
                    Ok(format!(
                        "thoughts:{}",
                        serde_json::to_string(id).unwrap_or_default()
                    ))
                } else {
                    Ok(format!("thoughts:{}", id))
                }
            } else {
                Err(D::Error::custom("Missing id field"))
            }
        }
        _ => Err(D::Error::custom("Invalid id type")),
    }
}

/// Data models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thought {
    #[serde(deserialize_with = "deserialize_thing_to_string")]
    pub id: String,
    pub content: String,
    pub created_at: surrealdb::sql::Datetime,
    #[serde(default)]
    pub embedding: Vec<f32>,
    #[serde(default)]
    pub injected_memories: Vec<String>,
    pub enriched_content: Option<String>,
    #[serde(default)]
    pub injection_scale: u8,
    #[serde(default)]
    pub significance: f32,
    #[serde(default)]
    pub access_count: u32,
    pub last_accessed: Option<surrealdb::sql::Datetime>,
    #[serde(default)]
    pub submode: Option<String>,
    #[serde(default)]
    pub framework_enhanced: Option<bool>,
    #[serde(default)]
    pub framework_analysis: Option<serde_json::Value>,
    pub embedding_model: Option<String>,
    #[serde(default)]
    pub embedding_provider: Option<String>,
    #[serde(default)]
    pub embedding_dim: Option<i64>,
    #[serde(default)]
    pub embedded_at: Option<surrealdb::sql::Datetime>,
    #[serde(default)]
    pub extracted_to_kg: bool,
    #[serde(default)]
    pub extraction_batch_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThoughtMatch {
    pub thought: Thought,
    pub similarity_score: f32,
    pub orbital_proximity: f32,
}

/// KG-only retrieval memory item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KGMemory {
    pub entity_id: String,
    pub name: String,
    pub entity_type: String,
    pub similarity: f32,
    pub proximity: f32,
    pub score: f32,
    pub neighbors: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DateRangeParam {
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
}

// Types are available for import from this module

#[derive(Debug, Deserialize)]
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

/// Main SurrealMind server implementation
#[derive(Clone)]
pub struct SurrealMindServer {
    pub db: Arc<Surreal<Client>>,
    pub thoughts: Arc<RwLock<LruCache<String, Thought>>>, // Bounded in-memory cache (LRU)
    pub embedder: Arc<dyn Embedder>,
    pub config: Arc<crate::config::Config>, // Retain config to avoid future env reads
}
