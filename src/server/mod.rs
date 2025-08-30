//! Server module containing the SurrealMindServer implementation

use crate::embeddings::Embedder;
use crate::error::{Result, SurrealMindError};
use anyhow::Context;
use lru::LruCache;
use rmcp::{
    ErrorData as McpError,
    handler::server::ServerHandler,
    model::{
        CallToolRequestParam, CallToolResult, Implementation, InitializeRequestParam,
        InitializeResult, ListToolsResult, PaginatedRequestParam, ProtocolVersion,
        ServerCapabilities, ServerInfo, ToolsCapability,
    },
    service::{RequestContext, RoleServer},
};
use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;
use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Client;
use tokio::sync::RwLock;
use tracing::info;

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
                    if let Some(inner_id) = id_obj.get("String") {
                        if let Some(s) = inner_id.as_str() {
                            return Ok(s.to_string());
                        }
                    }
                    Ok(format!("thoughts:{}", serde_json::to_string(id).unwrap_or_default()))
                } else {
                    Ok(format!("thoughts:{}", id))
                }
            } else {
                Err(D::Error::custom("Missing id field"))
            }
        }
        _ => Err(D::Error::custom("Invalid id type"))
    }
}

/// Data models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thought {
    #[serde(deserialize_with = "deserialize_thing_to_string")]
    pub id: String,
    pub content: String,
    pub created_at: surrealdb::sql::Datetime,
    pub embedding: Vec<f32>,
    pub injected_memories: Vec<String>,
    pub enriched_content: Option<String>,
    pub injection_scale: u8,
    pub significance: f32,
    pub access_count: u32,
    pub last_accessed: Option<surrealdb::sql::Datetime>,
    #[serde(default)]
    pub submode: Option<String>,
    #[serde(default)]
    pub framework_enhanced: Option<bool>,
    #[serde(default)]
    pub framework_analysis: Option<serde_json::Value>,
    #[serde(default)]
    pub is_inner_voice: Option<bool>,
    #[serde(default)]
    pub inner_visibility: Option<String>,
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
}

