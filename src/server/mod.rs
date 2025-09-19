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
use tracing::{info, warn};

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
    pub embedding_model: Option<String>,
    #[serde(default)]
    pub embedding_provider: Option<String>,
    #[serde(default)]
    pub embedding_dim: Option<i64>,
    #[serde(default)]
    pub embedded_at: Option<surrealdb::sql::Datetime>,
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
    /// Optional secondary database handle for photography namespace/db
    pub db_photo: Option<Arc<Surreal<Client>>>,
    pub thoughts: Arc<RwLock<LruCache<String, Thought>>>, // Bounded in-memory cache (LRU)
    pub embedder: Arc<dyn Embedder>,
    pub config: Arc<crate::config::Config>, // Retain config to avoid future env reads
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
                title: Some("Surreal Mind".to_string()),
                version: "0.1.0".to_string(),
                website_url: Some("https://github.com/8agana/surreal-mind".to_string()),
                icons: None,
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

        let convo_think_schema_map = crate::schemas::convo_think_schema();

        let legacymind_think_schema_map = crate::schemas::legacymind_think_schema();

        let maintenance_ops_schema_map = crate::schemas::maintenance_ops_schema();
        let kg_create_schema_map = crate::schemas::kg_create_schema();
        let kg_moderate_schema_map = crate::schemas::kg_moderate_schema();
        let detailed_help_schema_map = crate::schemas::detailed_help_schema();
        let inner_voice_schema_map = crate::schemas::inner_voice_schema();

        let mut tools = vec![
            Tool {
                name: "legacymind_think".into(),
                title: Some("LegacyMind Think".into()),
                description: Some("Unified thinking tool with automatic mode routing".into()),
                input_schema: legacymind_think_schema_map.clone(),
                icons: None,
                annotations: None,
                output_schema: None,
            },
            Tool {
                name: "maintenance_ops".into(),
                title: Some("Maintenance Operations".into()),
                description: Some("Maintenance operations for archival and cleanup".into()),
                input_schema: maintenance_ops_schema_map,
                icons: None,
                annotations: None,
                output_schema: None,
            },
            // (legacy think_search removed — use legacymind_search)
            Tool {
                name: "memories_create".into(),
                title: Some("Create Memories".into()),
                description: Some(
                    "Create entities and relationships in personal memory graph".into(),
                ),
                input_schema: kg_create_schema_map,
                icons: None,
                annotations: None,
                output_schema: None,
            },
            // (legacy memories_search removed — use legacymind_search or photography_search)
            Tool {
                name: "memories_moderate".into(),
                title: Some("Moderate Memories".into()),
                description: Some("Review and/or decide on memory graph candidates".into()),
                input_schema: kg_moderate_schema_map,
                icons: None,
                annotations: None,
                output_schema: None,
            },
            Tool {
                name: "detailed_help".into(),
                title: Some("Detailed Help".into()),
                description: Some("Get detailed help for a specific tool".into()),
                input_schema: detailed_help_schema_map,
                icons: None,
                annotations: None,
                output_schema: None,
            },
        ];

        // Always list the tool (visibility), enforce gating inside the handler if disabled
        tools.push(Tool {
            name: "inner_voice".into(),
            title: Some("Inner Voice".into()),
            description: Some(
                "Retrieves and synthesizes relevant memories/thoughts into a concise answer; can optionally auto-extract entities/relationships into staged knowledge‑graph candidates for review.".into(),
            ),
            input_schema: inner_voice_schema_map,
            icons: None,
            annotations: None,
            output_schema: None,
        });

        // Photography tools (always visible; handlers handle connection)
        let photo_mem_schema = crate::schemas::photography_memories_schema();
        let unified_schema = crate::schemas::unified_search_schema();
        tools.push(Tool {
            name: "photography_think".into(),
            title: Some("Photography Think".into()),
            description: Some(
                "Store photography thoughts with memory injection (isolated repo)".into(),
            ),
            input_schema: convo_think_schema_map.clone(),
            icons: None,
            annotations: None,
            output_schema: None,
        });
        tools.push(Tool {
            name: "photography_memories".into(),
            title: Some("Photography Memories".into()),
            description: Some(
                "Create/search/moderate photography knowledge graph (isolated repo)".into(),
            ),
            input_schema: photo_mem_schema,
            icons: None,
            annotations: None,
            output_schema: None,
        });
        tools.push(Tool {
            name: "legacymind_search".into(),
            title: Some("LegacyMind Search".into()),
            description: Some(
                "Unified LegacyMind search: memories (default) + optional thoughts".into(),
            ),
            input_schema: unified_schema.clone(),
            icons: None,
            annotations: None,
            output_schema: None,
        });
        tools.push(Tool {
            name: "photography_search".into(),
            title: Some("Photography Search".into()),
            description: Some(
                "Unified photography search: memories (default) + optional thoughts".into(),
            ),
            input_schema: unified_schema,
            icons: None,
            annotations: None,
            output_schema: None,
        });
        // (removed) photography_thoughts_search and photography_memories_search in favor of unified photography_search

        Ok(ListToolsResult {
            tools,
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        // Route to appropriate tool handler
        match request.name.as_ref() {
            // Unified thinking tool
            "legacymind_think" => self
                .handle_legacymind_think(request)
                .await
                .map_err(|e| e.into()),

            // Intelligence and utility
            "maintenance_ops" => self
                .handle_maintenance_ops(request)
                .await
                .map_err(|e| e.into()),
            // Memory tools
            "memories_create" => self
                .handle_knowledgegraph_create(request)
                .await
                .map_err(|e| e.into()),
            "memories_moderate" => self
                .handle_knowledgegraph_moderate(request)
                .await
                .map_err(|e| e.into()),
            // Inner voice retrieval
            // New canonical name
            "inner_voice" => self
                .handle_inner_voice_retrieve(request)
                .await
                .map_err(|e| e.into()),

            // Help
            "detailed_help" => self
                .handle_detailed_help(request)
                .await
                .map_err(|e| e.into()),
            // Photography (feature-gated)
            "photography_think" => self
                .handle_photography_think(request)
                .await
                .map_err(|e| e.into()),
            "photography_memories" => self
                .handle_photography_memories(request)
                .await
                .map_err(|e| e.into()),
            "legacymind_search" => self
                .handle_unified_search(request)
                .await
                .map_err(|e| e.into()),
            "photography_search" => self
                .handle_photography_unified_search(request)
                .await
                .map_err(|e| e.into()),
            "photography_voice" => self
                .handle_photography_voice(request)
                .await
                .map_err(|e| e.into()),
            "photography_moderate" => self
                .handle_photography_moderate(request)
                .await
                .map_err(|e| e.into()),
            // (removed) photography_thoughts_search and photography_memories_search
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
    pub async fn new(config: &crate::config::Config) -> Result<Self> {
        info!("Connecting to SurrealDB service via WebSocket");

        // Use the provided configuration directly instead of setting global env vars.
        // Embedder factory will read from the environment, but we keep the existing behaviour.

        // Normalize URL for SurrealDB Ws engine (expects host:port, no scheme)
        fn normalize_ws_url(s: &str) -> String {
            s.strip_prefix("ws://")
                .or_else(|| s.strip_prefix("wss://"))
                .or_else(|| s.strip_prefix("http://"))
                .or_else(|| s.strip_prefix("https://"))
                .unwrap_or(s)
                .to_string()
        }

        // Connect to SurrealDB instance
        // DB connection values from config
        let url = normalize_ws_url(&config.system.database_url);
        let user = &config.runtime.database_user;
        let pass = &config.runtime.database_pass;
        let ns = &config.system.database_ns;
        let dbname = &config.system.database_db;

        // Optional reconnection strategy with backoff
        let db_reconnect_enabled = std::env::var("SURR_DB_RECONNECT")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let max_retries = if db_reconnect_enabled { 5 } else { 0 };

        let mut db = None;
        for attempt in 0..=max_retries {
            match surrealdb::Surreal::new::<surrealdb::engine::remote::ws::Ws>(url.clone()).await {
                Ok(conn) => {
                    db = Some(conn);
                    if attempt > 0 {
                        info!(
                            "Successfully reconnected to SurrealDB after {} attempts",
                            attempt + 1
                        );
                    }
                    break;
                }
                Err(e) => {
                    if attempt == max_retries {
                        return Err(SurrealMindError::Database {
                            message: format!(
                                "Failed to connect to SurrealDB at {} after {} attempts: {}",
                                config.system.database_url,
                                max_retries + 1,
                                e
                            ),
                        });
                    } else {
                        let delay_ms = (1000 * (1u64 << attempt.min(5))).min(60000); // 1s, 2s, 4s, 8s, 16s, then 60s max
                        warn!(
                            "SurrealDB connection attempt {} failed: {}. Retrying in {}ms...",
                            attempt + 1,
                            e,
                            delay_ms
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    }
                }
            }
        }

        let db = db.expect("database should be initialized");

        // Sign in with credentials
        db.signin(surrealdb::opt::auth::Root {
            username: user.as_str(),
            password: pass.as_str(),
        })
        .await
        .with_context(|| format!("Failed to authenticate with SurrealDB as user '{}'", user))?;

        // Select namespace and database
        db.use_ns(ns)
            .await
            .with_context(|| format!("Failed to select namespace '{}'", ns))?;

        db.use_db(dbname)
            .await
            .with_context(|| format!("Failed to select database '{}'", dbname))?;

        // Initialize embedder
        let embedder = crate::embeddings::create_embedder(config)
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
        let thoughts_cache =
            LruCache::new(NonZeroUsize::new(cache_max).unwrap_or(NonZeroUsize::MIN));

        // Optionally connect a photography database handle
        let db_photo: Option<Arc<Surreal<Client>>> = if config.runtime.photo_enable {
            // Determine photo connection params (fallback to primary where not provided)
            let p_url = config
                .runtime
                .photo_url
                .as_ref()
                .map(|s| normalize_ws_url(s))
                .unwrap_or_else(|| url.clone());
            let p_user = config
                .runtime
                .photo_user
                .as_ref()
                .unwrap_or(user)
                .to_string();
            let p_pass = config
                .runtime
                .photo_pass
                .as_ref()
                .unwrap_or(pass)
                .to_string();
            let p_ns = config.runtime.photo_ns.as_ref().unwrap_or(ns).to_string();
            let p_db = config
                .runtime
                .photo_db
                .as_ref()
                .unwrap_or(dbname)
                .to_string();

            let dbp = Surreal::new::<surrealdb::engine::remote::ws::Ws>(&p_url)
                .await
                .context("Failed to connect to SurrealDB (photography)")?;
            dbp.signin(surrealdb::opt::auth::Root {
                username: &p_user,
                password: &p_pass,
            })
            .await
            .context("Failed to authenticate with SurrealDB (photography)")?;
            dbp.use_ns(&p_ns)
                .use_db(&p_db)
                .await
                .context("Failed to select photography NS/DB")?;
            Some(Arc::new(dbp))
        } else {
            None
        };

        let server = Self {
            db: Arc::new(db),
            db_photo,
            thoughts: Arc::new(RwLock::new(thoughts_cache)),
            embedder,
            config: Arc::new(config.clone()),
        };

        server
            .initialize_schema()
            .await
            .map_err(|e| SurrealMindError::Mcp {
                message: e.message.to_string(),
            })?;

        // Initialize schema in photography DB if present
        if let Some(photo_db) = &server.db_photo {
            let photo_server = server.clone_with_db(photo_db.clone());
            photo_server
                .initialize_schema()
                .await
                .map_err(|e| SurrealMindError::Mcp {
                    message: format!("(photography) {}", e.message),
                })?;
        }

        Ok(server)
    }

    /// Initialize the database schema
    async fn initialize_schema(&self) -> std::result::Result<(), McpError> {
        info!("Initializing consciousness graph schema");

        // Minimal schema to ensure required tables exist
        // Note: SurrealDB 2.x requires vector index definitions to include DIMENSION.
        // We derive the active embedding dimension from the embedder to avoid drift.
        let dim = self.embedder.dimensions();
        let schema_sql = format!(
            r#"
            DEFINE TABLE thoughts SCHEMAFULL;
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
            DEFINE FIELD status ON TABLE thoughts TYPE option<string>;
            -- Origin and privacy fields for retrieval
            DEFINE FIELD origin ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD tags ON TABLE thoughts TYPE option<array<string>>;
            DEFINE FIELD is_private ON TABLE thoughts TYPE option<bool>;
            -- Embedding metadata for future re-embedding
            DEFINE FIELD embedding_model ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD embedding_provider ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD embedding_dim ON TABLE thoughts TYPE option<int>;
            DEFINE FIELD embedded_at ON TABLE thoughts TYPE option<datetime>;
            -- Continuity fields for thought chaining
            DEFINE FIELD session_id ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD chain_id ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD previous_thought_id ON TABLE thoughts TYPE option<record<thoughts> | string>;
            DEFINE FIELD revises_thought ON TABLE thoughts TYPE option<record<thoughts> | string>;
            DEFINE FIELD branch_from ON TABLE thoughts TYPE option<record<thoughts> | string>;
            DEFINE FIELD confidence ON TABLE thoughts TYPE option<float>;
            DEFINE INDEX thoughts_embedding_idx ON TABLE thoughts FIELDS embedding HNSW DIMENSION {dim};
            DEFINE INDEX thoughts_status_idx ON TABLE thoughts FIELDS status;
            DEFINE INDEX idx_thoughts_created ON TABLE thoughts FIELDS created_at;
            DEFINE INDEX idx_thoughts_embedding_model ON TABLE thoughts FIELDS embedding_model;
            DEFINE INDEX idx_thoughts_embedding_dim ON TABLE thoughts FIELDS embedding_dim;
            -- Continuity indexes
            DEFINE INDEX idx_thoughts_session ON TABLE thoughts FIELDS session_id, created_at;
            DEFINE INDEX idx_thoughts_chain ON TABLE thoughts FIELDS chain_id, created_at;

            DEFINE TABLE recalls SCHEMALESS;
            DEFINE INDEX idx_recalls_created ON TABLE recalls FIELDS created_at;

            DEFINE TABLE kg_entities SCHEMALESS;
            DEFINE INDEX idx_kge_created ON TABLE kg_entities FIELDS created_at;
            DEFINE INDEX idx_kge_name ON TABLE kg_entities FIELDS name;
            DEFINE INDEX idx_kge_name_type ON TABLE kg_entities FIELDS name, data.entity_type;

            DEFINE TABLE kg_edges SCHEMALESS;
            DEFINE INDEX idx_kged_created ON TABLE kg_edges FIELDS created_at;
            DEFINE INDEX idx_kged_triplet ON TABLE kg_edges FIELDS source, target, rel_type;

            DEFINE TABLE kg_observations SCHEMALESS;
            DEFINE INDEX idx_kgo_created ON TABLE kg_observations FIELDS created_at;
            DEFINE INDEX idx_kgo_name ON TABLE kg_observations FIELDS name;
            DEFINE INDEX idx_kgo_name_src ON TABLE kg_observations FIELDS name, source_thought_id;

            -- Approval workflow candidate tables
            DEFINE TABLE kg_entity_candidates SCHEMALESS;
            DEFINE INDEX idx_kgec_status_created ON TABLE kg_entity_candidates FIELDS status, created_at;
            DEFINE INDEX idx_kgec_confidence ON TABLE kg_entity_candidates FIELDS confidence;
            DEFINE INDEX idx_kgec_name_type ON TABLE kg_entity_candidates FIELDS name, entity_type, status;

            DEFINE TABLE kg_edge_candidates SCHEMALESS;
            DEFINE INDEX idx_kgedc_status_created ON TABLE kg_edge_candidates FIELDS status, created_at;
            DEFINE INDEX idx_kgedc_confidence ON TABLE kg_edge_candidates FIELDS confidence;
            DEFINE INDEX idx_kgedc_triplet ON TABLE kg_edge_candidates FIELDS source_name, target_name, rel_type, status;

            -- Optional feedback helpers
            DEFINE TABLE kg_blocklist SCHEMALESS;
            DEFINE INDEX idx_kgb_item ON TABLE kg_blocklist FIELDS item;
        "#
        );

        self.db.query(schema_sql).await.map_err(|e| McpError {
            code: rmcp::model::ErrorCode::INTERNAL_ERROR,
            message: format!("Schema init failed: {}", e).into(),
            data: None,
        })?;

        Ok(())
    }

    /// Clone this server but swap the DB handle
    pub fn clone_with_db(&self, db: Arc<Surreal<Client>>) -> Self {
        Self {
            db,
            db_photo: self.db_photo.clone(),
            thoughts: self.thoughts.clone(),
            embedder: self.embedder.clone(),
            config: self.config.clone(),
        }
    }

    /// Connect to the photography database using runtime env or sensible defaults.
    /// Defaults: same URL/user/pass as primary; NS="photography", DB="work".
    pub async fn connect_photo_db(&self) -> crate::error::Result<Arc<Surreal<Client>>> {
        fn normalize_ws_url(s: &str) -> String {
            s.strip_prefix("ws://")
                .or_else(|| s.strip_prefix("wss://"))
                .or_else(|| s.strip_prefix("http://"))
                .or_else(|| s.strip_prefix("https://"))
                .unwrap_or(s)
                .to_string()
        }

        let p_url = self
            .config
            .runtime
            .photo_url
            .clone()
            .unwrap_or_else(|| self.config.system.database_url.clone());
        let p_user = self
            .config
            .runtime
            .photo_user
            .clone()
            .unwrap_or_else(|| self.config.runtime.database_user.clone());
        let p_pass = self
            .config
            .runtime
            .photo_pass
            .clone()
            .unwrap_or_else(|| self.config.runtime.database_pass.clone());
        let p_ns = self
            .config
            .runtime
            .photo_ns
            .clone()
            .unwrap_or_else(|| "photography".to_string());
        let p_db = self
            .config
            .runtime
            .photo_db
            .clone()
            .unwrap_or_else(|| "work".to_string());

        let url = normalize_ws_url(&p_url);
        let dbp = Surreal::new::<surrealdb::engine::remote::ws::Ws>(&url)
            .await
            .map_err(|e| SurrealMindError::Mcp {
                message: format!("photography connect failed: {}", e),
            })?;
        dbp.signin(surrealdb::opt::auth::Root {
            username: &p_user,
            password: &p_pass,
        })
        .await
        .map_err(|e| SurrealMindError::Mcp {
            message: format!("photography auth failed: {}", e),
        })?;
        dbp.use_ns(&p_ns)
            .use_db(&p_db)
            .await
            .map_err(|e| SurrealMindError::Mcp {
                message: format!("photography NS/DB select failed: {}", e),
            })?;
        Ok(Arc::new(dbp))
    }

    /// Get embedding metadata for tracking model/provider info
    pub fn get_embedding_metadata(&self) -> (String, String, i64) {
        let provider = self.config.system.embedding_provider.clone();
        let model = self.config.system.embedding_model.clone();
        let dim = self.embedder.dimensions() as i64;
        (provider, model, dim)
    }

    /// Calculate cosine similarity between two vectors (delegates to utils)
    #[allow(dead_code)]
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        crate::utils::cosine_similarity(a, b)
    }

    /// Perform KG-only memory injection: find similar KG entities and attach their IDs.
    pub async fn inject_memories(
        &self,
        thought_id: &str,
        embedding: &[f32],
        injection_scale: i64,
        submode: Option<&str>,
        tool_name: Option<&str>,
    ) -> crate::error::Result<(usize, Option<String>)> {
        tracing::debug!("inject_memories: query embedding dims: {}", embedding.len());
        // Orbital mechanics: determine limit and threshold from scale
        let scale = injection_scale.clamp(0, 3) as u8;
        if scale == 0 {
            return Ok((0, None));
        }
        // Thresholds from config.retrieval.t1, with optional env override and warn
        let t1 = std::env::var("SURR_INJECT_T1")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or_else(|| {
                if std::env::var("SURR_INJECT_T1").is_ok() {
                    tracing::warn!("Using env override SURR_INJECT_T1");
                }
                self.config.retrieval.t1
            });
        let t2 = std::env::var("SURR_INJECT_T2")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or_else(|| {
                if std::env::var("SURR_INJECT_T2").is_ok() {
                    tracing::warn!("Using env override SURR_INJECT_T2");
                }
                self.config.retrieval.t2
            });
        let t3 = std::env::var("SURR_INJECT_T3")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or_else(|| {
                if std::env::var("SURR_INJECT_T3").is_ok() {
                    tracing::warn!("Using env override SURR_INJECT_T3");
                }
                self.config.retrieval.t3
            });
        let (limit, mut prox_thresh) = match scale {
            0 => (0usize, 1.0f32),
            1 => (5usize, t1),
            2 => (10usize, t2),
            _ => (20usize, t3),
        };
        if limit == 0 {
            return Ok((0, None));
        }

        // Optional: submode-aware retrieval tweaks
        // Use config flag, with optional env override and warn
        if std::env::var("SURR_SUBMODE_RETRIEVAL").ok().as_deref() == Some("true")
            || (std::env::var("SURR_SUBMODE_RETRIEVAL").is_err()
                && self.config.retrieval.submode_tuning)
        {
            if std::env::var("SURR_SUBMODE_RETRIEVAL").is_ok() {
                tracing::warn!("Using env override SURR_SUBMODE_RETRIEVAL");
            }
            if let Some(sm) = submode {
                // Use lightweight profile deltas to adjust similarity threshold
                use crate::cognitive::profile::{Submode, profile_for};
                let profile = profile_for(Submode::from_str(sm));
                let delta = profile.injection.threshold_delta;
                // Clamp within [0.0, 0.99]
                prox_thresh = (prox_thresh + delta).clamp(0.0, 0.99);
            }
        }
        // Candidate pool size from config, with optional env override and warn
        let mut retrieve = std::env::var("SURR_KG_CANDIDATES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or_else(|| {
                if std::env::var("SURR_KG_CANDIDATES").is_ok() {
                    tracing::warn!("Using env override SURR_KG_CANDIDATES");
                }
                self.config.retrieval.candidates
            });

        // Tool-specific runtime defaults (no behavior drift beyond thresholds)
        if let Some(tool) = tool_name {
            // Only adjust candidate pool size per tool; do not override thresholds here
            retrieve = match tool {
                "think_convo" => 500,
                "think_plan" => 800,
                "think_debug" => 1000,
                "think_build" => 400,
                "think_stuck" => 600,
                "photography_think" => 500,
                _ => retrieve,
            };
        }

        // Fetch candidate entities and observations (two statements to avoid UNION pitfalls)
        // Filter by embedding_dim to avoid dimension mismatches at the DB level
        let q_dim = embedding.len() as i64;
        let mut q = self
            .db
            .query(
                "SELECT meta::id(id) as id, name, data, embedding FROM kg_entities \
                 WHERE embedding_dim = $dim AND embedding IS NOT NULL LIMIT $lim; \
                 SELECT meta::id(id) as id, name, data, embedding FROM kg_observations \
                 WHERE embedding_dim = $dim AND embedding IS NOT NULL LIMIT $lim;",
            )
            .bind(("dim", q_dim))
            .bind(("lim", retrieve as i64))
            .await?;
        let mut rows: Vec<serde_json::Value> = q.take(0).unwrap_or_default();
        let mut rows2: Vec<serde_json::Value> = q.take(1).unwrap_or_default();
        let total_candidates = rows.len() + rows2.len();
        rows.append(&mut rows2);
        tracing::debug!(
            "inject_memories: Retrieved {} candidates from KG (entities+observations)",
            total_candidates
        );

        // Iterate, compute or reuse embeddings, score by cosine similarity
        let mut scored: Vec<(String, f32, String, String)> = Vec::new();
        let mut skipped = 0;
        for r in rows {
            if let Some(id) = r.get("id").and_then(|v| v.as_str()) {
                // Try to use existing embedding; compute and persist if missing and allowed
                let mut emb_opt: Option<Vec<f32>> = None;
                if let Some(ev) = r.get("embedding").and_then(|v| v.as_array()) {
                    let vecf: Vec<f32> = ev
                        .iter()
                        .filter_map(|x| x.as_f64())
                        .map(|f| f as f32)
                        .collect();
                    if vecf.len() == embedding.len() {
                        emb_opt = Some(vecf);
                    }
                }
                if emb_opt.is_none() {
                    // Build text for embedding: name + type or description
                    let name_s = r.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let mut text = name_s.to_string();
                    if let Some(d) = r.get("data").and_then(|v| v.as_object()) {
                        if let Some(etype) = d.get("entity_type").and_then(|v| v.as_str()) {
                            text = format!("{} ({})", name_s, etype);
                        } else if let Some(desc) = d.get("description").and_then(|v| v.as_str()) {
                            text.push_str(" - ");
                            text.push_str(desc);
                        }
                    }
                    let new_emb = self.embedder.embed(&text).await.unwrap_or_default();
                    if new_emb.len() == embedding.len() {
                        emb_opt = Some(new_emb.clone());
                        // Determine table from id (kg_entities or kg_observations)
                        let tb = if id.starts_with("kg_entities:") {
                            "kg_entities"
                        } else if id.starts_with("kg_observations:") {
                            "kg_observations"
                        } else {
                            "kg_entities" // fallback
                        };
                        let inner_id = id
                            .split(':')
                            .nth(1)
                            .unwrap_or(id)
                            .trim_start_matches('⟨')
                            .trim_end_matches('⟩');
                        // Persist embedding for future fast retrieval (best-effort)
                        let (provider, model, dim) = self.get_embedding_metadata();
                        let _ = self
                            .db
                            .query("UPDATE type::thing($tb, $id) SET embedding = $emb, embedding_provider = $provider, embedding_model = $model, embedding_dim = $dim, embedded_at = time::now() RETURN meta::id(id) as id")
                            .bind(("tb", tb))
                            .bind(("id", inner_id.to_string()))
                            .bind(("emb", new_emb))
                            .bind(("provider", provider))
                            .bind(("model", model))
                            .bind(("dim", dim))
                            .await;
                    }
                }
                if let Some(emb_e) = emb_opt {
                    let sim = Self::cosine_similarity(embedding, &emb_e);
                    if sim >= prox_thresh {
                        let name_s = r
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let etype_or_desc = r
                            .get("data")
                            .and_then(|d| d.get("entity_type").or_else(|| d.get("description")))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        scored.push((id.to_string(), sim, name_s, etype_or_desc));
                    } else {
                        skipped += 1;
                    }
                }
            }
        }
        tracing::debug!(
            "inject_memories: {} candidates scored, {} skipped",
            scored.len(),
            skipped
        );

        // Sort by similarity and apply threshold; if nothing passes, take top by limit with a minimal floor (0.15)
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut selected: Vec<(String, f32, String, String)> = scored
            .iter()
            .filter(|&(_, s, _, _)| *s >= prox_thresh)
            .take(limit)
            .cloned()
            .collect();
        if selected.is_empty() && !scored.is_empty() {
            let floor = std::env::var("SURR_INJECT_FLOOR")
                .ok()
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or_else(|| {
                    if std::env::var("SURR_INJECT_FLOOR").is_ok() {
                        tracing::warn!("Using env override SURR_INJECT_FLOOR");
                    }
                    self.config.retrieval.floor
                });
            selected = scored
                .into_iter()
                .filter(|(_, s, _, _)| *s >= floor)
                .take(limit)
                .collect();
        }
        let memory_ids: Vec<String> = selected.iter().map(|(id, _, _, _)| id.clone()).collect();
        tracing::debug!(
            "inject_memories: Top {} matches: {:?}",
            selected.len(),
            selected
                .iter()
                .take(3)
                .map(|(_, sim, name, _)| format!("{:.2} {}", sim, name))
                .collect::<Vec<_>>()
        );

        // Optional enrichment with names/types
        let enriched = if !selected.is_empty() {
            let mut s = String::new();
            if let Some(sm) = submode {
                s.push_str(&format!("Submode: {}\n", sm));
            }
            s.push_str("Nearby entities:\n");
            for (i, (_id, sim, name, etype)) in selected.iter().take(5).enumerate() {
                if etype.is_empty() {
                    s.push_str(&format!("- ({:.2}) {}\n", sim, name));
                } else {
                    s.push_str(&format!("- ({:.2}) {} [{}]\n", sim, name, etype));
                }
                if i >= 4 {
                    break;
                }
            }
            Some(s)
        } else {
            None
        };

        // Persist to the thought
        let q = self
            .db
            .query("UPDATE type::thing($tb, $id) SET injected_memories = $mems, enriched_content = $enr RETURN meta::id(id) as id")
            .bind(("tb", "thoughts"))
            .bind(("id", thought_id.to_string()))
            .bind(("mems", memory_ids.clone()))
            .bind(("enr", enriched.clone().unwrap_or_default()));
        // Note: empty string will act like clearing or setting to empty; acceptable for now
        let _: Vec<serde_json::Value> = q.await?.take(0)?;
        tracing::debug!(
            "inject_memories: Injected {} memories for thought {}, enriched content length: {}",
            memory_ids.len(),
            thought_id,
            enriched.as_ref().map_or(0, |s| s.len())
        );

        Ok((memory_ids.len(), enriched))
    }

    /// Check for mixed embedding dimensions across thoughts and KG tables
    pub async fn check_embedding_dims(&self) -> Result<()> {
        // Query distinct embedding dimensions in thoughts
        let thoughts_dims: Vec<i64> = self
            .db
            .query("SELECT array::len(embedding) AS dim FROM thoughts GROUP ALL")
            .await
            .map_err(|e| SurrealMindError::Database {
                message: format!("Database query error: {}", e),
            })?
            .take(0)?;

        // Query distinct dimensions in KG entities
        let kg_entity_dims: Vec<i64> = self
            .db
            .query("SELECT array::len(embedding) AS dim FROM kg_entities GROUP ALL")
            .await
            .map_err(|e| SurrealMindError::Database {
                message: format!("Database query error: {}", e),
            })?
            .take(0)?;

        // Query distinct dimensions in KG observations
        let kg_obs_dims: Vec<i64> = self
            .db
            .query("SELECT array::len(embedding) AS dim FROM kg_observations GROUP ALL")
            .await
            .map_err(|e| SurrealMindError::Database {
                message: format!("Database query error: {}", e),
            })?
            .take(0)?;

        let mut all_dims = Vec::new();
        all_dims.extend(thoughts_dims);
        all_dims.extend(kg_entity_dims);
        all_dims.extend(kg_obs_dims);

        let unique_dims: std::collections::HashSet<_> = all_dims.into_iter().collect();

        if unique_dims.len() > 1 {
            return Err(SurrealMindError::Database {
                message: format!(
                    "Mixed embedding dimensions detected: {:?}. Re-embed to fix.",
                    unique_dims
                ),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let sim = SurrealMindServer::cosine_similarity(&a, &b);
        // Calculate expected: (1*4 + 2*5 + 3*6) / (sqrt(1+4+9) * sqrt(16+25+36)) = 32 / (sqrt(14) * sqrt(77)) ≈ 32 / (3.74 * 8.77) ≈ 32 / 32.84 ≈ 0.974
        assert!((sim - 0.974).abs() < 0.01);
    }

    #[test]
    fn test_tool_specific_defaults() {
        // Test that defaults are set correctly based on tool_name
        let mut prox_thresh = 0.5;
        let retrieve = 200;
        let tool = "think_convo";
        let (tool_sim_thresh, tool_db_limit) = match tool {
            "think_convo" => (0.35, 500),
            "think_plan" => (0.30, 800),
            "think_debug" => (0.20, 1000),
            "think_build" => (0.45, 400),
            "think_stuck" => (0.30, 600),
            _ => (prox_thresh, retrieve),
        };
        prox_thresh = tool_sim_thresh;
        let _retrieve = tool_db_limit;
        assert_eq!(prox_thresh, 0.35);
    }

    #[test]
    fn test_param_clamping() {
        // Test clamping for search params
        let params_top_k = 100; // Over limit
        let top_k = params_top_k.clamp(1, 50);
        assert_eq!(top_k, 50);

        let offset = 0;
        assert_eq!(offset, 0);
    }
}
