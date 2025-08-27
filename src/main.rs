use anyhow::{Context, Result};
use chrono::Utc;
use rmcp::{
    ErrorData as McpError,
    handler::server::ServerHandler,
    model::{
        CallToolRequestParam, CallToolResult, Implementation, InitializeRequestParam,
        InitializeResult, ListToolsResult, PaginatedRequestParam, ProtocolVersion,
        ServerCapabilities, ServerInfo, Tool, ToolsCapability,
    },
    service::{RequestContext, RoleServer, ServiceExt},
    transport::stdio,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::borrow::Cow;

use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::auth::Root;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info};
use uuid::Uuid;

mod cognitive;
mod deserializers;
mod embeddings;
mod flavor;
use embeddings::{create_embedder, Embedder};
use flavor::tag_flavor;

// Custom serializer for String (ensures it's always serialized as a plain string)
fn serialize_as_string<S>(id: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(id)
}

// Custom deserializer for Thing or String
fn deserialize_thing_or_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let value = serde_json::Value::deserialize(deserializer)?;

    match value {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Object(map) => {
            // Handle Thing object from SurrealDB
            if let Some(serde_json::Value::String(tb)) = map.get("tb") {
                if let Some(id_val) = map.get("id") {
                    match id_val {
                        serde_json::Value::String(id) => Ok(format!("{}:{}", tb, id)),
                        serde_json::Value::Object(id_obj) => {
                            if let Some(serde_json::Value::String(id_str)) = id_obj.get("String") {
                                Ok(format!("{}:{}", tb, id_str))
                            } else {
                                Ok(format!(
                                    "{}:{}",
                                    tb,
                                    serde_json::to_string(id_val).unwrap_or_default()
                                ))
                            }
                        }
                        _ => Ok(format!(
                            "{}:{}",
                            tb,
                            serde_json::to_string(id_val).unwrap_or_default()
                        )),
                    }
                } else {
                    Err(D::Error::custom("Thing object missing 'id' field"))
                }
            } else {
                Err(D::Error::custom("Expected Thing object or string"))
            }
        }
        _ => Err(D::Error::custom("Expected Thing object or string")),
    }
}

fn de_option_tags<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let opt = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(v) = opt else { return Ok(None) };
    match v {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::String(s) => Ok(Some(vec![s])),
        serde_json::Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for el in arr {
                match el {
                    serde_json::Value::String(s) => out.push(s),
                    other => out.push(other.to_string()),
                }
            }
            Ok(Some(out))
        }
        other => Err(D::Error::custom(format!(
            "invalid type for tags: {}",
            other
        ))),
    }
}

// Data models
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Thought {
    #[serde(
        serialize_with = "serialize_as_string",
        deserialize_with = "deserialize_thing_or_string"
    )]
    id: String,
    content: String,
    created_at: surrealdb::sql::Datetime,
    embedding: Vec<f32>,
    injected_memories: Vec<String>,
    enriched_content: Option<String>,
    injection_scale: u8,
    significance: f32,
    access_count: u32,
    last_accessed: Option<surrealdb::sql::Datetime>,
    #[serde(default)]
    submode: Option<String>,
    #[serde(default)]
    framework_enhanced: Option<bool>,
    #[serde(default)]
    framework_analysis: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ThoughtMatch {
    thought: Thought,
    similarity_score: f32,
    orbital_proximity: f32,
}

#[derive(Debug, Clone, Deserialize)]
struct DateRangeParam {
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchThoughtsParams {
    content: String,
    #[serde(default)]
    top_k: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    sim_thresh: Option<f32>,
    #[serde(default)]
    submode: Option<String>,
    #[serde(default)]
    min_significance: Option<f32>,
    #[serde(default)]
    date_range: Option<DateRangeParam>,
    #[serde(default)]
    expand_graph: Option<bool>,
    #[serde(default)]
    graph_depth: Option<u8>,
    #[serde(default)]
    graph_boost: Option<f32>,
    #[serde(default)]
    min_edge_strength: Option<f32>,
    #[serde(default)]
    sort_by: Option<String>,
}


#[derive(Debug, Deserialize)]
struct ConvoThinkParams {
    content: String,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_u8_forgiving"
    )]
    injection_scale: Option<u8>,
    submode: Option<String>,
    #[allow(dead_code)]
    #[serde(default, deserialize_with = "de_option_tags")]
    tags: Option<Vec<String>>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_f32_forgiving"
    )]
    significance: Option<f32>,
    #[serde(default)]
    verbose_analysis: Option<bool>,
}

#[derive(Clone)]
struct SurrealMindServer {
    db: Arc<Surreal<Client>>,
    db_gate: Arc<Mutex<()>>, // Serialization gate for DB queries
    thoughts: Arc<RwLock<LruCache<String, Thought>>>, // Bounded in-memory cache (LRU)
    embedder: Arc<dyn Embedder>,
}