impl ServerHandler for SurrealMindServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                ..Default::default()
            },
            server_info: Implementation {
                name: "surreal-mind".to_string(),
                version: "0.1.0".to_string(),
            },
            ..Default::default()
        }
    }

    async fn initialize(
        &self,
        request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<InitializeResult, McpError> {
        let mut info = self.get_info();
        info.protocol_version = request.protocol_version.clone();
        Ok(info)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        info!("tools/list requested");

        use rmcp::model::Tool;
        use serde_json::{json, Map, Value};
        use std::sync::Arc;

        // Minimal JSON Schemas for each tool input
        let convo_think_schema = json!({
            "type": "object",
            "properties": {
                "content": {"type": "string"},
                "injection_scale": {"type": ["integer", "string"]},
                "submode": {"type": "string"},
                "tags": {"type": "array", "items": {"type": "string"}},
                "significance": {"type": ["number", "string"]},
                "verbose_analysis": {"type": "boolean"}
            },
            "required": ["content"]
        });
        let convo_think_schema_map: Arc<Map<String, Value>> = Arc::new(
            convo_think_schema.as_object().cloned().unwrap_or_else(|| Map::new()),
        );

        let tech_think_schema = json!({
            "type": "object",
            "properties": {
                "content": {"type": "string"},
                "injection_scale": {"type": ["integer", "string"]},
                "submode": {"type": "string", "enum": ["plan", "build", "debug"], "default": "plan"},
                "tags": {"type": "array", "items": {"type": "string"}},
                "significance": {"type": ["number", "string"]},
                "verbose_analysis": {"type": "boolean"}
            },
            "required": ["content"]
        });
        let tech_think_schema_map: Arc<Map<String, Value>> = Arc::new(
            tech_think_schema.as_object().cloned().unwrap_or_else(|| Map::new()),
        );

        let inner_voice_schema = json!({
            "type": "object",
            "properties": {
                "content": {"type": "string"},
                "injection_scale": {"type": ["integer", "string"]},
                "tags": {"type": "array", "items": {"type": "string"}},
                "significance": {"type": ["number", "string"]},
                "verbose_analysis": {"type": "boolean"},
                "inner_visibility": {"type": "string", "enum": ["private", "context_only"], "default": "context_only"}
            },
            "required": ["content"]
        });
        let inner_voice_schema_map: Arc<Map<String, Value>> = Arc::new(
            inner_voice_schema.as_object().cloned().unwrap_or_else(|| Map::new()),
        );

        let search_thoughts_schema = json!({
            "type": "object",
            "properties": {
                "content": {"type": "string"},
                "top_k": {"type": "integer", "minimum": 1, "maximum": 50},
                "offset": {"type": "integer", "minimum": 0},
                "sim_thresh": {"type": "number", "minimum": 0.0, "maximum": 1.0},
                "submode": {"type": "string"},
                "min_significance": {"type": "number", "minimum": 0.0, "maximum": 1.0},
                "expand_graph": {"type": "boolean"},
                "graph_depth": {"type": "integer", "minimum": 0, "maximum": 3},
                "graph_boost": {"type": "number"},
                "min_edge_strength": {"type": "number"},
                "sort_by": {"type": "string", "enum": ["score", "similarity", "recency", "significance"], "default": "score"}
            },
            "required": ["content"]
        });
        let search_thoughts_schema_map: Arc<Map<String, Value>> = Arc::new(
            search_thoughts_schema
                .as_object()
                .cloned()
                .unwrap_or_else(|| Map::new()),
        );

        let kg_create_schema = json!({
            "type": "object",
            "properties": {
                "kind": {"type": "string", "enum": ["entity", "relationship", "observation"], "default": "entity"},
                "data": {"type": "object"},
                "upsert": {"type": "boolean", "default": true},
                "source_thought_id": {"type": "string"},
                "confidence": {"type": "number", "minimum": 0.0, "maximum": 1.0}
            },
            "required": ["kind", "data"]
        });
        let kg_create_schema_map: Arc<Map<String, Value>> = Arc::new(
            kg_create_schema.as_object().cloned().unwrap_or_else(|| Map::new()),
        );

        let kg_search_schema = json!({
            "type": "object",
            "properties": {
                "query": {"type": "object"},
                "target": {"type": "string", "enum": ["entity", "relationship", "observation", "mixed"], "default": "mixed"},
                "top_k": {"type": "integer", "minimum": 1, "maximum": 50}
            }
        });
        let kg_search_schema_map: Arc<Map<String, Value>> = Arc::new(
            kg_search_schema.as_object().cloned().unwrap_or_else(|| Map::new()),
        );

        let detailed_help_schema = json!({
            "type": "object",
            "properties": {
                "tool": {"type": "string", "enum": [
                    "convo_think", "tech_think", "inner_voice", "search_thoughts", "knowledgegraph_create", "knowledgegraph_search"
                ]},
                "format": {"type": "string", "enum": ["compact", "full"], "default": "full"}
            }
        });
        let detailed_help_schema_map: Arc<Map<String, Value>> = Arc::new(
            detailed_help_schema
                .as_object()
                .cloned()
                .unwrap_or_else(|| Map::new()),
        );

        let tools = vec![
            Tool { name: "convo_think".into(), description: Some("Store conversational thoughts with memory injection".into()), input_schema: convo_think_schema_map, annotations: None, output_schema: None },
            Tool { name: "tech_think".into(), description: Some("Technical reasoning with memory injection".into()), input_schema: tech_think_schema_map, annotations: None, output_schema: None },
            Tool { name: "inner_voice".into(), description: Some("Private inner thoughts with visibility controls".into()), input_schema: inner_voice_schema_map, annotations: None, output_schema: None },
            Tool { name: "search_thoughts".into(), description: Some("Semantic search with optional KG expansion".into()), input_schema: search_thoughts_schema_map, annotations: None, output_schema: None },
            Tool { name: "knowledgegraph_create".into(), description: Some("Create entities and relationships in the KG".into()), input_schema: kg_create_schema_map, annotations: None, output_schema: None },
            Tool { name: "knowledgegraph_search".into(), description: Some("Search entities/relationships in the KG".into()), input_schema: kg_search_schema_map, annotations: None, output_schema: None },
            Tool { name: "detailed_help".into(), description: Some("Get detailed help for a specific tool".into()), input_schema: detailed_help_schema_map, annotations: None, output_schema: None },
        ];

        Ok(ListToolsResult { tools, ..Default::default() })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        // Route to appropriate tool handler
        match request.name.as_ref() {
            "convo_think" => self.handle_convo_think(request).await.map_err(|e| e.into()),
            "tech_think" => self.handle_tech_think(request).await.map_err(|e| e.into()),
            "inner_voice" => self.handle_inner_voice(request).await.map_err(|e| e.into()),
            "search_thoughts" => self
                .handle_search_thoughts(request)
                .await
                .map_err(|e| e.into()),
            "knowledgegraph_create" => self
                .handle_knowledgegraph_create(request)
                .await
                .map_err(|e| e.into()),
            "knowledgegraph_search" => self
                .handle_knowledgegraph_search(request)
                .await
                .map_err(|e| e.into()),
            "detailed_help" => {
                // TODO: Implement detailed help
                Ok(CallToolResult::structured(
                    serde_json::json!({"tools": ["convo_think", "tech_think", "inner_voice", "search_thoughts", "knowledgegraph_create", "knowledgegraph_search"]}),
                ))
            }
            _ => Err(McpError {
                code: rmcp::model::ErrorCode::METHOD_NOT_FOUND,
                message: format!("Unknown tool: {}", request.name).into(),
                data: None,
            }),
        }
    }
}