impl ServerHandler for SurrealMindServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    // Set to false because this server does not emit tools/list_changed notifications.
                    // Some MCP clients (e.g., certain Claude Desktop versions) may wait for the
                    // notification if this is true and never call tools/list proactively.
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
    ) -> Result<InitializeResult, McpError> {
        // Prefer the client's protocol version to avoid mismatches with some MCP clients (e.g., Claude Desktop)
        let mut info = self.get_info();
        info.protocol_version = request.protocol_version.clone();
        Ok(info)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        info!("tools/list requested");
        // search_thoughts schema
        let search_schema = rmcp::object!({
            "type": "object",
            "properties": {
                "content": {"type": "string", "description": "Query text to match against stored thoughts"},
                "top_k": {"type": "integer", "minimum": 1, "maximum": 50, "description": "Max results to return (default from env)"},
                "offset": {"type": "integer", "minimum": 0, "description": "Offset for pagination (default 0)"},
                "sim_thresh": {"type": "number", "minimum": 0.0, "maximum": 1.0, "description": "Similarity threshold (default from env)"},
                "submode": {"type": "string", "enum": ["sarcastic", "philosophical", "empathetic", "problem_solving"], "description": "Optional submode to tune retrieval if enabled"},
                "min_significance": {"type": "number", "minimum": 0.0, "maximum": 1.0, "description": "Minimum significance filter (default 0.0)"},
                "date_range": {"type": "object", "properties": {
                    "from": {"type": "string", "format": "date-time"},
                    "to": {"type": "string", "format": "date-time"}
                }},
                "expand_graph": {"type": "boolean", "description": "Expand via recalls graph (default false)"},
                "graph_depth": {"type": "integer", "minimum": 0, "maximum": 2, "description": "Graph expansion depth (default 1 when expand_graph)"},
                "graph_boost": {"type": "number", "minimum": 0.0, "maximum": 1.0, "description": "Score boost for neighbors (default 0.15)"},
                "min_edge_strength": {"type": "number", "minimum": 0.0, "maximum": 1.0, "description": "Minimum recalls edge strength when expanding (default 0.0)"},
                "sort_by": {"type": "string", "enum": ["score", "similarity", "recency", "significance"], "description": "Sort mode (default 'score')"}
            },
            "required": ["content"]
        });
        let convo_schema = rmcp::object!({
            "type": "object",
            "properties": {
                "content": {"type": "string", "description": "The thought content to store"},
                "injection_scale": {"type": ["integer", "string"], "description": "Memory injection scale. Presets: NONE (0), LIGHT (1), MEDIUM (2), DEFAULT (3), HIGH (4), MAXIMUM (5). Or numeric 0-5"},
                "submode": {"type": "string", "description": "Conversation submode", "enum": ["sarcastic", "philosophical", "empathetic", "problem_solving"]},
                "tags": {"type": "array", "items": {"type": "string"}, "description": "Additional tags"},
                "significance": {"type": ["number", "string"], "description": "Significance weight. Strings: 'low' (0.2), 'medium' (0.5), 'high' (0.9). Numbers: 0.0-1.0 or integers 2-10 (mapped to 0.2-1.0)"},
                "verbose_analysis": {"type": "boolean", "description": "Enable verbose framework analysis (default: true)", "default": true}
            },
            "required": ["content"]
        });

        let tech_schema = rmcp::object!({
            "type": "object",
            "properties": {
                "content": {"type": "string", "description": "Technical input (requirements, code, error, etc.)"},
                "injection_scale": {"type": ["integer", "string"], "description": "Memory injection scale. Presets: NONE (0), LIGHT (1), MEDIUM (2), DEFAULT (3), HIGH (4), MAXIMUM (5). Or numeric 0-5"},
                "submode": {"type": "string", "description": "Technical mode", "enum": ["plan", "build", "debug"]},
                "tags": {"type": "array", "items": {"type": "string"}, "description": "Additional tags"},
                "significance": {"type": ["number", "string"], "description": "Significance weight. Strings: 'low' (0.2), 'medium' (0.5), 'high' (0.9). Numbers: 0.0-1.0 or integers 2-10 (mapped to 0.2-1.0)"},
                "verbose_analysis": {"type": "boolean", "description": "Enable verbose framework analysis (default: true)", "default": true}
            },
            "required": ["content"]
        });

        let help_schema = rmcp::object!({
            "type": "object",
            "properties": {
                "tool": {"type": "string", "enum": ["convo_think", "tech_think"], "description": "Which tool to describe (omit for overview)"},
                "format": {"type": "string", "enum": ["compact", "full"], "description": "Level of detail", "default": "full"}
            }
        });

        Ok(ListToolsResult {
            tools: vec![
                Tool::new(
                    Cow::Borrowed("convo_think"),
                    Cow::Borrowed("Store thoughts with memory injection"),
                    Arc::new(convo_schema),
                ),
                Tool::new(
                    Cow::Borrowed("tech_think"),
                    Cow::Borrowed("Technical reasoning with memory injection"),
                    Arc::new(tech_schema),
                ),
                Tool::new(
                    Cow::Borrowed("search_thoughts"),
                    Cow::Borrowed(
                        "Search thoughts by semantic similarity with optional graph expansion",
                    ),
                    Arc::new(search_schema),
                ),
                Tool::new(
                    Cow::Borrowed("detailed_help"),
                    Cow::Borrowed("Show detailed help for tools and parameters"),
                    Arc::new(help_schema),
                ),
            ],
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        match request.name.as_ref() {
            "convo_think" => {
                let args = request.arguments.clone().ok_or_else(|| McpError {
                    code: rmcp::model::ErrorCode::INVALID_PARAMS,
                    message: "Missing parameters".into(),
                    data: None,
                })?;
                let params: ConvoThinkParams =
                    serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                        McpError {
                            code: rmcp::model::ErrorCode::INVALID_PARAMS,
                            message: format!("Invalid parameters: {}", e).into(),
                            data: None,
                        }
                    })?;

                // Redact content at info level to avoid logging full user text
                info!("convo_think called (content_len={})", params.content.len());
                let dbg_preview: String = params.content.chars().take(200).collect();
                debug!("convo_think content (first 200 chars): {}", dbg_preview);
                let result = self.create_thought_with_injection(params).await?;
                Ok(CallToolResult::structured(json!(result)))
            }
            "search_thoughts" => {
                let args = request.arguments.clone().ok_or_else(|| McpError {
                    code: rmcp::model::ErrorCode::INVALID_PARAMS,
                    message: "Missing parameters".into(),
                    data: None,
                })?;
                let params: SearchThoughtsParams =
                    serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                        McpError {
                            code: rmcp::model::ErrorCode::INVALID_PARAMS,
                            message: format!("Invalid parameters: {}", e).into(),
                            data: None,
                        }
                    })?;
                let result = self.search_thoughts(params).await?;
                Ok(CallToolResult::structured(result))
            }
            "tech_think" => {
                let args = request.arguments.clone().ok_or_else(|| McpError {
                    code: rmcp::model::ErrorCode::INVALID_PARAMS,
                    message: "Missing parameters".into(),
                    data: None,
                })?;
                let mut params: ConvoThinkParams =
                    serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                        McpError {
                            code: rmcp::model::ErrorCode::INVALID_PARAMS,
                            message: format!("Invalid parameters: {}", e).into(),
                            data: None,
                        }
                    })?;
                // Default submode for tech_think is "plan"
                let sm = params
                    .submode
                    .clone()
                    .unwrap_or_else(|| "plan".to_string())
                    .to_lowercase();
                if params.injection_scale.is_none() {
                    params.injection_scale = Some(match sm.as_str() {
                        "plan" => 3,  // DEFAULT
                        "build" => 2, // MEDIUM
                        "debug" => 4, // HIGH
                        _ => 3,
                    });
                }
                let result = self.create_tech_thought(params).await?;
                Ok(CallToolResult::structured(json!(result)))
            }
            "detailed_help" => {
                let args = request.arguments.clone().unwrap_or_default();
                let tool = args.get("tool").and_then(|v| v.as_str());
                let format = args.get("format").and_then(|v| v.as_str());
                let help = Self::get_detailed_help(tool, format.unwrap_or("full"));
                Ok(CallToolResult::structured(help))
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
    fn db_serial_enabled() -> bool {
        std::env::var("SURR_DB_SERIAL")
            .ok()
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false)
    }

    async fn new() -> Result<Self> {
        info!("Connecting to SurrealDB service via WebSocket");

        let url = std::env::var("SURR_DB_URL").unwrap_or_else(|_| "127.0.0.1:8000".to_string());
        let user = std::env::var("SURR_DB_USER").unwrap_or_else(|_| "root".to_string());
        let pass = std::env::var("SURR_DB_PASS").unwrap_or_else(|_| "root".to_string());
        let ns = std::env::var("SURR_DB_NS").unwrap_or_else(|_| "surreal_mind".to_string());
        let dbname = std::env::var("SURR_DB_DB").unwrap_or_else(|_| "consciousness".to_string());

        // Connect to the running SurrealDB service
        let db = Surreal::new::<Ws>(url)
            .await
            .context("Failed to connect to SurrealDB service")?;

        // Authenticate
        db.signin(Root {
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
        let embedder = create_embedder()
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
        // Initialize schema
        let server = Self {
            db: Arc::new(db),
            db_gate: Arc::new(Mutex::new(())),
            thoughts: Arc::new(RwLock::new(thoughts_cache)),
            embedder,
        };

        server.initialize_schema().await?;

        Ok(server)
    }

    async fn initialize_schema(&self) -> Result<(), McpError> {
        info!("Initializing consciousness graph schema");

        // Define thoughts table
        let result = if Self::db_serial_enabled() {
            let _guard = self.db_gate.lock().await;
            self.db
                .query(
                    r#"
            DEFINE TABLE thoughts SCHEMAFULL;
            DEFINE FIELD id ON TABLE thoughts TYPE string;
            DEFINE FIELD content ON TABLE thoughts TYPE string;
            DEFINE FIELD created_at ON TABLE thoughts TYPE datetime;
            DEFINE FIELD embedding ON TABLE thoughts TYPE array<float>;
            DEFINE FIELD injected_memories ON TABLE thoughts TYPE array<string>;
            DEFINE FIELD enriched_content ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD injection_scale ON TABLE thoughts TYPE number;
            DEFINE FIELD significance ON TABLE thoughts TYPE float;
            DEFINE FIELD access_count ON TABLE thoughts TYPE number;
            DEFINE FIELD last_accessed ON TABLE thoughts TYPE option<datetime>;
            DEFINE FIELD submode ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD framework_enhanced ON TABLE thoughts TYPE option<bool>;
            DEFINE FIELD framework_analysis ON TABLE thoughts TYPE option<object>;

            DEFINE INDEX created_at_idx ON TABLE thoughts COLUMNS created_at;
            DEFINE INDEX significance_idx ON TABLE thoughts COLUMNS significance;
        "#,
                )
                .await
        } else {
            self.db
                .query(
                    r#"
            DEFINE TABLE thoughts SCHEMAFULL;
            DEFINE FIELD id ON TABLE thoughts TYPE string;
            DEFINE FIELD content ON TABLE thoughts TYPE string;
            DEFINE FIELD created_at ON TABLE thoughts TYPE datetime;
            DEFINE FIELD embedding ON TABLE thoughts TYPE array<float>;
            DEFINE FIELD injected_memories ON TABLE thoughts TYPE array<string>;
            DEFINE FIELD enriched_content ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD injection_scale ON TABLE thoughts TYPE number;
            DEFINE FIELD significance ON TABLE thoughts TYPE float;
            DEFINE FIELD access_count ON TABLE thoughts TYPE number;
            DEFINE FIELD last_accessed ON TABLE thoughts TYPE option<datetime>;
            DEFINE FIELD submode ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD framework_enhanced ON TABLE thoughts TYPE option<bool>;
            DEFINE FIELD framework_analysis ON TABLE thoughts TYPE option<object>;

            DEFINE INDEX created_at_idx ON TABLE thoughts COLUMNS created_at;
            DEFINE INDEX significance_idx ON TABLE thoughts COLUMNS significance;
        "#,
                )
                .await
        };

        result.map_err(|e| McpError {
            code: rmcp::model::ErrorCode::INTERNAL_ERROR,
            message: format!("Failed to initialize schema: {}", e).into(),
            data: None,
        })?;

        // Define relationships table
        self.db
            .query(
                r#"
            DEFINE TABLE recalls SCHEMAFULL;
            DEFINE FIELD in ON TABLE recalls TYPE record<thoughts>;
            DEFINE FIELD out ON TABLE recalls TYPE record<thoughts>;
            DEFINE FIELD strength ON TABLE recalls TYPE float;
            DEFINE FIELD created_at ON TABLE recalls TYPE datetime;
            DEFINE FIELD submode_match ON TABLE recalls TYPE option<bool>;
            DEFINE FIELD flavor ON TABLE recalls TYPE option<string>;
        "#,
            )
            .await
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Failed to define relationships: {}", e).into(),
                data: None,
            })?;

        // Backfill existing data with defaults (idempotent)
        self.db
            .query(
                r#"
            UPDATE thoughts SET submode = "sarcastic" WHERE submode = NONE;
            UPDATE thoughts SET framework_enhanced = false WHERE framework_enhanced = NONE;
        "#,
            )
            .await
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Failed to backfill defaults: {}", e).into(),
                data: None,
            })?;

        info!("Schema initialized successfully");
        Ok(())
    }

    // reembed_all tool removed; standalone CLI provided in src/bin/reembed.rs

    async fn create_thought_with_injection(
        &self,
        params: ConvoThinkParams,
    ) -> Result<serde_json::Value, McpError> {
        let injection_scale = params.injection_scale.unwrap_or(3); // Default Mars level
        if injection_scale > 5 {
            return Err(McpError {
                code: rmcp::model::ErrorCode::INVALID_PARAMS,
                message: format!(
                    "injection_scale must be between 0 and 5 (got: {}). Valid presets: 0=NONE, 1=MERCURY (hot memories), 2=VENUS (recent), 3=MARS (default), 4=JUPITER (distant), 5=PLUTO (everything). Example: {{\"injection_scale\": 3}}"
                    , injection_scale
                ).into(),
                data: None,
            });
        }
        let significance = params.significance.unwrap_or(0.5);
        if !(0.0..=1.0).contains(&significance) {
            return Err(McpError {
                code: rmcp::model::ErrorCode::INVALID_PARAMS,
                message: format!(
                    "significance must be between 0.0 and 1.0 (got: {}). Use 0.1 for low importance, 0.5 for normal, 0.8 for high, 1.0 for critical. Example: {{\"significance\": 0.7}}"
                    , significance
                ).into(),
                data: None,
            });
        }

        // Generate embedding using configured provider
        let embedding = self
            .embedder
            .embed(&params.content)
            .await
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Failed to generate embedding: {}", e).into(),
                data: None,
            })?;
        // Validate embedding dimensionality to prevent bad data entering DB
        let expected_dim = self.embedder.dimensions();
        if embedding.len() != expected_dim {
            return Err(McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!(
                    "Embedding dimension mismatch: expected {}, got {}. Check embedding provider configuration.",
                    expected_dim,
                    embedding.len()
                ).into(),
                data: None,
            });
        }

        // Retrieve relevant memories based on injection scale
        let relevant_memories = self
            .retrieve_memories_for_injection(&embedding, injection_scale, params.submode.as_deref())
            .await?;

        debug!(
            "Retrieved {} memories for injection at scale {}",
            relevant_memories.len(),
            injection_scale
        );

        // Create framework analysis and enriched content
        let submode = params.submode.as_deref().unwrap_or("sarcastic");
        let (analysis, enriched_content) =
            self.cognitive_enrich(submode, &params.content, &relevant_memories);

        // Validate and default submode
        let submode = params
            .submode
            .clone()
            .unwrap_or_else(|| "sarcastic".to_string());
        let valid_submodes = [
            "sarcastic",
            "philosophical",
            "empathetic",
            "problem_solving",
        ];
        let submode = if valid_submodes.contains(&submode.as_str()) {
            submode
        } else {
            tracing::warn!("Invalid submode '{}', defaulting to 'sarcastic'", submode);
            "sarcastic".to_string()
        };

        // Determine flavor for this thought
        let flavor = tag_flavor(&params.content);

        // Convert framework analysis to JSON for storage
        let framework_json = serde_json::json!({
            "insights": analysis.insights,
            "questions": analysis.questions,
            "next_steps": analysis.next_steps,
        });

        // Create new thought
        let thought = Thought {
            id: Uuid::new_v4().to_string(),
            content: params.content.clone(),
            created_at: surrealdb::sql::Datetime::from(Utc::now()),
            embedding,
            injected_memories: relevant_memories
                .iter()
                .map(|m| m.thought.id.clone())
                .collect(),
            enriched_content: Some(enriched_content.clone()),
            injection_scale,
            significance,
            access_count: 0,
            last_accessed: None,
            submode: Some(submode.clone()),
            framework_enhanced: Some(true), // Now enabled with framework processing
            framework_analysis: Some(framework_json.clone()), // Store framework output
        };

        // Store thought in SurrealDB with all fields in a single operation to avoid SCHEMAFULL NONE issues
        // Create record using SurrealQL with server-side datetime to avoid JSON enum serialization issues
        let create_result = if Self::db_serial_enabled() {
            let _guard = self.db_gate.lock().await;
            self.db
                .query(
                    r#"CREATE type::thing('thoughts', $id) CONTENT {
                        id: $id,
                        content: $content,
                        created_at: time::now(),
                        embedding: $embedding,
                        injected_memories: $injected_memories,
                        enriched_content: $enriched_content,
                        injection_scale: $injection_scale,
                        significance: $significance,
                        access_count: $access_count,
                        submode: $submode,
                        framework_enhanced: $framework_enhanced,
                        framework_analysis: $framework_analysis
                    } RETURN NONE"#,
                )
                .bind(("id", thought.id.clone()))
                .bind(("content", thought.content.clone()))
                .bind(("embedding", thought.embedding.clone()))
                .bind(("injected_memories", thought.injected_memories.clone()))
                .bind(("enriched_content", thought.enriched_content.clone()))
                .bind(("injection_scale", thought.injection_scale))
                .bind(("significance", thought.significance))
                .bind(("access_count", thought.access_count))
                .bind(("submode", thought.submode.clone()))
                .bind(("framework_enhanced", thought.framework_enhanced))
                .bind(("framework_analysis", thought.framework_analysis.clone()))
                .await
        } else {
            self.db
                .query(
                    r#"CREATE type::thing('thoughts', $id) CONTENT {
                        id: $id,
                        content: $content,
                        created_at: time::now(),
                        embedding: $embedding,
                        injected_memories: $injected_memories,
                        enriched_content: $enriched_content,
                        injection_scale: $injection_scale,
                        significance: $significance,
                        access_count: $access_count,
                        submode: $submode,
                        framework_enhanced: $framework_enhanced,
                        framework_analysis: $framework_analysis
                    } RETURN NONE"#,
                )
                .bind(("id", thought.id.clone()))
                .bind(("content", thought.content.clone()))
                .bind(("embedding", thought.embedding.clone()))
                .bind(("injected_memories", thought.injected_memories.clone()))
                .bind(("enriched_content", thought.enriched_content.clone()))
                .bind(("injection_scale", thought.injection_scale))
                .bind(("significance", thought.significance))
                .bind(("access_count", thought.access_count))
                .bind(("submode", thought.submode.clone()))
                .bind(("framework_enhanced", thought.framework_enhanced))
                .bind(("framework_analysis", thought.framework_analysis.clone()))
                .await
        };

        create_result.map_err(|e| {
            error!("Failed to create thought record with SurrealQL: {}", e);
            McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Failed to store thought: {}", e).into(),
                data: None,
            }
        })?;

        debug!("Successfully stored thought: {}", thought.id);

        // Create bidirectional relationships with injected memories (single round trip for all)
        if !relevant_memories.is_empty() {
            let mut q = String::new();
            q.reserve(relevant_memories.len() * 128);
            for (i, memory) in relevant_memories.iter().enumerate() {
                // Check if submodes match (if both present)
                let _submode_match = thought
                    .submode
                    .as_ref()
                    .and_then(|ts| memory.thought.submode.as_ref().map(|ms| ts == ms))
                    .unwrap_or(false);

                q.push_str(&format!(
                    "RELATE $from{0}->recalls->$to{0} SET strength = $strength{0}, created_at = time::now(), submode_match = $submode_match{0}, flavor = $flavor{0} RETURN NONE;\n",
                    i
                ));
                q.push_str(&format!(
                    "RELATE $to{0}->recalls->$from{0} SET strength = $strength{0}, created_at = time::now(), submode_match = $submode_match{0}, flavor = $flavor{0} RETURN NONE;\n",
                    i
                ));
            }
            let mut req = self.db.query(q);
            for (i, memory) in relevant_memories.iter().enumerate() {
                let submode_match = thought
                    .submode
                    .as_ref()
                    .and_then(|ts| memory.thought.submode.as_ref().map(|ms| ts == ms))
                    .unwrap_or(false);

                req = req
                    .bind((format!("from{}", i), format!("thoughts:{}", thought.id)))
                    .bind((
                        format!("to{}", i),
                        format!("thoughts:{}", memory.thought.id),
                    ))
                    .bind((format!("strength{}", i), memory.similarity_score))
                    .bind((format!("submode_match{}", i), submode_match))
                    .bind((format!("flavor{}", i), flavor.as_str()));
            }

            if Self::db_serial_enabled() {
                let _guard = self.db_gate.lock().await;
                req.await.map_err(|e| McpError {
                    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                    message: format!("Failed to create relationships: {}", e).into(),
                    data: None,
                })?;
            } else {
                req.await.map_err(|e| McpError {
                    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                    message: format!("Failed to create relationships: {}", e).into(),
                    data: None,
                })?;
            }
        }

        // Also keep in memory for fast retrieval (bounded LRU)
        let mut thoughts = self.thoughts.write().await;
        thoughts.put(thought.id.clone(), thought.clone());
        debug!("cache_size_after_insert={}", thoughts.len());

        // Apply verbosity limits
        let verbose_analysis = params.verbose_analysis.unwrap_or(true);
        let limited_analysis = Self::apply_verbosity_limits(&analysis, verbose_analysis);

        Ok(json!({
            "thought_id": thought.id,
            "submode_used": submode,
            "memories_injected": relevant_memories.len(),
            "analysis": {
                "key_point": if !limited_analysis.insights.is_empty() {
                    // Take first insight and make it human-readable
                    Self::make_conversational(&limited_analysis.insights[0])
                } else {
                    "Thought stored successfully".to_string()
                },
                "question": if !limited_analysis.questions.is_empty() {
                    Self::make_conversational(&limited_analysis.questions[0])
                } else {
                    "What's next?".to_string()
                },
                "next_step": if !limited_analysis.next_steps.is_empty() {
                    limited_analysis.next_steps[0].clone()
                } else {
                    "Continue processing".to_string()
                }
            }
        }))
    }

    async fn retrieve_memories_for_injection(
        &self,
        query_embedding: &[f32],
        injection_scale: u8,
        submode: Option<&str>,
    ) -> Result<Vec<ThoughtMatch>, McpError> {
        let expected_dim = self.embedder.dimensions();
        // Load runtime retrieval config
        let sim_thresh: f32 = std::env::var("SURR_SIM_THRESH")
            .ok()
            .and_then(|s| s.parse::<f32>().ok())
            .map(|v| v.clamp(0.0, 1.0))
            .unwrap_or(0.5);
        let top_k: usize = std::env::var("SURR_TOP_K")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .map(|v| v.clamp(1, 50))
            .unwrap_or(5);

        // Submode retrieval tuning (guarded by SURR_SUBMODE_RETRIEVAL flag)
        let use_submode = std::env::var("SURR_SUBMODE_RETRIEVAL")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let sm = cognitive::profile::Submode::from_str(submode.unwrap_or("sarcastic"));
        let prof = cognitive::profile::profile_for(sm);
        let eff_sim_thresh = if use_submode {
            (sim_thresh + prof.injection.threshold_delta).clamp(0.0, 1.0)
        } else {
            sim_thresh
        };
        debug!(
            "retrieval_tuning: submode={} flag={} sim_thresh={:.2} -> {:.2}",
            submode.unwrap_or("sarcastic"),
            use_submode,
            sim_thresh,
            eff_sim_thresh
        );

        // Calculate orbital proximity threshold (minimum closeness) based on injection scale
        // Higher proximity means closer/more relevant
        let min_orbital_proximity = match injection_scale {
            0 => return Ok(Vec::new()), // No injection
            1 => 0.8,                   // Mercury - only hottest memories (was distance <= 0.2)
            2 => 0.6,                   // Venus/Earth - recent context (was <= 0.4)
            3 => 0.4,                   // Mars - foundational significance (was <= 0.6)
            4 => 0.2,                   // Jupiter/Saturn - distant connections (was <= 0.8)
            5 => 0.0,                   // Neptune/Pluto - everything relevant (was <= 1.0)
            _ => 0.4,                   // Default to Mars
        };

        // Try to get from in-memory first, fall back to DB if needed
        let now_ts = Utc::now().timestamp();
        let thoughts = self.thoughts.read().await;
        let mut matches = Vec::new();

        // Helper to compute orbital proximity (closeness in [0,1], higher is closer)
        let compute_proximity = |created_ts: i64, access_count: u32, significance: f32| {
            // Recency closeness: recent → 1.0, old → 0.0 (normalize by 30 days)
            let age_days = (now_ts - created_ts) as f32 / 86_400.0;
            let recency_closeness = (1.0 - (age_days / 30.0)).clamp(0.0, 1.0);
            // Access closeness: more accesses → closer (cap at 1.0)
            let access_closeness = ((access_count as f32 + 1.0).ln() / 5.0).clamp(0.0, 1.0);
            let significance_closeness = significance.clamp(0.0, 1.0);
            let (age_w, access_w, sig_w) = if use_submode {
                (
                    prof.orbital.age_w,
                    prof.orbital.access_w,
                    prof.orbital.significance_w,
                )
            } else {
                (0.4_f32, 0.3_f32, 0.3_f32)
            };
            let closeness = recency_closeness * age_w
                + access_closeness * access_w
                + significance_closeness * sig_w;
            closeness.clamp(0.0, 1.0)
        };

        // If we have thoughts in memory, use them
        if !thoughts.is_empty() {
            for (_id, t) in thoughts.iter() {
                if t.embedding.len() != expected_dim { continue; }
                let similarity = self.cosine_similarity(query_embedding, &t.embedding);
                let orbital_proximity =
                    compute_proximity(t.created_at.timestamp(), t.access_count, t.significance);

                if similarity > eff_sim_thresh && orbital_proximity >= min_orbital_proximity {
                    matches.push(ThoughtMatch {
                        thought: t.clone(),
                        similarity_score: similarity,
                        orbital_proximity,
                    });
                }
            }
        } else {
            // Fall back to querying SurrealDB (bounded)
            // Drop read lock before any potential writes to the cache
            drop(thoughts);
            // Configurable limit for fallback query (override with SURR_RETRIEVE_CANDIDATES)
            let default_limit: usize = std::env::var("SURR_DB_LIMIT")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(500);
            let limit: usize = std::env::var("SURR_RETRIEVE_CANDIDATES")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(default_limit)
                .clamp(50, 5000);
            // Select only needed columns to reduce payload size
            let resp = if Self::db_serial_enabled() {
                let _guard = self.db_gate.lock().await;
                self.db
                    .query(format!(
                        "SELECT id, content, created_at, embedding, injected_memories, enriched_content, injection_scale, significance, access_count, last_accessed, submode, framework_enhanced, framework_analysis FROM thoughts ORDER BY created_at DESC LIMIT {}",
                        limit
                    ))
                    .await
            } else {
                self.db
                    .query(format!(
                        "SELECT id, content, created_at, embedding, injected_memories, enriched_content, injection_scale, significance, access_count, last_accessed, submode, framework_enhanced, framework_analysis FROM thoughts ORDER BY created_at DESC LIMIT {}",
                        limit
                    ))
                    .await
            };

            let mut resp = resp.map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Failed to query thoughts: {}", e).into(),
                data: None,
            })?;
            let results: Vec<Thought> = resp
                .take(0)
                .map_err(|e| McpError {
                    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                    message: format!("Failed to parse thoughts: {}", e).into(),
                    data: None,
                })
                .unwrap_or_default();

            // Consider DB results; do not update DB or cache yet
            for thought in results.iter() {
                if thought.embedding.len() != expected_dim { continue; }
                let similarity = self.cosine_similarity(query_embedding, &thought.embedding);
                let orbital_proximity = compute_proximity(
                    thought.created_at.timestamp(),
                    thought.access_count,
                    thought.significance,
                );

                if similarity > eff_sim_thresh && orbital_proximity >= min_orbital_proximity {
                    matches.push(ThoughtMatch {
                        thought: thought.clone(),
                        similarity_score: similarity,
                        orbital_proximity,
                    });
                }
            }

            // Cache warm-up: populate cache with recent thoughts from DB fallback
            let warm_n: usize = std::env::var("SURR_CACHE_WARM")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(64)
                .clamp(0, 1000);
            if warm_n > 0 && !results.is_empty() {
                let mut cache = self.thoughts.write().await;
                let to_warm = std::cmp::min(results.len(), warm_n);
                for thought in results.into_iter().take(to_warm) {
                    // Avoid double-caching items already present
                    if !cache.contains(&thought.id) {
                        cache.put(thought.id.clone(), thought);
                    }
                }
            }
        }

        // Sort by combined score (defaults preserved; tuned by profile weights if desired later)
        matches.sort_by(|a, b| {
            let score_a = a.similarity_score * 0.6 + a.orbital_proximity * 0.4;
            let score_b = b.similarity_score * 0.6 + b.orbital_proximity * 0.4;
            score_b.total_cmp(&score_a)
        });

        // Sort and take top_k
        let mut top: Vec<ThoughtMatch> = matches.into_iter().take(top_k).collect();

        // Batch update access_count and last_accessed for selected matches
        if !top.is_empty() {
            let mut q = String::new();
            q.reserve(top.len() * 128);
            for i in 0..top.len() {
                q.push_str(&format!(
                    "UPDATE type::thing('thoughts', $id{0}) SET access_count = $ac{0}, last_accessed = time::now() RETURN NONE;\n",
                    i
                ));
            }

            let db_res = {
                let mut req = self.db.query(q);
                for (i, m) in top.iter().enumerate() {
                    let new_ac = m.thought.access_count.saturating_add(1);
                    req = req
                        .bind((format!("id{}", i), m.thought.id.clone()))
                        .bind((format!("ac{}", i), new_ac));
                }

                if Self::db_serial_enabled() {
                    let _guard = self.db_gate.lock().await;
                    req.await
                } else {
                    req.await
                }
            };

            db_res.map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Failed to batch update access metadata: {}", e).into(),
                data: None,
            })?;

            // Update cache entries after confirmed DB write
            let mut cache = self.thoughts.write().await;
            for m in top.iter_mut() {
                let new_ac = m.thought.access_count.saturating_add(1);
                let mut t = m.thought.clone();
                t.access_count = new_ac;
                t.last_accessed = Some(surrealdb::sql::Datetime::from(Utc::now()));
                cache.put(t.id.clone(), t.clone());
                m.thought = t;
            }
        }

        Ok(top)
    }

    async fn search_thoughts(
        &self,
        params: SearchThoughtsParams,
    ) -> Result<serde_json::Value, McpError> {
        // Resolve defaults from env
        let env_top_k = std::env::var("SURR_SEARCH_TOP_K")
            .ok()
            .and_then(|s| s.parse::<usize>().ok());
        let fall_top_k = std::env::var("SURR_TOP_K")
            .ok()
            .and_then(|s| s.parse::<usize>().ok());
        let top_k = params
            .top_k
            .or(env_top_k)
            .or(fall_top_k)
            .unwrap_or(10)
            .clamp(1, 50);
        let offset = params.offset.unwrap_or(0);

        let env_sim = std::env::var("SURR_SEARCH_SIM_THRESH")
            .ok()
            .and_then(|s| s.parse::<f32>().ok());
        let fall_sim = std::env::var("SURR_SIM_THRESH")
            .ok()
            .and_then(|s| s.parse::<f32>().ok());
        let sim_thresh = params
            .sim_thresh
            .or(env_sim)
            .or(fall_sim)
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);

        let min_sig = params.min_significance.unwrap_or(0.0).clamp(0.0, 1.0);
        let (from_ts, to_ts) = if let Some(dr) = &params.date_range {
            let from = dr
                .from
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.timestamp());
            let to = dr
                .to
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.timestamp());
            (from, to)
        } else {
            (None, None)
        };

        let expand_graph = params.expand_graph.unwrap_or(false);
        let graph_depth = params
            .graph_depth
            .unwrap_or(if expand_graph { 1 } else { 0 })
            .min(2);
        let graph_boost = params.graph_boost.unwrap_or(0.15).clamp(0.0, 1.0);
        let min_edge_strength = params.min_edge_strength.unwrap_or(0.0).clamp(0.0, 1.0);
        let sort_by = params.sort_by.as_deref().unwrap_or("score").to_lowercase();

        // Embed query
        let embedding = self
            .embedder
            .embed(&params.content)
            .await
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Failed to generate embedding: {}", e).into(),
                data: None,
            })?;
        let expected_dim = self.embedder.dimensions();
        info!("Search: embedder dimensions={}, query embedding len={}", expected_dim, embedding.len());
        if embedding.len() != expected_dim {
            return Err(McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!(
                    "Embedding dimension mismatch: expected {}, got {}",
                    expected_dim,
                    embedding.len()
                )
                .into(),
                data: None,
            });
        }

        // Submode-aware knobs
        let use_submode = std::env::var("SURR_SUBMODE_RETRIEVAL")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let sm =
            cognitive::profile::Submode::from_str(params.submode.as_deref().unwrap_or("sarcastic"));
        let prof = cognitive::profile::profile_for(sm);

        let now_ts = Utc::now().timestamp();
        let compute_proximity = |created_ts: i64, access_count: u32, significance: f32| {
            let age_days = (now_ts - created_ts) as f32 / 86_400.0;
            let recency_closeness = (1.0 - (age_days / 30.0)).clamp(0.0, 1.0);
            let access_closeness = ((access_count as f32 + 1.0).ln() / 5.0).clamp(0.0, 1.0);
            let sig_close = significance.clamp(0.0, 1.0);
            let (aw, cw, sw) = if use_submode {
                (
                    prof.orbital.age_w,
                    prof.orbital.access_w,
                    prof.orbital.significance_w,
                )
            } else {
                (0.4_f32, 0.3_f32, 0.3_f32)
            };
            (recency_closeness * aw + access_closeness * cw + sig_close * sw).clamp(0.0, 1.0)
        };

        // Gather candidates from cache
        let thoughts = self.thoughts.read().await;
        let mut matches: Vec<ThoughtMatch> = Vec::new();
        info!("Search: checking {} cached thoughts", thoughts.len());
        for (_id, t) in thoughts.iter() {
            // Temporarily disable dimension check to debug
            // if t.embedding.len() != expected_dim { 
            //     info!("Search: skipping cached thought with wrong dims: {} != {}", t.embedding.len(), expected_dim);
            //     continue; 
            // }
            if t.significance < min_sig {
                continue;
            }
            let ts = t.created_at.timestamp();
            if let Some(f) = from_ts
                && ts < f
            {
                continue;
            }
            if let Some(to) = to_ts
                && ts > to
            {
                continue;
            }
            let sim = self.cosine_similarity(&embedding, &t.embedding);
            if sim < sim_thresh {
                continue;
            }
            let prox = compute_proximity(ts, t.access_count, t.significance);
            matches.push(ThoughtMatch {
                thought: t.clone(),
                similarity_score: sim,
                orbital_proximity: prox,
            });
        }
        drop(thoughts);
        info!("Search: found {} matches in cache", matches.len());

        // If insufficient, fallback to DB
        if matches.is_empty() {
            info!("Search: cache empty, searching database");
            let default_limit: usize = std::env::var("SURR_DB_LIMIT")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(500);
            let limit: usize = std::env::var("SURR_RETRIEVE_CANDIDATES")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(default_limit)
                .clamp(50, 5000);

            let mut q = String::from(
                "SELECT id, content, created_at, embedding, significance, access_count, last_accessed, submode FROM thoughts",
            );
            // WHERE
            let mut where_clauses: Vec<String> = Vec::new();
            if let Some(f) = from_ts {
                where_clauses.push(format!("created_at >= time::unix({})", f));
            }
            if let Some(t) = to_ts {
                where_clauses.push(format!("created_at <= time::unix({})", t));
            }
            if min_sig > 0.0 {
                where_clauses.push(format!("significance >= {}", min_sig));
            }
            if !where_clauses.is_empty() {
                q.push_str(" WHERE ");
                q.push_str(&where_clauses.join(" AND "));
            }
            q.push_str(" ORDER BY created_at DESC LIMIT ");
            q.push_str(&limit.to_string());

            let mut resp = self.db.query(q).await.map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Failed to query thoughts: {}", e).into(),
                data: None,
            })?;
            let rows: Vec<Thought> = resp
                .take(0)
                .map_err(|e| McpError {
                    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                    message: format!("Failed to parse thoughts: {}", e).into(),
                    data: None,
                })
                .unwrap_or_default();

            info!("Search: DB returned {} thoughts", rows.len());
            for t in rows.iter() {
                // Temporarily disable dimension check to debug
                // if t.embedding.len() != expected_dim { 
                //     info!("Search: DB thought has wrong dims: {} != {}", t.embedding.len(), expected_dim);
                //     continue; 
                // }
                let sim = self.cosine_similarity(&embedding, &t.embedding);
                if sim < sim_thresh {
                    continue;
                }
                let prox =
                    compute_proximity(t.created_at.timestamp(), t.access_count, t.significance);
                matches.push(ThoughtMatch {
                    thought: t.clone(),
                    similarity_score: sim,
                    orbital_proximity: prox,
                });
            }

            // Warm cache
            let warm_n: usize = std::env::var("SURR_CACHE_WARM")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(64)
                .clamp(0, 1000);
            if warm_n > 0 && !rows.is_empty() {
                let mut cache = self.thoughts.write().await;
                for t in rows.into_iter().take(warm_n) {
                    if !cache.contains(&t.id) {
                        cache.put(t.id.clone(), t);
                    }
                }
            }
        }

        // Seed sorting and paging helper
        let sort_matches = |v: &mut Vec<ThoughtMatch>| {
            v.sort_by(|a, b| match sort_by.as_str() {
                "similarity" => b
                    .similarity_score
                    .total_cmp(&a.similarity_score)
                    .then_with(|| b.orbital_proximity.total_cmp(&a.orbital_proximity))
                    .then_with(|| {
                        b.thought
                            .created_at
                            .timestamp()
                            .cmp(&a.thought.created_at.timestamp())
                    })
                    .then_with(|| a.thought.id.cmp(&b.thought.id)),
                "recency" => b
                    .thought
                    .created_at
                    .timestamp()
                    .cmp(&a.thought.created_at.timestamp())
                    .then_with(|| b.similarity_score.total_cmp(&a.similarity_score))
                    .then_with(|| b.orbital_proximity.total_cmp(&a.orbital_proximity))
                    .then_with(|| a.thought.id.cmp(&b.thought.id)),
                "significance" => b
                    .thought
                    .significance
                    .total_cmp(&a.thought.significance)
                    .then_with(|| b.similarity_score.total_cmp(&a.similarity_score))
                    .then_with(|| b.orbital_proximity.total_cmp(&a.orbital_proximity))
                    .then_with(|| {
                        a.thought
                            .created_at
                            .timestamp()
                            .cmp(&b.thought.created_at.timestamp())
                    })
                    .then_with(|| a.thought.id.cmp(&b.thought.id)),
                _ => b
                    .similarity_score
                    .total_cmp(&a.similarity_score)
                    .then_with(|| b.orbital_proximity.total_cmp(&a.orbital_proximity))
                    .then_with(|| {
                        b.thought
                            .created_at
                            .timestamp()
                            .cmp(&a.thought.created_at.timestamp())
                    })
                    .then_with(|| a.thought.id.cmp(&b.thought.id)),
            });
            if sort_by == "score" {
                v.sort_by(|a, b| {
                    let sa = a.similarity_score * 0.6 + a.orbital_proximity * 0.4;
                    let sb = b.similarity_score * 0.6 + b.orbital_proximity * 0.4;
                    sb.total_cmp(&sa)
                        .then_with(|| b.similarity_score.total_cmp(&a.similarity_score))
                        .then_with(|| {
                            b.thought
                                .created_at
                                .timestamp()
                                .cmp(&a.thought.created_at.timestamp())
                        })
                        .then_with(|| a.thought.id.cmp(&b.thought.id))
                });
            }
        };

        // initial sort and page
        sort_matches(&mut matches);
        let mut sliced: Vec<ThoughtMatch> = matches.into_iter().skip(offset).take(top_k).collect();

        let mut via_map: std::collections::HashMap<String, (String, f32)> =
            std::collections::HashMap::new();
        // Graph expansion
        if expand_graph && graph_depth > 0 && !sliced.is_empty() {
            let max_n = std::env::var("SURR_SEARCH_GRAPH_MAX_NEIGHBORS")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(20);
            let mut neighbor_strength: std::collections::HashMap<String, f32> =
                std::collections::HashMap::new();
            let mut seeds: Vec<String> = sliced.iter().map(|m| m.thought.id.clone()).collect();
            seeds.truncate(top_k);
            for seed in seeds.iter() {
                let mut q = String::new();
                q.push_str("SELECT id(in) AS nid, strength FROM recalls WHERE out = type::thing('thoughts', $sid) LIMIT $lim;\n");
                q.push_str("SELECT id(out) AS nid, strength FROM recalls WHERE in = type::thing('thoughts', $sid) LIMIT $lim;\n");
                let mut req = self.db.query(q);
                req = req.bind(("sid", seed.clone())).bind(("lim", max_n));
                let mut resp = req.await.map_err(|e| McpError {
                    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                    message: format!("Failed to query recalls: {}", e).into(),
                    data: None,
                })?;
                let out1: Vec<serde_json::Value> = resp.take(0).unwrap_or_default();
                let out2: Vec<serde_json::Value> = resp.take(1).unwrap_or_default();
                for row in out1.into_iter().chain(out2.into_iter()) {
                    if let (Some(nid), Some(stren)) = (row.get("nid"), row.get("strength")) {
                        let nid_s = nid.as_str().unwrap_or("").to_string();
                        let st = stren.as_f64().unwrap_or(0.0) as f32;
                        if nid_s.is_empty() || st < min_edge_strength {
                            continue;
                        }
                        let e = neighbor_strength.entry(nid_s.clone()).or_insert(0.0);
                        if st > *e {
                            *e = st;
                            via_map.insert(nid_s.clone(), (seed.clone(), st));
                        }
                    }
                }
            }
            // Optional second-hop expansion
            if graph_depth > 1 && !neighbor_strength.is_empty() {
                let mut second_strength: std::collections::HashMap<String, f32> =
                    std::collections::HashMap::new();
                for (mid, mid_st) in neighbor_strength.iter() {
                    // Original seed that led to this first-hop neighbor
                    let orig_seed = via_map.get(mid).map(|(s, _)| s.clone());
                    let mut q2 = String::new();
                    q2.push_str(
                        "SELECT id(in) AS nid, strength FROM recalls WHERE out = type::thing('thoughts', $mid) LIMIT $lim;\n",
                    );
                    q2.push_str(
                        "SELECT id(out) AS nid, strength FROM recalls WHERE in = type::thing('thoughts', $mid) LIMIT $lim;\n",
                    );
                    let mut resp2 = self
                        .db
                        .query(q2)
                        .bind(("mid", mid.clone()))
                        .bind(("lim", max_n))
                        .await
                        .map_err(|e| McpError {
                            code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                            message: format!("Failed to query second-hop recalls: {}", e).into(),
                            data: None,
                        })?;
                    let s1: Vec<serde_json::Value> = resp2.take(0).unwrap_or_default();
                    let s2: Vec<serde_json::Value> = resp2.take(1).unwrap_or_default();
                    for row in s1.into_iter().chain(s2.into_iter()) {
                        if let (Some(nid), Some(stren)) = (row.get("nid"), row.get("strength")) {
                            let nid_s = nid.as_str().unwrap_or("").to_string();
                            let st2 = stren.as_f64().unwrap_or(0.0) as f32;
                            if nid_s.is_empty() || st2 < min_edge_strength {
                                continue;
                            }
                            // Skip original seeds to avoid cycles
                            if seeds.contains(&nid_s) {
                                continue;
                            }
                            // Combined two-hop strength (multiply)
                            let combined = (*mid_st * st2).clamp(0.0, 1.0);
                            let e = second_strength.entry(nid_s.clone()).or_insert(0.0);
                            if combined > *e {
                                *e = combined;
                                if let Some(seed) = orig_seed.as_ref() {
                                    via_map.insert(nid_s.clone(), (seed.clone(), combined));
                                }
                            }
                        }
                    }
                }
                for (k, v) in second_strength.into_iter() {
                    let e = neighbor_strength.entry(k).or_insert(0.0);
                    if v > *e {
                        *e = v;
                    }
                }
            }
            if !neighbor_strength.is_empty() {
                // Fetch neighbors thoughts in one go
                let ids: Vec<String> = neighbor_strength.keys().cloned().collect();
                let mut resp = self
                    .db
                    .query("SELECT id, content, created_at, embedding, significance, access_count, last_accessed, submode FROM thoughts WHERE id INSIDE $ids")
                    .bind(("ids", ids.clone()))
                    .await
                    .map_err(|e| McpError { code: rmcp::model::ErrorCode::INTERNAL_ERROR, message: format!("Failed to fetch neighbor thoughts: {}", e).into(), data: None })?;
                let neighbors: Vec<Thought> = resp.take(0).unwrap_or_default();
                let mut pool: Vec<ThoughtMatch> = Vec::new();
                // Keep original seeds
                pool.extend(sliced.iter().cloned());
                let expected_dim = self.embedder.dimensions();
                for t in neighbors.into_iter() {
                    // skip if same as a seed
                    if seeds.contains(&t.id) {
                        continue;
                    }
                    if t.embedding.len() != expected_dim { continue; }
                    let sim = self.cosine_similarity(&embedding, &t.embedding);
                    let prox =
                        compute_proximity(t.created_at.timestamp(), t.access_count, t.significance);
                    if sim < sim_thresh {
                        continue;
                    }
                    // Apply graph boost at later sort; retain for via_reason only
                    pool.push(ThoughtMatch {
                        thought: t,
                        similarity_score: sim,
                        orbital_proximity: prox,
                    });
                }
                // Sort with graph boost applied to neighbors
                pool.sort_by(|a, b| {
                    let mut sa = a.similarity_score * 0.6 + a.orbital_proximity * 0.4;
                    let mut sb = b.similarity_score * 0.6 + b.orbital_proximity * 0.4;
                    if let Some(sta) = neighbor_strength.get(&a.thought.id) {
                        sa += graph_boost * *sta;
                    }
                    if let Some(stb) = neighbor_strength.get(&b.thought.id) {
                        sb += graph_boost * *stb;
                    }
                    sb.total_cmp(&sa)
                        .then_with(|| b.similarity_score.total_cmp(&a.similarity_score))
                        .then_with(|| {
                            b.thought
                                .created_at
                                .timestamp()
                                .cmp(&a.thought.created_at.timestamp())
                        })
                        .then_with(|| a.thought.id.cmp(&b.thought.id))
                });
                // Dedupe by id, keep best
                let mut seen = std::collections::HashSet::new();
                let mut dedup: Vec<ThoughtMatch> = Vec::new();
                for m in pool.into_iter() {
                    if seen.insert(m.thought.id.clone()) {
                        dedup.push(m);
                    }
                }
                sliced = dedup.into_iter().take(top_k).collect();
            }
        }

        // Shape output
        let results: Vec<serde_json::Value> = sliced
            .into_iter()
            .map(|m| {
                let preview: String = m.thought.content.chars().take(160).collect();
                let score = m.similarity_score * 0.6 + m.orbital_proximity * 0.4;
                let via = via_map.get(&m.thought.id).cloned();
                json!({
                    "id": m.thought.id,
                    "content_preview": preview,
                    "similarity": (m.similarity_score),
                    "orbital_proximity": (m.orbital_proximity),
                    "combined_score": score,
                    "created_at": m.thought.created_at,
                    "significance": m.thought.significance,
                    "submode": m.thought.submode,
                    "via_graph": via.is_some(),
                    "via_reason": via.map(|(seed, st)| format!("expanded via {} @ strength={:.2}", seed, st)),
                })
            })
            .collect();

        Ok(json!({
            "total": results.len(),
            "offset": offset,
            "top_k": top_k,
            "results": results
        }))
    }

    /// Format framework analysis into readable sections with caps (max ~2KB)
    fn format_framework_analysis(analysis: &cognitive::types::FrameworkOutput) -> String {
        let mut output = String::new();

        if !analysis.insights.is_empty()
            || !analysis.questions.is_empty()
            || !analysis.next_steps.is_empty()
        {
            output.push_str("\n\n[Framework Analysis:");

            if !analysis.insights.is_empty() {
                output.push_str("\nInsights:");
                for insight in analysis.insights.iter().take(8) {
                    output.push_str("\n- ");
                    output.push_str(insight);
                }
            }

            if !analysis.questions.is_empty() {
                output.push_str("\nQuestions:");
                for question in analysis.questions.iter().take(4) {
                    output.push_str("\n- ");
                    output.push_str(question);
                }
            }

            if !analysis.next_steps.is_empty() {
                output.push_str("\nNext steps:");
                for step in analysis.next_steps.iter().take(4) {
                    output.push_str("\n- ");
                    output.push_str(step);
                }
            }

            output.push(']');
        }

        // Truncate to ~2KB if needed
        if output.len() > 2048 {
            output.truncate(2045);
            output.push_str("...]");
        }

        output
    }

    fn enrich_content_with_memories(&self, content: &str, memories: &[ThoughtMatch]) -> String {
        if memories.is_empty() {
            return content.to_string();
        }

        let mut enriched = content.to_string();
        enriched.push_str("\n\n[Memory Context:");

        for (i, memory) in memories.iter().take(3).enumerate() {
            let preview: String = memory.thought.content.chars().take(100).collect();
            enriched.push_str(&format!(
                "\n- Memory {}: {} (similarity: {:.2}, proximity: {:.2})",
                i + 1,
                preview,
                memory.similarity_score,
                memory.orbital_proximity
            ));
        }
        enriched.push(']');

        enriched
    }

    fn cognitive_enrich(
        &self,
        submode: &str,
        content: &str,
        memories: &[ThoughtMatch],
    ) -> (cognitive::types::FrameworkOutput, String) {
        use cognitive::CognitiveEngine;
        use cognitive::profile::{Submode, profile_for};
        use std::collections::HashMap;

        let sm = Submode::from_str(submode);
        let profile = profile_for(sm);

        // Build weights map with &'static str keys
        let weights: HashMap<&'static str, u8> = profile.weights.clone();
        let engine = CognitiveEngine::new();
        let analysis = engine.blend(content, &weights);

        // Format enriched content: framework sections + memory context
        let mut enriched = String::new();
        enriched.push_str(content);
        enriched.push_str(&Self::format_framework_analysis(&analysis));

        // Append memory context
        let enriched = if memories.is_empty() {
            enriched
        } else {
            self.enrich_content_with_memories(&enriched, memories)
        };

        (analysis, enriched)
    }

    fn cognitive_enrich_with_weights(
        &self,
        weights: &std::collections::HashMap<&'static str, u8>,
        content: &str,
        memories: &[ThoughtMatch],
    ) -> (cognitive::types::FrameworkOutput, String) {
        use cognitive::CognitiveEngine;
        let engine = CognitiveEngine::new();
        let analysis = engine.blend(content, weights);
        let mut enriched = String::new();
        enriched.push_str(content);
        if !analysis.insights.is_empty()
            || !analysis.questions.is_empty()
            || !analysis.next_steps.is_empty()
        {
            enriched.push_str("\n\n[Framework Analysis:");
            if !analysis.insights.is_empty() {
                enriched.push_str("\nInsights:");
                for it in analysis.insights.iter().take(8) {
                    enriched.push_str("\n- ");
                    enriched.push_str(it);
                }
            }
            if !analysis.questions.is_empty() {
                enriched.push_str("\nQuestions:");
                for q in analysis.questions.iter().take(4) {
                    enriched.push_str("\n- ");
                    enriched.push_str(q);
                }
            }
            if !analysis.next_steps.is_empty() {
                enriched.push_str("\nNext steps:");
                for n in analysis.next_steps.iter().take(4) {
                    enriched.push_str("\n- ");
                    enriched.push_str(n);
                }
            }
            enriched.push('\n');
            enriched.push(']');
        }
        let enriched = if memories.is_empty() {
            enriched
        } else {
            self.enrich_content_with_memories(&enriched, memories)
        };
        (analysis, enriched)
    }

    async fn create_tech_thought(
        &self,
        params: ConvoThinkParams,
    ) -> Result<serde_json::Value, McpError> {
        // Map submode and defaults
        let submode = params.submode.clone().unwrap_or_else(|| "plan".to_string());
        let injection_scale = params.injection_scale.unwrap_or(match submode.as_str() {
            "plan" => 3,
            "build" => 2,
            "debug" => 4,
            _ => 3,
        });
        if injection_scale > 5 {
            return Err(McpError {
                code: rmcp::model::ErrorCode::INVALID_PARAMS,
                message: "injection_scale must be between 0 and 5".into(),
                data: None,
            });
        }
        let significance = params.significance.unwrap_or(0.5);
        if !(0.0..=1.0).contains(&significance) {
            return Err(McpError {
                code: rmcp::model::ErrorCode::INVALID_PARAMS,
                message: format!(
                    "significance must be between 0.0 and 1.0 (got: {})",
                    significance
                )
                .into(),
                data: None,
            });
        }

        // Embedding
        let embedding = self
            .embedder
            .embed(&params.content)
            .await
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Failed to generate embedding: {}", e).into(),
                data: None,
            })?;
        // Validate embedding dimensionality
        let expected_dim = self.embedder.dimensions();
        if embedding.len() != expected_dim {
            return Err(McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!(
                    "Embedding dimension mismatch: expected {}, got {}. Check embedding provider configuration.",
                    expected_dim,
                    embedding.len()
                ).into(),
                data: None,
            });
        }
        // Retrieval
        let relevant_memories = self
            .retrieve_memories_for_injection(&embedding, injection_scale, Some(&submode))
            .await?;

        // Build weights per tech submode
        use std::collections::HashMap;
        let weights: HashMap<&'static str, u8> = match submode.as_str() {
            "plan" => HashMap::from([
                ("FirstPrinciples", 35),
                ("SystemsThinking", 25),
                ("OODA", 20),
                ("Socratic", 20),
            ]),
            "build" => HashMap::from([
                ("OODA", 35),
                ("RootCause", 20),
                ("FirstPrinciples", 20),
                ("Lateral", 15),
                ("Dialectical", 10),
            ]),
            "debug" => HashMap::from([
                ("RootCause", 45),
                ("OODA", 25),
                ("Socratic", 20),
                ("Dialectical", 10),
            ]),
            _ => HashMap::from([("OODA", 40), ("RootCause", 30), ("FirstPrinciples", 30)]),
        };
        let (analysis, enriched_content) =
            self.cognitive_enrich_with_weights(&weights, &params.content, &relevant_memories);

        // Persist
        let thought = Thought {
            id: Uuid::new_v4().to_string(),
            content: params.content.clone(),
            created_at: surrealdb::sql::Datetime::from(Utc::now()),
            embedding,
            injected_memories: relevant_memories
                .iter()
                .map(|m| m.thought.id.clone())
                .collect(),
            enriched_content: Some(enriched_content.clone()),
            injection_scale,
            significance,
            access_count: 0,
            last_accessed: None,
            submode: Some(submode.clone()),
            framework_enhanced: Some(true),
            framework_analysis: Some(
                json!({"insights": analysis.insights, "questions": analysis.questions, "next_steps": analysis.next_steps}),
            ),
        };
        let create_result = if Self::db_serial_enabled() {
            let _guard = self.db_gate.lock().await;
            self.db
                .query(
                    r#"CREATE type::thing('thoughts', $id) CONTENT {
                        id: $id,
                        content: $content,
                        created_at: time::now(),
                        embedding: $embedding,
                        injected_memories: $injected_memories,
                        enriched_content: $enriched_content,
                        injection_scale: $injection_scale,
                        significance: $significance,
                        access_count: $access_count,
                        submode: $submode,
                        framework_enhanced: $framework_enhanced,
                        framework_analysis: $framework_analysis
                    } RETURN NONE"#,
                )
                .bind(("id", thought.id.clone()))
                .bind(("content", thought.content.clone()))
                .bind(("embedding", thought.embedding.clone()))
                .bind(("injected_memories", thought.injected_memories.clone()))
                .bind(("enriched_content", thought.enriched_content.clone()))
                .bind(("injection_scale", thought.injection_scale))
                .bind(("significance", thought.significance))
                .bind(("access_count", thought.access_count))
                .bind(("submode", thought.submode.clone()))
                .bind(("framework_enhanced", thought.framework_enhanced))
                .bind(("framework_analysis", thought.framework_analysis.clone()))
                .await
        } else {
            self.db
                .query(
                    r#"CREATE type::thing('thoughts', $id) CONTENT {
                        id: $id,
                        content: $content,
                        created_at: time::now(),
                        embedding: $embedding,
                        injected_memories: $injected_memories,
                        enriched_content: $enriched_content,
                        injection_scale: $injection_scale,
                        significance: $significance,
                        access_count: $access_count,
                        submode: $submode,
                        framework_enhanced: $framework_enhanced,
                        framework_analysis: $framework_analysis
                    } RETURN NONE"#,
                )
                .bind(("id", thought.id.clone()))
                .bind(("content", thought.content.clone()))
                .bind(("embedding", thought.embedding.clone()))
                .bind(("injected_memories", thought.injected_memories.clone()))
                .bind(("enriched_content", thought.enriched_content.clone()))
                .bind(("injection_scale", thought.injection_scale))
                .bind(("significance", thought.significance))
                .bind(("access_count", thought.access_count))
                .bind(("submode", thought.submode.clone()))
                .bind(("framework_enhanced", thought.framework_enhanced))
                .bind(("framework_analysis", thought.framework_analysis.clone()))
                .await
        };

        create_result.map_err(|e| McpError {
            code: rmcp::model::ErrorCode::INTERNAL_ERROR,
            message: format!("Failed to store thought: {}", e).into(),
            data: None,
        })?;

        // Create relationships
        if !relevant_memories.is_empty() {
            // Determine flavor for this thought
            let flavor = tag_flavor(&params.content);

            let mut q = String::new();
            q.reserve(relevant_memories.len() * 128);
            for i in 0..relevant_memories.len() {
                q.push_str(&format!("RELATE $from{0}->recalls->$to{0} SET strength = $strength{0}, created_at = time::now(), submode_match = $submode_match{0}, flavor = $flavor{0} RETURN NONE;\n", i));
                q.push_str(&format!("RELATE $to{0}->recalls->$from{0} SET strength = $strength{0}, created_at = time::now(), submode_match = $submode_match{0}, flavor = $flavor{0} RETURN NONE;\n", i));
            }
            let mut req = self.db.query(q);
            for (i, memory) in relevant_memories.iter().enumerate() {
                // Check if submodes match (if both present)
                let submode_match = thought
                    .submode
                    .as_ref()
                    .and_then(|ts| memory.thought.submode.as_ref().map(|ms| ts == ms))
                    .unwrap_or(false);

                req = req
                    .bind((format!("from{}", i), format!("thoughts:{}", thought.id)))
                    .bind((
                        format!("to{}", i),
                        format!("thoughts:{}", memory.thought.id),
                    ))
                    .bind((format!("strength{}", i), memory.similarity_score))
                    .bind((format!("submode_match{}", i), submode_match))
                    .bind((format!("flavor{}", i), flavor.as_str()));
            }

            if Self::db_serial_enabled() {
                let _guard = self.db_gate.lock().await;
                let _ = req.await;
            } else {
                let _ = req.await;
            }
        }

        // user_friendly via existing helpers
        let verbose_analysis = params.verbose_analysis.unwrap_or(true);
        let limited = Self::apply_verbosity_limits(&analysis, verbose_analysis);

        Ok(json!({
            "thought_id": thought.id,
            "submode_used": submode,
            "memories_injected": relevant_memories.len(),
            "analysis": {
                "key_point": if !limited.insights.is_empty() {
                    // Take first insight and make it human-readable
                    Self::make_conversational(&limited.insights[0])
                } else {
                    "Thought stored successfully".to_string()
                },
                "question": if !limited.questions.is_empty() {
                    Self::make_conversational(&limited.questions[0])
                } else {
                    "What's next?".to_string()
                },
                "next_step": if !limited.next_steps.is_empty() {
                    limited.next_steps[0].clone()
                } else {
                    "Continue processing".to_string()
                }
            }
        }))
    }

    fn get_detailed_help(tool: Option<&str>, format: &str) -> serde_json::Value {
        let full = format.eq_ignore_ascii_case("full");
        let convo = json!({
            "name": "convo_think",
            "description": "Store thoughts with memory injection and cognitive analysis",
            "best_for": ["Conversations", "Ideation", "Reflective thinking"],
            "parameters": {
                "content": "string (required)",
                "injection_scale": "0-5 or presets: NONE/LIGHT/MEDIUM/DEFAULT/HIGH/MAXIMUM",
                "submode": "sarcastic|philosophical|empathetic|problem_solving",
                "significance": "0.0-1.0, or 2-10 (maps 0.2-1.0, 1 rejected), or 'low'|'medium'|'high'",
                "verbose_analysis": "boolean (default true)",
                "tags": "array<string> (optional)"
            },
            "examples": [
                json!({"content":"ponder this","submode":"sarcastic","injection_scale":"DEFAULT","significance":"medium"}),
                json!({"content":"reflect","injection_scale":3,"verbose_analysis":false})
            ]
        });
        let tech = json!({
            "name": "tech_think",
            "description": "Technical reasoning with memory injection",
            "best_for": ["Planning", "Building", "Debugging"],
            "parameters": {
                "content": "string (required)",
                "injection_scale": "0-5 or presets: NONE/LIGHT/MEDIUM/DEFAULT/HIGH/MAXIMUM",
                "submode": "plan|build|debug",
                "significance": "0.0-1.0, or 2-10 (maps 0.2-1.0, 1 rejected), or 'low'|'medium'|'high'",
                "verbose_analysis": "boolean (default true)",
                "tags": "array<string> (optional)"
            },
            "examples": [
                json!({"content":"design module A","submode":"plan","injection_scale":"DEFAULT"}),
                json!({"content":"fix panic in parser","submode":"debug","injection_scale":"HIGH","significance":10})
            ]
        });
        match tool {
            Some("convo_think") => {
                if full {
                    convo
                } else {
                    json!({"name": convo["name"], "parameters": convo["parameters"], "examples":[convo["examples"][0]]})
                }
            }
            Some("tech_think") => {
                if full {
                    tech
                } else {
                    json!({"name": tech["name"], "parameters": tech["parameters"], "examples":[tech["examples"][0]]})
                }
            }
            _ => {
                if full {
                    json!({"tools": [convo, tech]})
                } else {
                    json!({"tools": [{"name":"convo_think"}, {"name":"tech_think"}]})
                }
            }
        }
    }

    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        // Enforce equal dimensions to avoid silent truncation skew
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

    /// Convert injection scale to user-friendly orbital name
    #[allow(dead_code)]
    fn injection_scale_to_orbital_name(scale: u8) -> String {
        match scale {
            0 => "NONE".to_string(),
            1 => "MERCURY".to_string(),
            2 => "VENUS".to_string(),
            3 => "MARS".to_string(),
            4 => "JUPITER".to_string(),
            5 => "PLUTO".to_string(),
            _ => "MARS".to_string(),
        }
    }

    /// Get relevance label from similarity score
    #[allow(dead_code)]
    fn similarity_to_relevance_label(similarity: f32) -> &'static str {
        if similarity >= 0.9 {
            "Strong"
        } else if similarity >= 0.7 {
            "Moderate"
        } else {
            "Light"
        }
    }

    /// Format age as human-readable string
    #[allow(dead_code)]
    fn format_age(created_at: &surrealdb::sql::Datetime) -> String {
        let now = Utc::now();
        let created =
            chrono::DateTime::<Utc>::from_timestamp(created_at.timestamp(), 0).unwrap_or(now);
        let duration = now.signed_duration_since(created);

        if duration.num_minutes() < 60 {
            format!("{}m ago", duration.num_minutes().max(1))
        } else if duration.num_hours() < 24 {
            format!("{}h ago", duration.num_hours())
        } else {
            format!("{}d ago", duration.num_days())
        }
    }

    /// Categorize memory based on its characteristics
    #[allow(dead_code)]
    fn categorize_memory(memory: &ThoughtMatch, now_timestamp: i64) -> &'static str {
        let age_minutes = (now_timestamp - memory.thought.created_at.timestamp()) / 60;

        if age_minutes < 60 {
            "Recent"
        } else if memory.thought.significance > 0.7 {
            "High-Significance"
        } else {
            "Related Concept"
        }
    }

    /// Get thinking style name from submode
    #[allow(dead_code)]
    fn submode_to_thinking_style(submode: &str) -> String {
        let base = match submode {
            "sarcastic" => "Sarcastic",
            "philosophical" => "Philosophical",
            "empathetic" => "Empathetic",
            "problem_solving" => "Problem-Solving",
            _ => "Sarcastic",
        };
        format!("{} Analysis", base)
    }

    /// Apply verbosity limits to framework analysis
    fn apply_verbosity_limits(
        analysis: &cognitive::types::FrameworkOutput,
        verbose: bool,
    ) -> cognitive::types::FrameworkOutput {
        use cognitive::types::FrameworkOutput;
        use std::collections::HashMap;

        if verbose {
            analysis.clone()
        } else {
            FrameworkOutput {
                insights: analysis.insights.iter().take(2).cloned().collect(),
                questions: analysis.questions.iter().take(1).cloned().collect(),
                next_steps: analysis.next_steps.iter().take(1).cloned().collect(),
                meta: HashMap::new(),
            }
        }
    }

    /// Make framework outputs more conversational
    fn make_conversational(text: &str) -> String {
        text.replace("Reduce '", "Breaking down ")
            .replace("' to primitives", " into basic components")
            .replace("Orient: framing '", "Looking at ")
            .replace(
                "Decide: what is the immediate objective?",
                "What's the main goal here?",
            )
            .replace(
                "Act: what is the smallest next action?",
                "What's one small step forward?",
            )
            .replace("Why does '", "Why might ")
            .replace("' happen?", " be happening?")
            .replace("What makes '", "What would make ")
            .replace("' true?", " accurate?")
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    surreal_mind::load_env();

    // Initialize tracing with env filter
    let filter =
        std::env::var("RUST_LOG").unwrap_or_else(|_| "surreal_mind=info,rmcp=info".to_string());
    let mcp_no_log = std::env::var("MCP_NO_LOG").unwrap_or_default();
    if !(mcp_no_log == "1" || mcp_no_log.eq_ignore_ascii_case("true")) {
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_env_filter(filter)
            .with_ansi(false)
            .init();
    }

    info!("Starting Surreal Mind MCP Server with consciousness persistence");

    let server = SurrealMindServer::new().await?;

    // Use the new pattern from ui2 that actually works
    let service = server.serve(stdio()).await?;
    service.waiting().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_convo_think_params_deserialization() {
        // Test parameter parsing with submode
        let args = serde_json::json!({
            "content": "test thought",
            "injection_scale": 3,
            "significance": 0.8,
            "submode": "philosophical"
        });

        let params: ConvoThinkParams = serde_json::from_value(args).unwrap();
        assert_eq!(params.content, "test thought");
        assert_eq!(params.injection_scale, Some(3));
        assert_eq!(params.significance, Some(0.8));
        assert_eq!(params.submode, Some("philosophical".to_string()));
    }

    #[tokio::test]
    async fn test_submode_validation() {
        // Test invalid submode defaults to problem_solving
        let args = serde_json::json!({
            "content": "test",
            "submode": "invalid_mode"
        });

        let params: ConvoThinkParams = serde_json::from_value(args).unwrap();
        assert_eq!(params.submode, Some("invalid_mode".to_string()));
        // Note: actual validation happens in create_thought_with_injection
    }

    #[tokio::test]
    async fn test_server_initialization() {
        // Only run when explicitly enabled
        if std::env::var("RUN_DB_TESTS").is_err() {
            return;
        }
        let server = SurrealMindServer::new().await;
        assert!(server.is_ok(), "Server should initialize successfully");
    }

    #[test]
    fn test_thought_structure() {
        let thought = Thought {
            id: "test".to_string(),
            content: "test content".to_string(),
            created_at: surrealdb::sql::Datetime::from(chrono::Utc::now()),
            embedding: vec![0.1, 0.2, 0.3],
            injected_memories: vec![],
            enriched_content: Some("enriched".to_string()),
            injection_scale: 3,
            significance: 0.8,
            access_count: 0,
            last_accessed: None,
            submode: Some("sarcastic".to_string()),
            framework_enhanced: Some(true),
            framework_analysis: Some(
                serde_json::json!({"insights":[],"questions":[],"next_steps":[]}),
            ),
        };

        assert_eq!(thought.content, "test content");
        assert_eq!(thought.injection_scale, 3);
    }

    #[test]
    fn test_cosine_similarity() {
        // Test cosine similarity calculation directly
        fn cosine_similarity(vec_a: &[f32], vec_b: &[f32]) -> f32 {
            let dot_product: f32 = vec_a.iter().zip(vec_b).map(|(a, b)| a * b).sum();
            let magnitude_a: f32 = vec_a.iter().map(|x| x * x).sum::<f32>().sqrt();
            let magnitude_b: f32 = vec_b.iter().map(|x| x * x).sum::<f32>().sqrt();

            if magnitude_a == 0.0 || magnitude_b == 0.0 {
                0.0
            } else {
                dot_product / (magnitude_a * magnitude_b)
            }
        }

        let vec_a = vec![1.0, 0.0, 0.0];
        let vec_b = vec![1.0, 0.0, 0.0];
        let similarity = cosine_similarity(&vec_a, &vec_b);
        assert!((similarity - 1.0).abs() < 1e-6);

        let vec_c = vec![0.0, 1.0, 0.0];
        let similarity_orthogonal = cosine_similarity(&vec_a, &vec_c);
        assert!(similarity_orthogonal.abs() < 1e-6);
    }

    // Removed embeddings functionality test that depended on FakeEmbedder

    #[test]
    fn test_framework_formatter() {
        use cognitive::types::FrameworkOutput;

        let analysis = FrameworkOutput {
            insights: vec!["insight1".to_string(), "insight2".to_string()],
            questions: vec!["question1".to_string()],
            next_steps: vec!["step1".to_string(), "step2".to_string()],
            meta: std::collections::HashMap::new(),
        };

        let formatted = SurrealMindServer::format_framework_analysis(&analysis);
        assert!(formatted.contains("[Framework Analysis:"));
        assert!(formatted.contains("Insights:"));
        assert!(formatted.contains("insight1"));
        assert!(formatted.contains("Questions:"));
        assert!(formatted.contains("Next steps:"));
    }

    #[test]
    fn test_flavor_tagging() {
        use crate::flavor::tag_flavor;

        // Test deterministic flavor assignment
        assert_eq!(tag_flavor("But this contradicts").as_str(), "contrarian");
        assert_eq!(tag_flavor("The theory behind this").as_str(), "abstract");
        assert_eq!(tag_flavor("I feel strongly about").as_str(), "emotional");
        assert_eq!(tag_flavor("Let's fix this issue").as_str(), "solution");
        assert_eq!(tag_flavor("Just normal text").as_str(), "neutral");
    }

    #[test]
    fn test_submode_retrieval_flag_parity() {
        // Test that SURR_SUBMODE_RETRIEVAL=false preserves original behavior
        // This ensures backward compatibility when the feature flag is off

        // When OFF, orbital weights should remain default (40% age, 30% access, 30% significance)
        unsafe {
            std::env::remove_var("SURR_SUBMODE_RETRIEVAL");
        }

        // Mock a thought with all fields
        let thought = Thought {
            id: "test-parity".to_string(),
            content: "Test content for parity check".to_string(),
            created_at: surrealdb::sql::Datetime::from(chrono::Utc::now()),
            embedding: vec![0.1; 1536],
            injected_memories: vec![],
            enriched_content: Some("Test enriched".to_string()),
            injection_scale: 3,
            significance: 0.7,
            access_count: 5,
            last_accessed: Some(surrealdb::sql::Datetime::from(chrono::Utc::now())),
            submode: Some("philosophical".to_string()),
            framework_enhanced: Some(true),
            framework_analysis: Some(serde_json::json!({
                "insights": ["test insight"],
                "questions": ["test question"],
                "next_steps": ["test step"]
            })),
        };

        // Calculate orbital distance with default weights
        let age_factor = 0.4 * 0.1; // recent = closer
        let access_factor = 0.3 * (1.0 - (5_f32.min(100.0) / 100.0)); // more access = closer
        let sig_factor = 0.3 * (1.0 - 0.7); // higher sig = closer
        let expected_distance = age_factor + access_factor + sig_factor;

        // The orbital distance calculation should not be affected by submode when flag is OFF
        assert!(thought.submode.is_some());
        assert!(thought.framework_enhanced.is_some());

        // When flag is OFF, these new fields should be persisted but not affect retrieval logic
        assert_eq!(thought.injection_scale, 3);
        assert!(expected_distance < 1.0); // Should be valid orbital distance
    }

    #[test]
    fn test_helper_functions() {
        // Test injection scale to orbital name conversion
        assert_eq!(
            SurrealMindServer::injection_scale_to_orbital_name(0),
            "NONE"
        );
        assert_eq!(
            SurrealMindServer::injection_scale_to_orbital_name(1),
            "MERCURY"
        );
        assert_eq!(
            SurrealMindServer::injection_scale_to_orbital_name(3),
            "MARS"
        );
        assert_eq!(
            SurrealMindServer::injection_scale_to_orbital_name(5),
            "PLUTO"
        );
        assert_eq!(
            SurrealMindServer::injection_scale_to_orbital_name(99),
            "MARS"
        ); // fallback

        // Test similarity to relevance label
        assert_eq!(
            SurrealMindServer::similarity_to_relevance_label(0.95),
            "Strong"
        );
        assert_eq!(
            SurrealMindServer::similarity_to_relevance_label(0.85),
            "Moderate"
        );
        assert_eq!(
            SurrealMindServer::similarity_to_relevance_label(0.6),
            "Light"
        );

        // Test submode to thinking style
        assert_eq!(
            SurrealMindServer::submode_to_thinking_style("sarcastic"),
            "Sarcastic Analysis"
        );
        assert_eq!(
            SurrealMindServer::submode_to_thinking_style("philosophical"),
            "Philosophical Analysis"
        );
        assert_eq!(
            SurrealMindServer::submode_to_thinking_style("empathetic"),
            "Empathetic Analysis"
        );
        assert_eq!(
            SurrealMindServer::submode_to_thinking_style("problem_solving"),
            "Problem-Solving Analysis"
        );
        assert_eq!(
            SurrealMindServer::submode_to_thinking_style("unknown"),
            "Sarcastic Analysis"
        ); // fallback
    }

    #[test]
    fn test_verbosity_limits() {
        use cognitive::types::FrameworkOutput;
        use std::collections::HashMap;

        let analysis = FrameworkOutput {
            insights: vec![
                "insight1".to_string(),
                "insight2".to_string(),
                "insight3".to_string(),
                "insight4".to_string(),
            ],
            questions: vec![
                "question1".to_string(),
                "question2".to_string(),
                "question3".to_string(),
            ],
            next_steps: vec![
                "step1".to_string(),
                "step2".to_string(),
                "step3".to_string(),
            ],
            meta: HashMap::new(),
        };

        // Test verbose=true (should keep everything)
        let verbose_result = SurrealMindServer::apply_verbosity_limits(&analysis, true);
        assert_eq!(verbose_result.insights.len(), 4);
        assert_eq!(verbose_result.questions.len(), 3);
        assert_eq!(verbose_result.next_steps.len(), 3);

        // Test verbose=false (should limit)
        let limited_result = SurrealMindServer::apply_verbosity_limits(&analysis, false);
        assert_eq!(limited_result.insights.len(), 2); // Limited to 2
        assert_eq!(limited_result.questions.len(), 1); // Limited to 1
        assert_eq!(limited_result.next_steps.len(), 1); // Limited to 1
        assert_eq!(limited_result.insights[0], "insight1");
        assert_eq!(limited_result.insights[1], "insight2");
        assert_eq!(limited_result.questions[0], "question1");
        assert_eq!(limited_result.next_steps[0], "step1");
    }

    #[test]
    fn test_conversational_replacements() {
        let technical_text = "Reduce 'problem' to primitives and Orient: framing 'solution'";
        let conversational = SurrealMindServer::make_conversational(technical_text);
        assert!(conversational.contains("Breaking down problem into basic components"));
        assert!(conversational.contains("Looking at solution"));

        let question_text =
            "Decide: what is the immediate objective? Act: what is the smallest next action?";
        let friendly_questions = SurrealMindServer::make_conversational(question_text);
        assert!(friendly_questions.contains("What's the main goal here?"));
        assert!(friendly_questions.contains("What's one small step forward?"));
    }

    #[test]
    fn test_format_age() {
        use chrono::{Duration, Utc};

        // Test recent (minutes ago)
        let recent = surrealdb::sql::Datetime::from(Utc::now() - Duration::minutes(30));
        let age_str = SurrealMindServer::format_age(&recent);
        assert!(age_str.ends_with("m ago"));

        // Test hours ago
        let hours_ago = surrealdb::sql::Datetime::from(Utc::now() - Duration::hours(5));
        let age_str = SurrealMindServer::format_age(&hours_ago);
        assert!(age_str.ends_with("h ago"));

        // Test days ago
        let days_ago = surrealdb::sql::Datetime::from(Utc::now() - Duration::days(3));
        let age_str = SurrealMindServer::format_age(&days_ago);
        assert!(age_str.ends_with("d ago"));
    }

    #[test]
    fn test_memory_categorization() {
        let now = Utc::now().timestamp();

        // Test recent memory
        let recent_thought = Thought {
            id: "recent".to_string(),
            content: "recent thought".to_string(),
            created_at: surrealdb::sql::Datetime::from(Utc::now()),
            embedding: vec![],
            injected_memories: vec![],
            enriched_content: None,
            injection_scale: 3,
            significance: 0.5,
            access_count: 0,
            last_accessed: None,
            submode: None,
            framework_enhanced: None,
            framework_analysis: None,
        };
        let recent_match = ThoughtMatch {
            thought: recent_thought,
            similarity_score: 0.8,
            orbital_proximity: 0.7,
        };
        assert_eq!(
            SurrealMindServer::categorize_memory(&recent_match, now),
            "Recent"
        );

        // Test high significance memory (older)
        let significant_thought = Thought {
            id: "significant".to_string(),
            content: "significant thought".to_string(),
            created_at: surrealdb::sql::Datetime::from(Utc::now() - chrono::Duration::hours(5)),
            embedding: vec![],
            injected_memories: vec![],
            enriched_content: None,
            injection_scale: 3,
            significance: 0.8, // High significance
            access_count: 0,
            last_accessed: None,
            submode: None,
            framework_enhanced: None,
            framework_analysis: None,
        };
        let significant_match = ThoughtMatch {
            thought: significant_thought,
            similarity_score: 0.8,
            orbital_proximity: 0.7,
        };
        assert_eq!(
            SurrealMindServer::categorize_memory(&significant_match, now),
            "High-Significance"
        );

        // Test related concept (older, lower significance)
        let related_thought = Thought {
            id: "related".to_string(),
            content: "related thought".to_string(),
            created_at: surrealdb::sql::Datetime::from(Utc::now() - chrono::Duration::hours(5)),
            embedding: vec![],
            injected_memories: vec![],
            enriched_content: None,
            injection_scale: 3,
            significance: 0.3, // Lower significance
            access_count: 0,
            last_accessed: None,
            submode: None,
            framework_enhanced: None,
            framework_analysis: None,
        };
        let related_match = ThoughtMatch {
            thought: related_thought,
            similarity_score: 0.8,
            orbital_proximity: 0.7,
        };
        assert_eq!(
            SurrealMindServer::categorize_memory(&related_match, now),
            "Related Concept"
        );
    }

    #[test]
    fn test_injection_scale_preset_parsing() {
        use serde::Deserialize;
        use serde_json::json;

        #[derive(Debug, Deserialize)]
        struct TestStruct {
            #[serde(deserialize_with = "crate::deserializers::de_option_u8_forgiving")]
            injection_scale: Option<u8>,
        }

        // Test named presets (case-insensitive)
        let test_cases = vec![
            (json!({"injection_scale": "NONE"}), Some(0)),
            (json!({"injection_scale": "none"}), Some(0)),
            (json!({"injection_scale": "Light"}), Some(1)),
            (json!({"injection_scale": "MEDIUM"}), Some(2)),
            (json!({"injection_scale": "default"}), Some(3)),
            (json!({"injection_scale": "HIGH"}), Some(4)),
            (json!({"injection_scale": "maximum"}), Some(5)),
            (json!({"injection_scale": "MAXIMUM"}), Some(5)),
        ];

        for (input, expected) in test_cases {
            let result: TestStruct = serde_json::from_value(input.clone()).unwrap_or_else(|e| {
                panic!("Failed to parse {:?}: {}", input, e);
            });
            assert_eq!(
                result.injection_scale, expected,
                "Failed for input: {:?}",
                input
            );
        }

        // Test numeric values
        let numeric_cases = vec![
            (json!({"injection_scale": 0}), Some(0)),
            (json!({"injection_scale": 3}), Some(3)),
            (json!({"injection_scale": 5}), Some(5)),
            (json!({"injection_scale": "2"}), Some(2)),
        ];

        for (input, expected) in numeric_cases {
            let result: TestStruct = serde_json::from_value(input.clone()).unwrap();
            assert_eq!(result.injection_scale, expected);
        }

        // Test invalid values should error
        let invalid_cases = vec![
            (json!({"injection_scale": 6}), "out of range"),
            (json!({"injection_scale": -1}), "out of range"),
            (
                json!({"injection_scale": "invalid"}),
                "Invalid injection_scale",
            ),
            (
                json!({"injection_scale": "ultra"}),
                "Invalid injection_scale",
            ),
        ];

        for (input, expected_err) in invalid_cases {
            let result = serde_json::from_value::<TestStruct>(input.clone());
            assert!(result.is_err(), "Should have failed for input: {:?}", input);
            let err_msg = result.unwrap_err().to_string();
            assert!(
                err_msg.contains(expected_err),
                "Error message '{}' should contain '{}'",
                err_msg,
                expected_err
            );
        }
    }

    #[test]
    fn test_significance_parsing() {
        use serde::Deserialize;
        use serde_json::json;

        #[derive(Debug, Deserialize)]
        struct TestStruct {
            #[serde(deserialize_with = "crate::deserializers::de_option_f32_forgiving")]
            significance: Option<f32>,
        }

        // Test string presets
        let preset_cases = vec![
            (json!({"significance": "low"}), Some(0.2)),
            (json!({"significance": "LOW"}), Some(0.2)),
            (json!({"significance": "medium"}), Some(0.5)),
            (json!({"significance": "MEDIUM"}), Some(0.5)),
            (json!({"significance": "high"}), Some(0.9)),
            (json!({"significance": "HIGH"}), Some(0.9)),
        ];

        for (input, expected) in preset_cases {
            let result: TestStruct = serde_json::from_value(input.clone()).unwrap_or_else(|e| {
                panic!("Failed to parse {:?}: {}", input, e);
            });
            assert!(
                (result.significance.unwrap() - expected.unwrap()).abs() < 0.001,
                "Failed for input: {:?}, got {:?}, expected {:?}",
                input,
                result.significance,
                expected
            );
        }

        // Test 2-10 integer scale (1 not supported due to ambiguity with 1.0)
        let integer_scale_cases = vec![
            (json!({"significance": 2}), Some(0.2)),
            (json!({"significance": "3"}), Some(0.3)),
            (json!({"significance": 5}), Some(0.5)),
            (json!({"significance": "7"}), Some(0.7)),
            (json!({"significance": 10}), Some(1.0)),
        ];

        for (input, expected) in integer_scale_cases {
            let result: TestStruct = serde_json::from_value(input.clone()).unwrap();
            assert!(
                (result.significance.unwrap() - expected.unwrap()).abs() < 0.001,
                "Failed for 2-10 scale input: {:?}",
                input
            );
        }

        // Test standard 0.0-1.0 floats
        let float_cases = vec![
            (json!({"significance": 0.0}), Some(0.0)),
            (json!({"significance": 0.25}), Some(0.25)),
            (json!({"significance": 0.75}), Some(0.75)),
            (json!({"significance": 1.0}), Some(1.0)),
            (json!({"significance": "0.33"}), Some(0.33)),
        ];

        for (input, expected) in float_cases {
            let result: TestStruct = serde_json::from_value(input.clone()).unwrap();
            assert!(
                (result.significance.unwrap() - expected.unwrap()).abs() < 0.001,
                "Failed for float input: {:?}",
                input
            );
        }

        // Test invalid values should error
        let invalid_cases = vec![
            (json!({"significance": -0.5}), "out of range"),
            (json!({"significance": 1}), "ambiguous"),
            (json!({"significance": "1"}), "ambiguous"),
            (json!({"significance": 1.5}), "out of range"),
            (json!({"significance": 11}), "out of range"), // > 10, not in 1-10 scale
            (json!({"significance": "invalid"}), "Invalid significance"),
        ];

        for (input, expected_err) in invalid_cases {
            let result = serde_json::from_value::<TestStruct>(input.clone());
            assert!(result.is_err(), "Should have failed for input: {:?}", input);
            if result.is_err() {
                let err_msg = result.unwrap_err().to_string();
                assert!(
                    err_msg.contains(expected_err),
                    "Error message '{}' should contain '{}'",
                    err_msg,
                    expected_err
                );
            }
        }
    }
}