impl SurrealMindServer {
    /// Create a new SurrealMind server instance
    pub async fn new() -> Result<Self> {
        info!("Connecting to SurrealDB service via WebSocket");

        let url = std::env::var("SURR_DB_URL").unwrap_or_else(|_| "127.0.0.1:8000".to_string());
        let user = std::env::var("SURR_DB_USER").unwrap_or_else(|_| "root".to_string());
        let pass = std::env::var("SURR_DB_PASS").unwrap_or_else(|_| "root".to_string());
        let ns = std::env::var("SURR_DB_NS").unwrap_or_else(|_| "surreal_mind".to_string());
        let dbname = std::env::var("SURR_DB_DB").unwrap_or_else(|_| "consciousness".to_string());

        // Connect to the running SurrealDB service
        let db = Surreal::new::<surrealdb::engine::remote::ws::Ws>(url)
            .await
            .context("Failed to connect to SurrealDB service")?;

        // Authenticate
        db.signin(surrealdb::opt::auth::Root {
            username: &user,
            password: &pass,
        })
        .await
        .context("Failed to authenticate with SurrealDB")?;

        // Select namespace and database
        db.use_ns(ns)
            .use_db(dbname)
            .await
            .context("Failed to select namespace and database")?;

        // Initialize embedder
        let embedder = crate::embeddings::create_embedder()
            .await
            .context("Failed to create embedder")?;
        info!(
            "Embedder initialized with {} dimensions",
            embedder.dimensions()
        );

        // Initialize bounded in-memory cache (LRU)
        let cache_max: usize = std::env::var("SURR_CACHE_MAX")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(5000);
        let thoughts_cache = LruCache::new(NonZeroUsize::new(cache_max).unwrap());

        let server = Self {
            db: Arc::new(db),
            thoughts: Arc::new(RwLock::new(thoughts_cache)),
            embedder,
        };

        server
            .initialize_schema()
            .await
            .map_err(|e| SurrealMindError::Mcp {
                message: e.message.to_string(),
            })?;

        Ok(server)
    }

    /// Initialize the database schema
    async fn initialize_schema(&self) -> std::result::Result<(), McpError> {
        info!("Initializing consciousness graph schema");

        // Minimal schema to ensure required tables exist
        let schema_sql = r#"
            DEFINE TABLE thoughts SCHEMAFULL;
            DEFINE FIELD id ON TABLE thoughts TYPE string;
            DEFINE FIELD content ON TABLE thoughts TYPE string;
            DEFINE FIELD created_at ON TABLE thoughts TYPE datetime;
            DEFINE FIELD embedding ON TABLE thoughts TYPE array<float>;
            DEFINE FIELD injected_memories ON TABLE thoughts TYPE array<string>;
            DEFINE FIELD enriched_content ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD injection_scale ON TABLE thoughts TYPE int;
            DEFINE FIELD significance ON TABLE thoughts TYPE float;
            DEFINE FIELD access_count ON TABLE thoughts TYPE int;
            DEFINE FIELD last_accessed ON TABLE thoughts TYPE option<datetime>;
            DEFINE FIELD submode ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD framework_enhanced ON TABLE thoughts TYPE option<bool>;
            DEFINE FIELD framework_analysis ON TABLE thoughts FLEXIBLE TYPE option<object>;
            DEFINE FIELD is_inner_voice ON TABLE thoughts TYPE option<bool>;
            DEFINE FIELD inner_visibility ON TABLE thoughts TYPE option<string>;
            DEFINE INDEX idx_thoughts_created ON TABLE thoughts FIELDS created_at;

            DEFINE TABLE recalls SCHEMALESS;
            DEFINE INDEX idx_recalls_created ON TABLE recalls FIELDS created_at;

            DEFINE TABLE kg_entities SCHEMALESS;
            DEFINE INDEX idx_kge_created ON TABLE kg_entities FIELDS created_at;

            DEFINE TABLE kg_edges SCHEMALESS;
            DEFINE INDEX idx_kged_created ON TABLE kg_edges FIELDS created_at;
        "#;

        self
            .db
            .query(schema_sql)
            .await
            .map_err(|e| McpError { code: rmcp::model::ErrorCode::INTERNAL_ERROR, message: format!("Schema init failed: {}", e).into(), data: None })?;

        Ok(())
    }

    /// Calculate cosine similarity between two vectors
    #[allow(dead_code)]
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            tracing::warn!(
                "cosine_similarity dimension mismatch: a={}, b={}",
                a.len(),
                b.len()
            );
            return 0.0;
        }
        if a.is_empty() {
            return 0.0;
        }
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    }
}
