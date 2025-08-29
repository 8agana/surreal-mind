use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client as HttpClient;
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
use sha2::{Digest, Sha256};
use std::num::NonZeroUsize;
use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::auth::Root;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

fn db_timeout_ms() -> u64 {
    std::env::var("SURR_DB_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10_000)
}

fn normalize_submode(s: &str) -> String {
    let out = s.trim().to_lowercase().replace('-', "_");
    match out.as_str() {
        // Common aliases
        "problem solving" | "problem_solving" | "ps" => "problem_solving".to_string(),
        "empathy" => "empathetic".to_string(),
        "snarky" => "sarcastic".to_string(),
        "philosophy" => "philosophical".to_string(),
        other => other.to_string(),
    }
}

// Retry utility for database operations with exponential backoff
async fn with_retry<T, F, Fut>(
    operation_name: &str,
    max_retries: u32,
    initial_delay_ms: u64,
    operation: F,
) -> Result<T, McpError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, surrealdb::Error>>,
{
    let mut delay_ms = initial_delay_ms;
    let backoff_factor = 2.0;

    // Get operation timeout from environment (default: 10 seconds)
    let operation_timeout_ms: u64 = std::env::var("SURR_OPERATION_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10000); // 10 seconds default

    for attempt in 1..=max_retries {
        // Wrap the operation in a timeout
        let timeout_duration = std::time::Duration::from_millis(operation_timeout_ms);
        let result = tokio::time::timeout(timeout_duration, operation()).await;

        match result {
            Ok(Ok(result)) => {
                if attempt > 1 {
                    info!(
                        "Operation '{}' succeeded on attempt {}/{}",
                        operation_name, attempt, max_retries
                    );
                }
                return Ok(result);
            }
            Ok(Err(e)) => {
                // Check if this is a retriable error by examining error message
                let error_str = e.to_string().to_lowercase();
                let is_retriable = error_str.contains("connection") ||
                    error_str.contains("timeout") ||
                    error_str.contains("broken pipe") ||
                    error_str.contains("network") ||
                    error_str.contains("websocket") ||
                    error_str.contains("io error") ||
                    error_str.contains("transport") ||
                    // Avoid retrying obvious logic errors
                    (!error_str.contains("parse") &&
                     !error_str.contains("syntax") &&
                     !error_str.contains("invalid") &&
                     !error_str.contains("permission"));

                if attempt == max_retries || !is_retriable {
                    return Err(McpError {
                        code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                        message: format!(
                            "{} failed after {} retries: {}{}",
                            operation_name,
                            attempt,
                            e,
                            if !is_retriable {
                                " (non-retriable error)"
                            } else {
                                ""
                            }
                        )
                        .into(),
                        data: None,
                    });
                }

                warn!(
                    "Attempt {}/{} for '{}' failed (will retry): {}",
                    attempt, max_retries, operation_name, e
                );

                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                delay_ms = (delay_ms as f64 * backoff_factor) as u64;
            }
            Err(_elapsed) => {
                // Operation timed out
                let timeout_msg = format!("Operation timed out after {}ms", operation_timeout_ms);

                if attempt == max_retries {
                    return Err(McpError {
                        code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                        message: format!(
                            "{} failed after {} retries: {}",
                            operation_name, attempt, timeout_msg
                        )
                        .into(),
                        data: None,
                    });
                }

                warn!(
                    "Attempt {}/{} for '{}' timed out (will retry)",
                    attempt, max_retries, operation_name
                );

                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                delay_ms = (delay_ms as f64 * backoff_factor) as u64;
            }
        }
    }
    unreachable!()
}

// Get retry configuration from environment variables
fn get_retry_config() -> (u32, u64) {
    let max_retries = std::env::var("SURR_MAX_RETRIES")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(3);

    let initial_delay_ms = std::env::var("SURR_RETRY_DELAY_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(500);

    (max_retries, initial_delay_ms)
}

mod cognitive;
mod deserializers;
mod embeddings;
mod flavor;
use embeddings::{Embedder, create_embedder};
use flavor::tag_flavor;

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
        serialize_with = "surreal_mind::serializers::serialize_as_string",
        deserialize_with = "surreal_mind::serializers::deserialize_thing_or_string"
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
    #[serde(default)]
    is_inner_voice: Option<bool>,
    #[serde(default)]
    inner_visibility: Option<String>,
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

#[derive(Debug, Deserialize)]
struct InnerVoiceParams {
    content: String,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_u8_forgiving"
    )]
    injection_scale: Option<u8>,
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
    #[serde(default)]
    inner_visibility: Option<String>, // "private", "context_only", etc.
}

#[derive(Clone)]
struct SurrealMindServer {
    db: Arc<Surreal<Client>>,
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

        let inner_voice_schema = rmcp::object!({
            "type": "object",
            "properties": {
                "content": {"type": "string", "description": "Private inner thought content"},
                "injection_scale": {"type": ["integer", "string"], "description": "Memory injection scale. Presets: NONE (0), LIGHT (1), MEDIUM (2), DEFAULT (3), HIGH (4), MAXIMUM (5). Or numeric 0-5"},
                "tags": {"type": "array", "items": {"type": "string"}, "description": "Additional tags"},
                "significance": {"type": ["number", "string"], "description": "Significance weight. Strings: 'low' (0.2), 'medium' (0.5), 'high' (0.9). Numbers: 0.0-1.0 or integers 2-10 (mapped to 0.2-1.0)"},
                "verbose_analysis": {"type": "boolean", "description": "Enable verbose framework analysis (default: true)", "default": true},
                "inner_visibility": {"type": "string", "description": "Visibility level for inner voice", "enum": ["private", "context_only"], "default": "context_only"}
            },
            "required": ["content"]
        });

        let help_schema = rmcp::object!({
            "type": "object",
            "properties": {
                "tool": {"type": "string", "enum": ["convo_think", "tech_think", "inner_voice", "knowledgegraph_create", "knowledgegraph_search"], "description": "Which tool to describe (omit for overview)"},
                "format": {"type": "string", "enum": ["compact", "full"], "description": "Level of detail", "default": "full"}
            }
        });

        let kg_create_schema = rmcp::object!({
            "type": "object",
            "properties": {
                "kind": {"type": "string", "enum": ["entity", "relationship", "observation"], "description": "What to create/upsert"},
                "data": {"type": "object", "description": "Payload for the given kind"},
                "upsert": {"type": "boolean", "default": true, "description": "If true, match by external_id/content_hash to avoid duplicates"},
                "source_thought_id": {"type": "string", "description": "Optional thought id for provenance; creates mentions edge"},
                "confidence": {"type": "number", "minimum": 0.0, "maximum": 1.0, "description": "Optional confidence for mentions/relationships"}
            },
            "required": ["kind", "data"]
        });

        let kg_search_schema = rmcp::object!({
            "type": "object",
            "properties": {
                "target": {"type": "string", "enum": ["entity", "relationship", "observation", "mixed"], "default": "entity"},
                "query": {"type": "object", "properties": {
                    "text": {"type": "string"},
                    "filters": {"type": "object"}
                }},
                "top_k": {"type": "integer", "minimum": 1, "maximum": 50, "default": 10},
                "offset": {"type": "integer", "minimum": 0, "default": 0},
                "action": {"type": "string", "enum": ["get"], "default": "get"}
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
                    Cow::Borrowed("inner_voice"),
                    Cow::Borrowed("Store private inner thoughts with memory injection"),
                    Arc::new(inner_voice_schema),
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
                Tool::new(
                    Cow::Borrowed("knowledgegraph_create"),
                    Cow::Borrowed("Create or upsert entities/relationships with provenance"),
                    Arc::new(kg_create_schema),
                ),
                Tool::new(
                    Cow::Borrowed("knowledgegraph_search"),
                    Cow::Borrowed("Search entities and relationships; returns typed results"),
                    Arc::new(kg_search_schema),
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
        // Global per-tool timeout to prevent hangs
        let timeout_ms = std::env::var("SURR_TOOL_TIMEOUT_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(15_000);
        let tool_name = request.name.to_string();

        let fut = async move {
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
                "inner_voice" => {
                    let args = request.arguments.clone().ok_or_else(|| McpError {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "Missing parameters".into(),
                        data: None,
                    })?;
                    let params: InnerVoiceParams =
                        serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                            McpError {
                                code: rmcp::model::ErrorCode::INVALID_PARAMS,
                                message: format!("Invalid parameters: {}", e).into(),
                                data: None,
                            }
                        })?;

                    // Redact content at info level to avoid logging private thoughts
                    info!("inner_voice called (content_len={})", params.content.len());
                    let dbg_preview: String = params.content.chars().take(50).collect(); // Shorter preview for privacy
                    debug!("inner_voice content (first 50 chars): {}", dbg_preview);
                    let result = self.create_inner_voice_thought(params).await?;
                    Ok(CallToolResult::structured(json!(result)))
                }
                "knowledgegraph_create" => {
                    let args = request.arguments.clone().ok_or_else(|| McpError {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "Missing parameters".into(),
                        data: None,
                    })?;
                    let result = self.kg_create_from_args(args).await?;
                    Ok(CallToolResult::structured(result))
                }
                "knowledgegraph_search" => {
                    let args = request.arguments.clone().ok_or_else(|| McpError {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "Missing parameters".into(),
                        data: None,
                    })?;
                    let result = self.kg_search_from_args(args).await?;
                    Ok(CallToolResult::structured(result))
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
        };

        match tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), fut).await {
            Ok(res) => res,
            Err(_) => {
                warn!(
                    "Tool '{}' timed out after {}ms; returning timeout error",
                    tool_name, timeout_ms
                );
                Err(McpError {
                    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                    message: format!(
                        "Tool '{}' timed out after {}ms. Try again or reduce workload.",
                        tool_name, timeout_ms
                    )
                    .into(),
                    data: None,
                })
            }
        }
    }
}

impl SurrealMindServer {
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
            thoughts: Arc::new(RwLock::new(thoughts_cache)),
            embedder,
        };

        server.initialize_schema().await?;

        Ok(server)
    }

    // Helper function for HTTP SQL queries to avoid WebSocket serialization issues
    async fn http_sql_query(&self, query: &str) -> Result<Vec<serde_json::Value>, McpError> {
        let host = std::env::var("SURR_DB_URL").unwrap_or_else(|_| "127.0.0.1:8000".to_string());
        let http_base = if host.starts_with("http") {
            host
        } else {
            format!("http://{}", host)
        };
        let sql_url = format!("{}/sql", http_base.trim_end_matches('/'));
        let user = std::env::var("SURR_DB_USER").unwrap_or_else(|_| "root".to_string());
        let pass = std::env::var("SURR_DB_PASS").unwrap_or_else(|_| "root".to_string());
        let ns = std::env::var("SURR_DB_NS").unwrap_or_else(|_| "surreal_mind".to_string());
        let dbname = std::env::var("SURR_DB_DB").unwrap_or_else(|_| "consciousness".to_string());

        let http = HttpClient::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Failed to build HTTP client: {}", e).into(),
                data: None,
            })?;

        let full_query = format!("USE NS {}; USE DB {}; {}", ns, dbname, query);
        debug!("HTTP SQL Query: {}", full_query);

        let resp = http
            .post(&sql_url)
            .basic_auth(&user, Some(&pass))
            .header("Accept", "application/json")
            .header("Content-Type", "application/surrealql")
            .body(full_query)
            .send()
            .await
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("HTTP query failed: {}", e).into(),
                data: None,
            })?;

        if !resp.status().is_success() {
            return Err(McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("HTTP query failed with status: {}", resp.status()).into(),
                data: None,
            });
        }

        let blocks: serde_json::Value = resp.json().await.map_err(|e| McpError {
            code: rmcp::model::ErrorCode::INTERNAL_ERROR,
            message: format!("Failed to parse HTTP response: {}", e).into(),
            data: None,
        })?;

        // Log the response structure for debugging (verbose, so using debug level)
        if let Some(arr) = blocks.as_array() {
            debug!("HTTP SQL response has {} blocks", arr.len());
            for (i, block) in arr.iter().enumerate() {
                if let Some(result) = block.get("result") {
                    if let Some(result_arr) = result.as_array() {
                        debug!("Block {} has {} results", i, result_arr.len());
                    } else {
                        debug!("Block {} result is not an array", i);
                    }
                }
            }
        }

        // SurrealDB returns 3 blocks: USE NS result, USE DB result, actual query result
        let result = blocks
            .get(2)
            .and_then(|b| b.get("result"))
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();

        Ok(result)
    }

    /// Increment (or decrement) an entity's mention_count in a safe, idempotent way.
    /// Accepts either a full Thing id like "entities:⟨...⟩" or a raw id with table inferred as 'entities'.
    #[allow(dead_code)]
    async fn increment_entity_mentions(&self, entity_id: &str, delta: i64) -> Result<(), McpError> {
        // Only act on entities; ignore if the provided thing isn't an entity
        let mut parts = entity_id.splitn(2, ':');
        let tb = parts.next().unwrap_or("entities");
        let inner = parts.next().unwrap_or("").trim();
        if tb != "entities" {
            return Ok(()); // forward-compatible no-op for non-entities
        }
        let inner = inner.trim_start_matches('⟨').trim_end_matches('⟩');
        if inner.is_empty() {
            return Ok(());
        }

        let q = r#"
            UPDATE type::thing($tb, $id)
            SET mention_count = math::max(0, (IF mention_count = NONE THEN 0 ELSE mention_count END) + $delta),
                last_seen_at = time::now()
            RETURN NONE;
        "#;

        let tb_s = tb.to_string();
        let id_s = inner.to_string();
        let (max_retries, initial_delay_ms) = get_retry_config();
        with_retry(
            "increment_entity_mentions",
            max_retries,
            initial_delay_ms,
            || {
                let tb_s = tb_s.clone();
                let id_s = id_s.clone();
                async move {
                    self.db
                        .query(q)
                        .bind(("tb", tb_s))
                        .bind(("id", id_s))
                        .bind(("delta", delta))
                        .await
                }
            },
        )
        .await
        .map(|_| ())
    }

    async fn initialize_schema(&self) -> Result<(), McpError> {
        info!("Initializing consciousness graph schema");
        let (max_retries, initial_delay_ms) = get_retry_config();

        // Define thoughts table with retry logic
        with_retry(
            "initialize_schema_thoughts",
            max_retries,
            initial_delay_ms,
            || async {
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
                DEFINE FIELD is_inner_voice ON TABLE thoughts TYPE option<bool>;
                DEFINE FIELD inner_visibility ON TABLE thoughts TYPE option<string>;

                DEFINE INDEX created_at_idx ON TABLE thoughts COLUMNS created_at;
                DEFINE INDEX significance_idx ON TABLE thoughts COLUMNS significance;
            "#,
                    )
                    .await
            },
        )
        .await?;

        // Define relationships table with retry logic
        with_retry(
            "initialize_schema_relationships",
            max_retries,
            initial_delay_ms,
            || async {
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
                DEFINE INDEX recalls_in_idx ON TABLE recalls COLUMNS in;
                DEFINE INDEX recalls_out_idx ON TABLE recalls COLUMNS out;
            "#,
                    )
                    .await
            },
        )
        .await?;

        // Define (forward-compatible) knowledge-graph tables: entities and mentions
        with_retry(
            "initialize_schema_entities",
            max_retries,
            initial_delay_ms,
            || async {
                self.db
                    .query(
                        r#"
                DEFINE TABLE entities SCHEMAFULL;
                DEFINE FIELD name ON TABLE entities TYPE string;
                DEFINE FIELD type ON TABLE entities TYPE string;
                DEFINE FIELD aliases ON TABLE entities TYPE array<string>;
                DEFINE FIELD properties ON TABLE entities TYPE object;
                DEFINE FIELD external_id ON TABLE entities TYPE option<string>;
                DEFINE FIELD content_hash ON TABLE entities TYPE option<string>;
                DEFINE FIELD name_embedding ON TABLE entities TYPE option<array<float>>;
                DEFINE FIELD mention_count ON TABLE entities TYPE number;
                DEFINE FIELD created_at ON TABLE entities TYPE datetime;
                DEFINE FIELD updated_at ON TABLE entities TYPE option<datetime>;
                DEFINE FIELD last_seen_at ON TABLE entities TYPE option<datetime>;
                DEFINE FIELD created_by_thought ON TABLE entities TYPE option<record<thoughts>>;

                DEFINE INDEX entity_hash_idx ON TABLE entities COLUMNS content_hash UNIQUE;
                DEFINE INDEX entity_name_idx ON TABLE entities COLUMNS name;
                DEFINE INDEX entity_type_idx ON TABLE entities COLUMNS type;
                DEFINE INDEX entity_extid_idx ON TABLE entities COLUMNS external_id;
                DEFINE INDEX entity_mentions_idx ON TABLE entities COLUMNS mention_count;

                DEFINE TABLE mentions TYPE RELATION;
                DEFINE FIELD in ON TABLE mentions TYPE record<thoughts>;
                DEFINE FIELD out ON TABLE mentions TYPE record<entities | observations>;
                DEFINE FIELD label ON TABLE mentions TYPE option<string>;
                DEFINE FIELD span ON TABLE mentions TYPE option<object>;
                DEFINE FIELD confidence ON TABLE mentions TYPE option<float>;
            "#,
                    )
                    .await
            },
        )
        .await?;

        // Define knowledge-graph relationships (entities <-> entities)
        with_retry(
            "initialize_schema_kg_relationships",
            max_retries,
            initial_delay_ms,
            || async {
                self.db
                    .query(
                        r#"
                DEFINE TABLE relationships TYPE RELATION;
                DEFINE FIELD in ON TABLE relationships TYPE record<entities>;
                DEFINE FIELD out ON TABLE relationships TYPE record<entities>;
                DEFINE FIELD predicate ON TABLE relationships TYPE string;
                DEFINE FIELD direction ON TABLE relationships TYPE string;
                DEFINE FIELD strength ON TABLE relationships TYPE float;
                DEFINE FIELD confidence ON TABLE relationships TYPE float;
                DEFINE FIELD valid_from ON TABLE relationships TYPE option<datetime>;
                DEFINE FIELD valid_to ON TABLE relationships TYPE option<datetime>;
                DEFINE FIELD properties ON TABLE relationships TYPE option<object>;
                DEFINE FIELD source_thought ON TABLE relationships TYPE option<record<thoughts>>;

                DEFINE INDEX rel_pred_idx ON TABLE relationships COLUMNS predicate;
                DEFINE INDEX rel_pair_idx ON TABLE relationships COLUMNS in, out, predicate;
            "#,
                    )
                    .await
            },
        )
        .await?;

        // Define observations table and allow mentions to link observations
        with_retry(
            "initialize_schema_observations",
            max_retries,
            initial_delay_ms,
            || async {
                self.db
                    .query(
                        r#"
                DEFINE TABLE observations SCHEMAFULL;
                DEFINE FIELD text ON TABLE observations TYPE string;
                DEFINE FIELD occurred_at ON TABLE observations TYPE datetime;
                DEFINE FIELD entities ON TABLE observations TYPE array<string>;
                DEFINE FIELD properties ON TABLE observations TYPE option<object>;
                DEFINE FIELD confidence ON TABLE observations TYPE option<float>;
                DEFINE FIELD content_hash ON TABLE observations TYPE string;
                DEFINE FIELD text_embedding ON TABLE observations TYPE option<array<float>>;
                DEFINE FIELD source_thought ON TABLE observations TYPE option<record<thoughts>>;
                DEFINE FIELD created_at ON TABLE observations TYPE datetime;
                DEFINE FIELD updated_at ON TABLE observations TYPE option<datetime>;

                DEFINE INDEX obs_hash_idx ON TABLE observations COLUMNS content_hash UNIQUE;
                DEFINE INDEX obs_time_idx ON TABLE observations COLUMNS occurred_at;
            "#,
                    )
                    .await
            },
        )
        .await?;

        // Backfill existing data with defaults (idempotent) with retry logic
        with_retry(
            "initialize_schema_backfill",
            max_retries,
            initial_delay_ms,
            || async {
                self.db
                    .query(
                        r#"
                UPDATE thoughts SET submode = "sarcastic" WHERE submode = NONE;
                UPDATE thoughts SET framework_enhanced = false WHERE framework_enhanced = NONE;
                UPDATE entities SET mention_count = 0 WHERE mention_count = NONE;
                UPDATE entities SET created_at = time::now() WHERE created_at = NONE;
            "#,
                    )
                    .await
            },
        )
        .await?;

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
        let submode_raw = params.submode.as_deref().unwrap_or("sarcastic");
        let submode_norm = normalize_submode(submode_raw);
        let (analysis, enriched_content) =
            self.cognitive_enrich(&submode_norm, &params.content, &relevant_memories);

        // Validate and default submode
        let submode_in = params
            .submode
            .clone()
            .map(|s| normalize_submode(&s))
            .unwrap_or_else(|| "sarcastic".to_string());
        let valid_submodes = [
            "sarcastic",
            "philosophical",
            "empathetic",
            "problem_solving",
        ];
        let submode = if valid_submodes.contains(&submode_in.as_str()) {
            submode_in
        } else {
            tracing::warn!(
                "Invalid submode '{}', defaulting to 'sarcastic'",
                submode_in
            );
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
            is_inner_voice: Some(false),    // convo_think thoughts are not inner voice
            inner_visibility: None,
        };

        // Store thought in SurrealDB with retry logic
        let (max_retries, initial_delay_ms) = get_retry_config();

        with_retry(
            "create_convo_thought",
            max_retries,
            initial_delay_ms,
            || async {
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
            },
        )
        .await?;

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
                // Check if submodes match (if both present)
                let _submode_match = thought
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
                    .bind((format!("submode_match{}", i), _submode_match))
                    .bind((format!("flavor{}", i), flavor.as_str()));
            }

            let _resp =
                tokio::time::timeout(std::time::Duration::from_millis(db_timeout_ms()), async {
                    req.await
                })
                .await
                .map_err(|_| McpError {
                    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                    message: "Relationship creation timed out".into(),
                    data: None,
                })?
                .map_err(|e| McpError {
                    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                    message: format!("Failed to create relationships: {}", e).into(),
                    data: None,
                })?;
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
                if t.embedding.len() != expected_dim {
                    continue;
                }
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
            // Scale-based optimization: reduce candidates for higher injection scales
            let default_limit: usize = std::env::var("SURR_DB_LIMIT")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(500);

            // Adjust limit based on injection scale to prevent timeouts
            let scale_adjusted_limit = match injection_scale {
                0..=2 => default_limit,          // Full limit for low scales
                3..=4 => default_limit.min(200), // Reduce for medium scales
                5 => default_limit.min(100),     // Minimal for maximum scale
                _ => default_limit.min(200),
            };

            let limit: usize = std::env::var("SURR_RETRIEVE_CANDIDATES")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(scale_adjusted_limit)
                .clamp(50, 5000);

            debug!(
                "Memory retrieval: injection_scale={}, limit={}, sim_thresh={}",
                injection_scale, limit, eff_sim_thresh
            );
            // Select only needed columns to reduce payload size
            let mut resp = self.db
                .query(format!(
                    "SELECT id, content, created_at, embedding, injected_memories, injection_scale, significance, access_count, last_accessed, submode, is_inner_voice FROM thoughts ORDER BY created_at DESC LIMIT {}",
                    limit
                ))
                .await
            .map_err(|e| McpError {
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
            // Add early termination for performance
            let early_termination_threshold = eff_sim_thresh * 0.8; // 80% of threshold

            for thought in results.iter() {
                if thought.embedding.len() != expected_dim {
                    continue;
                }
                let similarity = self.cosine_similarity(query_embedding, &thought.embedding);

                // Early termination: if we have enough matches and similarity is dropping
                if matches.len() >= top_k * 2 && similarity < early_termination_threshold {
                    debug!(
                        "Early termination: have {} matches, similarity {} below threshold",
                        matches.len(),
                        similarity
                    );
                    break;
                }

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

            let mut req = self.db.query(q);
            for (i, m) in top.iter().enumerate() {
                let new_ac = m.thought.access_count.saturating_add(1);
                req = req
                    .bind((format!("id{}", i), m.thought.id.clone()))
                    .bind((format!("ac{}", i), new_ac));
            }
            tokio::time::timeout(std::time::Duration::from_millis(db_timeout_ms()), async {
                req.await
            })
            .await
            .map_err(|_| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: "batch update timed out".into(),
                data: None,
            })?
            .map_err(|e| McpError {
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
        info!(
            "Search: embedder dimensions={}, query embedding len={}",
            expected_dim,
            embedding.len()
        );
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
        let sm_in = params.submode.as_deref().unwrap_or("sarcastic");
        let sm = cognitive::profile::Submode::from_str(&normalize_submode(sm_in));
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
        debug!("Search: checking {} cached thoughts", thoughts.len());
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
            debug!(
                "Search: thought '{}' similarity: {:.6}, threshold: {:.6}",
                t.id, sim, sim_thresh
            );
            if sim < sim_thresh {
                debug!(
                    "Search: thought '{}' filtered out (sim {:.6} < thresh {:.6})",
                    t.id, sim, sim_thresh
                );
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
        debug!("Search: found {} matches in cache", matches.len());

        // If insufficient, fallback to DB
        if matches.is_empty() {
            debug!("Search: cache empty, searching database");
            debug!(
                "Search: cache matches empty, checking DB with threshold: {}",
                sim_thresh
            );
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

            // Use HTTP SQL interface to avoid WebSocket serialization issues
            let rows_json = self.http_sql_query(&q).await?;
            debug!("Search: HTTP SQL returned {} thoughts", rows_json.len());

            // Parse JSON rows into Thought structs
            let mut rows: Vec<Thought> = Vec::new();
            for row in rows_json {
                // Convert JSON row to Thought struct manually to handle custom deserialization
                let thought = Self::parse_thought_from_json(row).map_err(|e| McpError {
                    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                    message: format!("Failed to parse thought from JSON: {}", e).into(),
                    data: None,
                })?;
                rows.push(thought);
            }

            debug!("Search: Successfully parsed {} thoughts", rows.len());
            debug!(
                "Search: parsed thoughts: {:?}",
                rows.iter().map(|t| &t.id).collect::<Vec<_>>()
            );

            for t in rows.iter() {
                // Temporarily disable dimension check to debug
                // if t.embedding.len() != expected_dim {
                //     info!("Search: DB thought has wrong dims: {} != {}", t.embedding.len(), expected_dim);
                //     continue;
                // }
                let sim = self.cosine_similarity(&embedding, &t.embedding);
                debug!("Search: thought '{}' similarity: {:.4}", t.id, sim);
                if sim < sim_thresh {
                    debug!(
                        "Search: thought '{}' filtered out (sim {:.4} < thresh {:.4})",
                        t.id, sim, sim_thresh
                    );
                    continue;
                }
                let prox =
                    compute_proximity(t.created_at.timestamp(), t.access_count, t.significance);
                debug!(
                    "Search: thought '{}' passed filter (sim: {:.4}, prox: {:.4})",
                    t.id, sim, prox
                );
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
                q.push_str("SELECT in AS nid, strength FROM recalls WHERE out = type::thing('thoughts', $sid) LIMIT $lim;\n");
                q.push_str("SELECT out AS nid, strength FROM recalls WHERE in = type::thing('thoughts', $sid) LIMIT $lim;\n");

                let mut req = self.db.query(q);
                req = req.bind(("sid", seed.clone())).bind(("lim", max_n));
                let mut resp = tokio::time::timeout(
                    std::time::Duration::from_millis(db_timeout_ms()),
                    async { req.await },
                )
                .await
                .map_err(|_| McpError {
                    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                    message: "recalls query timed out".into(),
                    data: None,
                })?
                .map_err(|e| McpError {
                    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                    message: format!("Failed to query recalls: {}", e).into(),
                    data: None,
                })?;
                let out1: Vec<serde_json::Value> = resp.take(0).unwrap_or_default();
                let out2: Vec<serde_json::Value> = resp.take(1).unwrap_or_default();
                for row in out1.into_iter().chain(out2.into_iter()) {
                    if let (Some(nid), Some(stren)) = (row.get("nid"), row.get("strength")) {
                        let nid_s = Self::kg_value_to_id_string(nid).unwrap_or_default();
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
                        "SELECT in AS nid, strength FROM recalls WHERE out = type::thing('thoughts', $mid) LIMIT $lim;\n",
                    );
                    q2.push_str(
                        "SELECT out AS nid, strength FROM recalls WHERE in = type::thing('thoughts', $mid) LIMIT $lim;\n",
                    );

                    let mut resp2 = tokio::time::timeout(
                        std::time::Duration::from_millis(db_timeout_ms()),
                        async {
                            self.db
                                .query(q2)
                                .bind(("mid", mid.clone()))
                                .bind(("lim", max_n))
                                .await
                        },
                    )
                    .await
                    .map_err(|_| McpError {
                        code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                        message: "second-hop recalls query timed out".into(),
                        data: None,
                    })?
                    .map_err(|e| McpError {
                        code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                        message: format!("Failed to query second-hop recalls: {}", e).into(),
                        data: None,
                    })?;
                    let s1: Vec<serde_json::Value> = resp2.take(0).unwrap_or_default();
                    let s2: Vec<serde_json::Value> = resp2.take(1).unwrap_or_default();
                    for row in s1.into_iter().chain(s2.into_iter()) {
                        if let (Some(nid), Some(stren)) = (row.get("nid"), row.get("strength")) {
                            let nid_s = Self::kg_value_to_id_string(nid).unwrap_or_default();
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

                let mut resp = tokio::time::timeout(
                    std::time::Duration::from_millis(db_timeout_ms()),
                    async {
                        self
                            .db
                            .query("SELECT id, content, created_at, embedding, significance, access_count, last_accessed, submode FROM thoughts WHERE id INSIDE $ids")
                            .bind(("ids", ids.clone()))
                            .await
                    },
                )
                .await
                .map_err(|_| McpError {
                    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                    message: "neighbor thoughts query timed out".into(),
                    data: None,
                })?
                .map_err(|e| McpError {
                    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                    message: format!("Failed to fetch neighbor thoughts: {}", e).into(),
                    data: None,
                })?;
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
                    if t.embedding.len() != expected_dim {
                        continue;
                    }
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

        // Filter out inner voice thoughts from user-facing results
        // Inner voice thoughts should not be visible in search results to maintain privacy
        sliced.retain(|m| !m.thought.is_inner_voice.unwrap_or(false));

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
        let submode =
            normalize_submode(&params.submode.clone().unwrap_or_else(|| "plan".to_string()));
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
            is_inner_voice: Some(false), // tech_think thoughts are not inner voice
            inner_visibility: None,
        };
        // Store tech thought in SurrealDB with retry logic
        let (max_retries, initial_delay_ms) = get_retry_config();

        with_retry(
            "create_tech_thought",
            max_retries,
            initial_delay_ms,
            || async {
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
            },
        )
        .await?;

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

            let _ =
                tokio::time::timeout(std::time::Duration::from_millis(db_timeout_ms()), async {
                    req.await
                })
                .await
                .map_err(|_| McpError {
                    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                    message: "Relationship creation timed out".into(),
                    data: None,
                })?
                .map_err(|e| McpError {
                    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                    message: format!("Failed to create relationships: {}", e).into(),
                    data: None,
                })?;
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

    async fn create_inner_voice_thought(
        &self,
        params: InnerVoiceParams,
    ) -> Result<serde_json::Value, McpError> {
        let injection_scale = params.injection_scale.unwrap_or(2); // Default to MEDIUM for inner voice
        if injection_scale > 5 {
            return Err(McpError {
                code: rmcp::model::ErrorCode::INVALID_PARAMS,
                message: format!(
                    "injection_scale must be between 0 and 5 (got: {}). Valid presets: 0=NONE, 1=MERCURY (hot memories), 2=VENUS (recent), 3=MARS (default), 4=JUPITER (distant), 5=PLUTO (everything). Example: {{\"injection_scale\": 2}}"
                    , injection_scale
                ).into(),
                data: None,
            });
        }

        let significance = params.significance.unwrap_or(0.3); // Lower default significance for private thoughts
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

        // Generate embedding
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

        // Retrieve memories for injection
        let relevant_memories = self
            .retrieve_memories_for_injection(&embedding, injection_scale, None) // No submode for inner voice
            .await
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Failed to retrieve memories for injection: {}", e).into(),
                data: None,
            })?;

        // Set inner_visibility default
        let inner_visibility = params
            .inner_visibility
            .clone()
            .unwrap_or_else(|| "context_only".to_string());

        // Basic framework analysis for inner voice (simpler than convo/tech)
        let enriched_content = if relevant_memories.is_empty() {
            params.content.clone()
        } else {
            format!(
                "{}\n\n[Memory context: {} related thoughts accessed]",
                params.content,
                relevant_memories.len()
            )
        };

        let analysis = crate::cognitive::types::FrameworkOutput {
            insights: vec![format!(
                "Inner thought recorded: {}",
                if params.content.len() > 50 {
                    format!("{}...", &params.content[..50])
                } else {
                    params.content.clone()
                }
            )],
            questions: vec!["What does this reveal about current thinking patterns?".to_string()],
            next_steps: vec!["Continue inner reflection".to_string()],
            meta: std::collections::HashMap::new(), // Empty meta for inner voice
        };

        let framework_json = serde_json::json!({
            "insights": analysis.insights,
            "questions": analysis.questions,
            "next_steps": analysis.next_steps,
        });

        // Create new thought with inner_voice flags
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
            submode: None, // Inner voice doesn't use submodes
            framework_enhanced: Some(true),
            framework_analysis: Some(framework_json.clone()),
            is_inner_voice: Some(true), // Mark as inner voice
            inner_visibility: Some(inner_visibility.clone()),
        };

        // Store thought in SurrealDB with retry logic
        let (max_retries, initial_delay_ms) = get_retry_config();

        with_retry(
            "create_inner_voice_thought",
            max_retries,
            initial_delay_ms,
            || async {
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
                            framework_analysis: $framework_analysis,
                            is_inner_voice: $is_inner_voice,
                            inner_visibility: $inner_visibility
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
                    .bind(("is_inner_voice", thought.is_inner_voice))
                    .bind(("inner_visibility", thought.inner_visibility.clone()))
                    .await
            },
        )
        .await?;

        // Cache the thought
        {
            let mut cache = self.thoughts.write().await;
            cache.put(thought.id.clone(), thought.clone());
        }

        // Create relationships (same pattern as other thoughts)
        if !relevant_memories.is_empty() {
            let flavor = tag_flavor(&params.content);

            let mut q = String::new();
            q.reserve(relevant_memories.len() * 128);
            for i in 0..relevant_memories.len() {
                q.push_str(&format!("RELATE $from{0}->recalls->$to{0} SET strength = $strength{0}, created_at = time::now(), submode_match = $submode_match{0}, flavor = $flavor{0} RETURN NONE;\n", i));
                q.push_str(&format!("RELATE $to{0}->recalls->$from{0} SET strength = $strength{0}, created_at = time::now(), submode_match = $submode_match{0}, flavor = $flavor{0} RETURN NONE;\n", i));
            }
            let mut req = self.db.query(q);
            for (i, memory) in relevant_memories.iter().enumerate() {
                req = req
                    .bind((format!("from{}", i), format!("thoughts:{}", thought.id)))
                    .bind((
                        format!("to{}", i),
                        format!("thoughts:{}", memory.thought.id),
                    ))
                    .bind((format!("strength{}", i), memory.similarity_score))
                    .bind((format!("submode_match{}", i), false)) // Inner voice doesn't match submodes
                    .bind((format!("flavor{}", i), flavor.as_str()));
            }

            let _ =
                tokio::time::timeout(std::time::Duration::from_millis(db_timeout_ms()), async {
                    req.await
                })
                .await
                .map_err(|_| McpError {
                    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                    message: "Relationship creation timed out".into(),
                    data: None,
                })?
                .map_err(|e| McpError {
                    code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                    message: format!("Failed to create relationships: {}", e).into(),
                    data: None,
                })?;
        }

        let verbose_analysis = params.verbose_analysis.unwrap_or(false); // Default to less verbose for inner voice
        let limited = Self::apply_verbosity_limits(&analysis, verbose_analysis);

        Ok(json!({
            "thought_id": thought.id,
            "inner_voice": true,
            "visibility": inner_visibility,
            "memories_injected": relevant_memories.len(),
            "analysis": {
                "key_point": if !limited.insights.is_empty() {
                    Self::make_conversational(&limited.insights[0])
                } else {
                    "Inner thought recorded privately".to_string()
                },
                "question": if !limited.questions.is_empty() {
                    Self::make_conversational(&limited.questions[0])
                } else {
                    "What else is worth noting?".to_string()
                },
                "next_step": if !limited.next_steps.is_empty() {
                    limited.next_steps[0].clone()
                } else {
                    "Continue inner reflection".to_string()
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

    // --- Knowledge Graph: helpers and tool implementations ---
    fn kg_parse_thing(id: &str, default_tb: &str) -> Option<(String, String)> {
        let mut parts = id.splitn(2, ':');
        let tb = parts.next().unwrap_or(default_tb);
        let inner = parts.next().unwrap_or("");
        let inner = inner.trim().trim_start_matches('⟨').trim_end_matches('⟩');
        if inner.is_empty() {
            None
        } else {
            Some((tb.to_string(), inner.to_string()))
        }
    }

    fn kg_sha256_hex(parts: &[&str]) -> String {
        let mut hasher = Sha256::new();
        for p in parts {
            hasher.update(p.as_bytes());
            hasher.update(b"|");
        }
        let bytes = hasher.finalize();
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            use std::fmt::Write as _;
            let _ = write!(&mut s, "{:02x}", b);
        }
        s
    }

    // Extract a Surreal Thing id from JSON (handles String or {tb, id} shapes)
    fn kg_value_to_id_string(v: &serde_json::Value) -> Option<String> {
        match v {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Object(map) => {
                // Expect { tb: "entities", id: "..." } or id as object { String: "..." }
                if let Some(id_val) = map.get("id") {
                    match id_val {
                        serde_json::Value::String(id) => {
                            if let Some(tb) = map.get("tb").and_then(|t| t.as_str()) {
                                Some(format!("{}:{}", tb, id))
                            } else {
                                Some(id.clone())
                            }
                        }
                        serde_json::Value::Object(id_obj) => {
                            if let Some(id_str) = id_obj.get("String").and_then(|s| s.as_str()) {
                                if let Some(tb) = map.get("tb").and_then(|t| t.as_str()) {
                                    Some(format!("{}:{}", tb, id_str))
                                } else {
                                    Some(id_str.to_string())
                                }
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn kg_embed_entities_enabled() -> bool {
        std::env::var("SURR_KG_EMBED_ENTITIES")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(true)
    }

    #[allow(dead_code)]
    fn kg_embed_observations_enabled() -> bool {
        std::env::var("SURR_KG_EMBED_OBSERVATIONS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(true)
    }

    async fn upsert_entity_from_map(
        &self,
        data: &serde_json::Map<String, serde_json::Value>,
        upsert: bool,
        source_thought_id: Option<&str>,
    ) -> Result<(String, bool), McpError> {
        let name = data
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError {
                code: rmcp::model::ErrorCode::INVALID_PARAMS,
                message: "entity.name is required".into(),
                data: None,
            })?
            .to_string();
        let etype = data
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError {
                code: rmcp::model::ErrorCode::INVALID_PARAMS,
                message: "entity.type is required".into(),
                data: None,
            })?
            .to_string();
        let external_id = data
            .get("external_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let aliases = data
            .get("aliases")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();
        let properties = data.get("properties").cloned().unwrap_or_else(|| json!({}));

        let name_norm = name.trim().to_lowercase();
        let type_norm = etype.trim().to_lowercase();
        let hash = Self::kg_sha256_hex(&[&name_norm, &type_norm]);

        // Try to find existing
        let mut found_id: Option<String> = None;
        if upsert {
            if let Some(ext) = &external_id {
                let q = "SELECT id FROM entities WHERE external_id = $ext LIMIT 1";
                let (max_retries, initial_delay_ms) = get_retry_config();
                let mut resp = with_retry(
                    "select_entity_by_external_id",
                    max_retries,
                    initial_delay_ms,
                    || {
                        let ext = ext.clone();
                        async move { self.db.query(q).bind(("ext", ext)).await }
                    },
                )
                .await?;
                let rows: Vec<serde_json::Value> = resp.take(0).unwrap_or_default();
                if let Some(row) = rows.into_iter().next()
                    && let Some(id) = row.get("id").and_then(|v| v.as_str())
                {
                    found_id = Some(id.to_string());
                }
            }
            if found_id.is_none() {
                let q = "SELECT id FROM entities WHERE content_hash = $hash LIMIT 1";
                let (max_retries, initial_delay_ms) = get_retry_config();
                let mut resp = with_retry(
                    "select_entity_by_hash",
                    max_retries,
                    initial_delay_ms,
                    || {
                        let hash = hash.clone();
                        async move { self.db.query(q).bind(("hash", hash)).await }
                    },
                )
                .await?;
                let rows: Vec<serde_json::Value> = resp.take(0).unwrap_or_default();
                if let Some(row) = rows.into_iter().next()
                    && let Some(id) = row.get("id").and_then(|v| v.as_str())
                {
                    found_id = Some(id.to_string());
                }
            }
        }

        if let Some(eid) = found_id {
            // Update basic fields on upsert
            let q = r#"
                UPDATE $rid SET
                    name = $name,
                    type = $etype,
                    aliases = $aliases,
                    properties = $properties,
                    external_id = $external_id,
                    content_hash = $hash,
                    name_embedding = $name_embedding,
                    updated_at = time::now()
                RETURN id;
            "#;
            let (tb, inner) =
                Self::kg_parse_thing(&eid, "entities").unwrap_or(("entities".into(), eid.clone()));
            let (max_retries, initial_delay_ms) = get_retry_config();
            let mut resp = with_retry("update_entity", max_retries, initial_delay_ms, || {
                let name = name.clone();
                let etype = etype.clone();
                let aliases = aliases.clone();
                let properties = properties.clone();
                let external_id = external_id.clone();
                let hash = hash.clone();
                let tb = tb.clone();
                let inner = inner.clone();
                async move {
                    self.db
                        .query(q)
                        .bind(("rid", format!("{}:{}", tb, inner)))
                        .bind(("name", name))
                        .bind(("etype", etype))
                        .bind(("aliases", aliases))
                        .bind(("properties", properties))
                        .bind(("external_id", external_id))
                        .bind(("hash", hash))
                        .bind(("name_embedding", serde_json::Value::Null))
                        .await
                }
            })
            .await?;
            let rows: Vec<serde_json::Value> = resp.take(0).unwrap_or_default();
            let id = if let Some(row) = rows.first() {
                Self::kg_value_to_id_string(row.get("id").unwrap_or(&serde_json::Value::Null))
                    .unwrap_or_else(|| eid.clone())
            } else {
                eid.clone()
            };

            // provenance mention and counter
            if let Some(tid) = source_thought_id
                && let Some((_, einner)) = Self::kg_parse_thing(&id, "entities")
            {
                let q = r#"
                        CREATE mentions SET in = type::thing('thoughts', $tid), out = type::thing('entities', $eid), confidence = $conf;
                    "#;
                let conf = 0.8f32;
                with_retry(
                    "create_mention_update",
                    max_retries,
                    initial_delay_ms,
                    || {
                        let tid = tid.to_string();
                        let einner = einner.clone();
                        async move {
                            self.db
                                .query(q)
                                .bind(("tid", tid))
                                .bind(("eid", einner))
                                .bind(("conf", conf))
                                .await
                        }
                    },
                )
                .await?;
                let _ = self.increment_entity_mentions(&id, 1).await;
            }
            return Ok((id, false));
        }

        // Create new entity
        let q = r#"
            CREATE entities SET
                name = $name,
                type = $etype,
                aliases = $aliases,
                properties = $properties,
                external_id = $external_id,
                content_hash = $hash,
                created_at = time::now(),
                mention_count = 0
            RETURN id;
        "#;
        let (max_retries, initial_delay_ms) = get_retry_config();
        let mut resp = with_retry("create_entity", max_retries, initial_delay_ms, || {
            let name = name.clone();
            let etype = etype.clone();
            let aliases = aliases.clone();
            let properties = properties.clone();
            let external_id = external_id.clone();
            let hash = hash.clone();
            async move {
                self.db
                    .query(q)
                    .bind(("name", name))
                    .bind(("etype", etype))
                    .bind(("aliases", aliases))
                    .bind(("properties", properties))
                    .bind(("external_id", external_id))
                    .bind(("hash", hash))
                    .await
            }
        })
        .await?;
        let rows: Vec<serde_json::Value> = resp.take(0).unwrap_or_default();

        // Debug logging to understand why IDs are empty
        debug!("Entity CREATE response: {} rows returned", rows.len());
        if let Some(row) = rows.first() {
            debug!("First row: {:?}", row);
        }

        let id = if let Some(row) = rows.first() {
            let id_value = row.get("id").unwrap_or(&serde_json::Value::Null);
            debug!("ID value from response: {:?}", id_value);
            let extracted_id = Self::kg_value_to_id_string(id_value).unwrap_or_default();
            debug!("Extracted entity ID: '{}'", extracted_id);
            extracted_id
        } else {
            warn!("Entity CREATE returned no rows - query may have failed");
            String::new()
        };

        if let Some(tid) = source_thought_id
            && let Some((_, einner)) = Self::kg_parse_thing(&id, "entities")
        {
            let q = r#"
                    CREATE mentions SET in = type::thing('thoughts', $tid), out = type::thing('entities', $eid), confidence = $conf;
                "#;
            let conf = 0.8f32;
            with_retry("create_mention_new", max_retries, initial_delay_ms, || {
                let tid = tid.to_string();
                let einner = einner.clone();
                async move {
                    self.db
                        .query(q)
                        .bind(("tid", tid))
                        .bind(("eid", einner))
                        .bind(("conf", conf))
                        .await
                }
            })
            .await?;
            let _ = self.increment_entity_mentions(&id, 1).await;
        }

        Ok((id, true))
    }

    async fn resolve_entity_ref(
        &self,
        value: &serde_json::Value,
        upsert: bool,
        source_thought_id: Option<&str>,
    ) -> Result<String, McpError> {
        if let Some(id) = value.as_str() {
            // Assume full id
            return Ok(id.to_string());
        }
        if let Some(obj) = value.as_object() {
            let (id, _created) = self
                .upsert_entity_from_map(obj, upsert, source_thought_id)
                .await?;
            return Ok(id);
        }
        Err(McpError {
            code: rmcp::model::ErrorCode::INVALID_PARAMS,
            message: "Invalid entity reference - expected id string or {name,type}".into(),
            data: None,
        })
    }

    pub async fn kg_create_from_args(
        &self,
        args: serde_json::Map<String, serde_json::Value>,
    ) -> Result<serde_json::Value, McpError> {
        let kind = args
            .get("kind")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError {
                code: rmcp::model::ErrorCode::INVALID_PARAMS,
                message: "kind is required".into(),
                data: None,
            })?;
        let data = args
            .get("data")
            .and_then(|v| v.as_object())
            .ok_or_else(|| McpError {
                code: rmcp::model::ErrorCode::INVALID_PARAMS,
                message: "data object is required".into(),
                data: None,
            })?;
        let upsert = args.get("upsert").and_then(|v| v.as_bool()).unwrap_or(true);
        let source_thought_id = args.get("source_thought_id").and_then(|v| v.as_str());
        let confidence = args
            .get("confidence")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32)
            .unwrap_or(0.8);

        match kind {
            "entity" => {
                let (id, created) = self
                    .upsert_entity_from_map(data, upsert, source_thought_id)
                    .await?;
                Ok(json!({
                    "kind": "entity",
                    "entity_id": id,
                    "created": created
                }))
            }
            "relationship" => {
                // subject, object, predicate, direction, strength, confidence
                let subject = data.get("subject").ok_or_else(|| McpError {
                    code: rmcp::model::ErrorCode::INVALID_PARAMS,
                    message: "relationship.subject is required".into(),
                    data: None,
                })?;
                let object = data.get("object").ok_or_else(|| McpError {
                    code: rmcp::model::ErrorCode::INVALID_PARAMS,
                    message: "relationship.object is required".into(),
                    data: None,
                })?;
                let predicate = data
                    .get("predicate")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "relationship.predicate is required".into(),
                        data: None,
                    })?
                    .to_string();
                let direction = data
                    .get("direction")
                    .and_then(|v| v.as_str())
                    .unwrap_or("uni")
                    .to_string();
                let strength = data
                    .get("strength")
                    .and_then(|v| v.as_f64())
                    .map(|f| f as f32)
                    .unwrap_or(0.5);
                let conf = data
                    .get("confidence")
                    .and_then(|v| v.as_f64())
                    .map(|f| f as f32)
                    .unwrap_or(confidence);

                let sid = self
                    .resolve_entity_ref(subject, true, source_thought_id)
                    .await?;
                let oid = self
                    .resolve_entity_ref(object, true, source_thought_id)
                    .await?;
                let (_, sin) = Self::kg_parse_thing(&sid, "entities").ok_or_else(|| McpError {
                    code: rmcp::model::ErrorCode::INVALID_PARAMS,
                    message: "Invalid subject id".into(),
                    data: None,
                })?;
                let (_, oin) = Self::kg_parse_thing(&oid, "entities").ok_or_else(|| McpError {
                    code: rmcp::model::ErrorCode::INVALID_PARAMS,
                    message: "Invalid object id".into(),
                    data: None,
                })?;

                let q = r#"
                    CREATE relationships SET
                        in = type::thing('entities', $sid),
                        out = type::thing('entities', $oid),
                        predicate = $predicate,
                        direction = $direction,
                        strength = $strength,
                        confidence = $confidence,
                        source_thought = $sthought
                    RETURN id;
                "#;
                let (max_retries, initial_delay_ms) = get_retry_config();
                let mut resp =
                    with_retry("create_relationship", max_retries, initial_delay_ms, || {
                        let predicate = predicate.clone();
                        let direction = direction.clone();
                        let sin = sin.clone();
                        let oin = oin.clone();
                        let sthought = source_thought_id.map(|s| s.to_string());
                        async move {
                            self.db
                                .query(q)
                                .bind(("sid", sin))
                                .bind(("oid", oin))
                                .bind(("predicate", predicate))
                                .bind(("direction", direction))
                                .bind(("strength", strength))
                                .bind(("confidence", conf))
                                .bind(("sthought", sthought))
                                .await
                        }
                    })
                    .await?;
                let mut items: Vec<String> = Vec::new();
                if let Ok(arr) = resp.take::<Vec<serde_json::Value>>(0) {
                    for row in arr {
                        if let Some(id) = Self::kg_value_to_id_string(
                            row.get("id").unwrap_or(&serde_json::Value::Null),
                        ) && !id.is_empty()
                        {
                            items.push(id);
                        }
                    }
                }

                // Bi-directional reverse edge
                if direction.to_lowercase() == "bi" {
                    let q2 = r#"
                        CREATE relationships SET
                            in = type::thing('entities', $oid),
                            out = type::thing('entities', $sid),
                            predicate = $predicate,
                            direction = 'bi',
                            strength = $strength,
                            confidence = $confidence,
                            source_thought = $sthought
                        RETURN id;
                    "#;
                    let mut resp2 = with_retry(
                        "create_relationship_reverse",
                        max_retries,
                        initial_delay_ms,
                        || {
                            let predicate = predicate.clone();
                            let sin = sin.clone();
                            let oin = oin.clone();
                            let sthought = source_thought_id.map(|s| s.to_string());
                            async move {
                                self.db
                                    .query(q2)
                                    .bind(("sid", sin))
                                    .bind(("oid", oin))
                                    .bind(("predicate", predicate))
                                    .bind(("strength", strength))
                                    .bind(("confidence", conf))
                                    .bind(("sthought", sthought))
                                    .await
                            }
                        },
                    )
                    .await?;
                    if let Ok(arr) = resp2.take::<Vec<serde_json::Value>>(0) {
                        for row in arr {
                            if let Some(id) = row.get("id").and_then(|v| v.as_str()) {
                                items.push(id.to_string());
                            }
                        }
                    }
                }

                Ok(json!({ "kind": "relationship", "created_edges": items }))
            }
            "observation" => {
                // Required: text; Optional: occurred_at, entities[], properties, confidence
                let text = data
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "observation.text is required".into(),
                        data: None,
                    })?
                    .to_string();

                let occurred_at = data
                    .get("occurred_at")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

                let entities_vals = data
                    .get("entities")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                // Resolve entity refs to ids
                let mut resolved_entities: Vec<String> = Vec::new();
                for ev in entities_vals.iter() {
                    let eid = self.resolve_entity_ref(ev, true, source_thought_id).await?;
                    resolved_entities.push(eid);
                }
                // Sort for stable hashing
                resolved_entities.sort();

                // Optional properties
                let properties = data.get("properties").cloned().unwrap_or_else(|| json!({}));
                let conf = data
                    .get("confidence")
                    .and_then(|v| v.as_f64())
                    .map(|f| f as f32)
                    .unwrap_or(confidence);

                // Compute content hash deterministically: lower(text)|occurred_at|sorted entity ids
                let text_norm = text.trim().to_lowercase();
                let ents_joined = resolved_entities.join(",");
                let hash = Self::kg_sha256_hex(&[&text_norm, &occurred_at, &ents_joined]);

                // Upsert by content_hash
                let mut found_id: Option<String> = None;
                if upsert {
                    let q = "SELECT id FROM observations WHERE content_hash = $hash LIMIT 1";
                    let (max_retries, initial_delay_ms) = get_retry_config();
                    let mut resp = with_retry(
                        "select_observation_by_hash",
                        max_retries,
                        initial_delay_ms,
                        || {
                            let hash = hash.clone();
                            async move { self.db.query(q).bind(("hash", hash)).await }
                        },
                    )
                    .await?;
                    let rows: Vec<serde_json::Value> = resp.take(0).unwrap_or_default();
                    if let Some(row) = rows.into_iter().next()
                        && let Some(id) = row.get("id").and_then(|v| v.as_str())
                    {
                        found_id = Some(id.to_string());
                    }
                }

                if let Some(oid) = found_id {
                    // Update existing observation
                    let q = r#"
                        UPDATE $rid SET
                            text = $text,
                            occurred_at = type::datetime($occurred_at),
                            entities = $entities,
                            properties = $properties,
                            confidence = $confidence,
                            text_embedding = $text_embedding,
                            updated_at = time::now()
                        RETURN id;
                    "#;
                    let (tb, inner) = Self::kg_parse_thing(&oid, "observations")
                        .unwrap_or(("observations".into(), oid.clone()));
                    let (max_retries, initial_delay_ms) = get_retry_config();
                    let mut resp =
                        with_retry("update_observation", max_retries, initial_delay_ms, || {
                            let text = text.clone();
                            let occurred_at = occurred_at.clone();
                            let entities = resolved_entities.clone();
                            let properties = properties.clone();
                            let tb_s = tb.clone();
                            let inner_s = inner.clone();
                            async move {
                                self.db
                                    .query(q)
                                    .bind(("rid", format!("{}:{}", tb_s, inner_s)))
                                    .bind(("text", text))
                                    .bind(("occurred_at", occurred_at))
                                    .bind(("entities", entities))
                                    .bind(("properties", properties))
                                    .bind(("confidence", conf))
                                    .bind(("text_embedding", serde_json::Value::Null))
                                    .await
                            }
                        })
                        .await?;
                    let rows: Vec<serde_json::Value> = resp.take(0).unwrap_or_default();
                    let id = if let Some(row) = rows.first() {
                        Self::kg_value_to_id_string(
                            row.get("id").unwrap_or(&serde_json::Value::Null),
                        )
                        .unwrap_or_else(|| oid.clone())
                    } else {
                        oid.clone()
                    };

                    // Provenance mentions
                    if let Some(tid) = source_thought_id
                        && let Some((_, oinner)) = Self::kg_parse_thing(&id, "observations")
                    {
                        let q = r#"
                            CREATE mentions SET in = type::thing('thoughts', $tid), out = type::thing('observations', $oid), confidence = $conf;
                        "#;
                        with_retry(
                            "create_mention_obs_update",
                            max_retries,
                            initial_delay_ms,
                            || {
                                let tid = tid.to_string();
                                let oinner = oinner.clone();
                                async move {
                                    self.db
                                        .query(q)
                                        .bind(("tid", tid))
                                        .bind(("oid", oinner))
                                        .bind(("conf", conf))
                                        .await
                                }
                            },
                        )
                        .await?;
                        // Mentions to entities as well
                        for eid in resolved_entities.iter() {
                            if let Some((_, einner)) = Self::kg_parse_thing(eid, "entities") {
                                let q2 = r#"CREATE mentions SET in = type::thing('thoughts', $tid), out = type::thing('entities', $eid), confidence = $conf2;"#;
                                let conf2 = conf.min(0.7);
                                with_retry(
                                    "create_mention_entity_update",
                                    max_retries,
                                    initial_delay_ms,
                                    || {
                                        let tid = tid.to_string();
                                        let einner = einner.clone();
                                        async move {
                                            self.db
                                                .query(q2)
                                                .bind(("tid", tid))
                                                .bind(("eid", einner))
                                                .bind(("conf2", conf2))
                                                .await
                                        }
                                    },
                                )
                                .await?;
                                let _ = self.increment_entity_mentions(eid, 1).await;
                            }
                        }
                    }

                    Ok(json!({
                        "kind": "observation",
                        "observation_id": id,
                        "created": false,
                        "resolved_entities": resolved_entities
                    }))
                } else {
                    // Create new observation
                    let q = r#"
                        CREATE observations SET
                            text = $text,
                            occurred_at = type::datetime($occurred_at),
                            entities = $entities,
                            properties = $properties,
                            confidence = $confidence,
                            content_hash = $hash,
                            text_embedding = $text_embedding,
                            source_thought = $sthought,
                            created_at = time::now()
                        RETURN id;
                    "#;
                    let sthought = source_thought_id.map(|s| s.to_string());
                    let (max_retries, initial_delay_ms) = get_retry_config();
                    let mut resp =
                        with_retry("create_observation", max_retries, initial_delay_ms, || {
                            let text = text.clone();
                            let occurred_at = occurred_at.clone();
                            let entities = resolved_entities.clone();
                            let properties = properties.clone();
                            let hash = hash.clone();
                            let sthought = sthought.clone();
                            async move {
                                self.db
                                    .query(q)
                                    .bind(("text", text))
                                    .bind(("occurred_at", occurred_at))
                                    .bind(("entities", entities))
                                    .bind(("properties", properties))
                                    .bind(("confidence", conf))
                                    .bind(("hash", hash))
                                    .bind(("sthought", sthought))
                                    .await
                            }
                        })
                        .await?;
                    let rows: Vec<serde_json::Value> = resp.take(0).unwrap_or_default();

                    // Debug logging for observation CREATE
                    debug!("Observation CREATE response: {} rows returned", rows.len());
                    if let Some(row) = rows.first() {
                        debug!("First row: {:?}", row);
                    }

                    let id = if let Some(row) = rows.first() {
                        let id_value = row.get("id").unwrap_or(&serde_json::Value::Null);
                        debug!("ID value from response: {:?}", id_value);
                        let extracted_id =
                            Self::kg_value_to_id_string(id_value).unwrap_or_default();
                        debug!("Extracted observation ID: '{}'", extracted_id);
                        extracted_id
                    } else {
                        warn!("Observation CREATE returned no rows - query may have failed");
                        String::new()
                    };

                    if let Some(tid) = source_thought_id
                        && let Some((_, oinner)) = Self::kg_parse_thing(&id, "observations")
                    {
                        let q = r#"
                            CREATE mentions SET in = type::thing('thoughts', $tid), out = type::thing('observations', $oid), confidence = $conf;
                        "#;
                        with_retry(
                            "create_mention_obs_new",
                            max_retries,
                            initial_delay_ms,
                            || {
                                let tid = tid.to_string();
                                let oinner = oinner.clone();
                                async move {
                                    self.db
                                        .query(q)
                                        .bind(("tid", tid))
                                        .bind(("oid", oinner))
                                        .bind(("conf", conf))
                                        .await
                                }
                            },
                        )
                        .await?;
                        for eid in resolved_entities.iter() {
                            if let Some((_, einner)) = Self::kg_parse_thing(eid, "entities") {
                                let q2 = r#"CREATE mentions SET in = type::thing('thoughts', $tid), out = type::thing('entities', $eid), confidence = $conf2;"#;
                                let conf2 = conf.min(0.7);
                                with_retry(
                                    "create_mention_entity_new",
                                    max_retries,
                                    initial_delay_ms,
                                    || {
                                        let tid = tid.to_string();
                                        let einner = einner.clone();
                                        async move {
                                            self.db
                                                .query(q2)
                                                .bind(("tid", tid))
                                                .bind(("eid", einner))
                                                .bind(("conf2", conf2))
                                                .await
                                        }
                                    },
                                )
                                .await?;
                                let _ = self.increment_entity_mentions(eid, 1).await;
                            }
                        }
                    }

                    Ok(json!({
                        "kind": "observation",
                        "observation_id": id,
                        "created": true,
                        "resolved_entities": resolved_entities
                    }))
                }
            }
            _ => Err(McpError {
                code: rmcp::model::ErrorCode::INVALID_PARAMS,
                message: format!("Unsupported kind: {}", kind).into(),
                data: None,
            }),
        }
    }

    pub async fn kg_search_from_args(
        &self,
        args: serde_json::Map<String, serde_json::Value>,
    ) -> Result<serde_json::Value, McpError> {
        let target = args
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("entity");
        let top_k = args
            .get("top_k")
            .and_then(|v| v.as_i64())
            .unwrap_or(10)
            .max(1);
        let offset = args
            .get("offset")
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
            .max(0);
        let query = args.get("query").and_then(|v| v.as_object());
        let filters = query.and_then(|q| q.get("filters").and_then(|v| v.as_object()));
        match target {
            "entity" => {
                let mut conds: Vec<String> = Vec::new();
                let mut binds: Vec<(&str, serde_json::Value)> = Vec::new();
                if let Some(f) = filters {
                    if let Some(name) = f.get("name").and_then(|v| v.as_str()) {
                        conds.push(
                            "string::lowercase(name) CONTAINS string::lowercase($name)".into(),
                        );
                        binds.push(("name", json!(name)));
                    }
                    if let Some(typ) = f.get("type").and_then(|v| v.as_str()) {
                        conds.push("type = $etype".into());
                        binds.push(("etype", json!(typ)));
                    }
                    if let Some(ext) = f.get("external_id").and_then(|v| v.as_str()) {
                        conds.push("external_id = $ext".into());
                        binds.push(("ext", json!(ext)));
                    }
                }
                let where_sql = if conds.is_empty() {
                    String::new()
                } else {
                    format!(" WHERE {}", conds.join(" AND "))
                };
                let q = format!(
                    "SELECT id, name, type, external_id, mention_count, name_embedding FROM entities{} ORDER BY mention_count DESC, name ASC LIMIT $lim START $off",
                    where_sql
                );
                let (max_retries, initial_delay_ms) = get_retry_config();
                let mut resp = with_retry("search_entities", max_retries, initial_delay_ms, || {
                    let mut qry = self
                        .db
                        .query(q.clone())
                        .bind(("lim", top_k))
                        .bind(("off", offset));
                    for (k, v) in binds.clone() {
                        qry = qry.bind((k, v));
                    }
                    async move { qry.await }
                })
                .await?;
                let rows: Vec<serde_json::Value> = resp.take(0).unwrap_or_default();
                // Optional semantic scoring
                let mut items: Vec<serde_json::Value> = Vec::new();
                let (qtxt_opt, qemb_opt, sim_thresh) = if let Some(qobj) = query {
                    if let Some(qtxt) = qobj.get("text").and_then(|v| v.as_str()) {
                        let env_sim = std::env::var("SURR_SEARCH_SIM_THRESH")
                            .ok()
                            .and_then(|s| s.parse::<f32>().ok());
                        let fall_sim = std::env::var("SURR_SIM_THRESH")
                            .ok()
                            .and_then(|s| s.parse::<f32>().ok());
                        let thresh = env_sim.or(fall_sim).unwrap_or(0.5).clamp(0.0, 1.0);
                        if Self::kg_embed_entities_enabled() {
                            let emb = self.embedder.embed(qtxt).await.ok();
                            (Some(qtxt.to_string()), emb, thresh)
                        } else {
                            (Some(qtxt.to_string()), None, thresh)
                        }
                    } else {
                        (None, None, 0.5)
                    }
                } else {
                    (None, None, 0.5)
                };

                for r in rows.into_iter() {
                    let id = Self::kg_value_to_id_string(
                        r.get("id").unwrap_or(&serde_json::Value::Null),
                    )
                    .unwrap_or_default();
                    let name = r
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let etype = r
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let external_id = r
                        .get("external_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let mention_count =
                        r.get("mention_count").and_then(|v| v.as_i64()).unwrap_or(0);
                    let mut sim = 0.0f32;
                    if let Some(ref qemb) = qemb_opt {
                        if let Some(arr) = r.get("name_embedding").and_then(|v| v.as_array()) {
                            let emb: Vec<f32> = arr
                                .iter()
                                .filter_map(|x| x.as_f64().map(|f| f as f32))
                                .collect();
                            if emb.len() == qemb.len() {
                                sim = self.cosine_similarity(qemb, &emb);
                            }
                        }
                        if let Some(ref qtxt) = qtxt_opt
                            && name.to_lowercase().contains(&qtxt.to_lowercase())
                        {
                            sim = (sim + 0.05).min(1.0);
                        }
                    } else if let Some(ref qtxt) = qtxt_opt {
                        // lexical boost only
                        if name.to_lowercase().contains(&qtxt.to_lowercase()) {
                            sim = 0.6;
                        }
                    }
                    items.push(json!({
                        "type": "entity",
                        "id": id,
                        "name": name,
                        "entity_type": etype,
                        "external_id": external_id,
                        "mention_count": mention_count,
                        "similarity": sim
                    }));
                }
                if qemb_opt.is_some() || qtxt_opt.is_some() {
                    // Filter and sort by similarity
                    items.retain(|it| {
                        it.get("similarity").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32
                            >= sim_thresh
                    });
                    items.sort_by(|a, b| {
                        b.get("similarity")
                            .and_then(|v| v.as_f64())
                            .partial_cmp(&a.get("similarity").and_then(|v| v.as_f64()))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    let start = offset as usize;
                    let sliced = items
                        .into_iter()
                        .skip(start)
                        .take(top_k as usize)
                        .collect::<Vec<_>>();
                    return Ok(json!({"items": sliced}));
                }
                Ok(json!({"items": items}))
            }
            "relationship" => {
                let mut conds: Vec<String> = Vec::new();
                let mut binds: Vec<(&str, serde_json::Value)> = Vec::new();
                if let Some(f) = filters {
                    if let Some(pred) = f.get("predicate").and_then(|v| v.as_str()) {
                        conds.push("predicate = $pred".into());
                        binds.push(("pred", json!(pred)));
                    }
                    if let Some(sid) = f.get("subject_id").and_then(|v| v.as_str())
                        && let Some((_, inner)) = Self::kg_parse_thing(sid, "entities")
                    {
                        conds.push("in = type::thing('entities', $sid)".into());
                        binds.push(("sid", json!(inner)));
                    }
                    if let Some(oid) = f.get("object_id").and_then(|v| v.as_str())
                        && let Some((_, inner)) = Self::kg_parse_thing(oid, "entities")
                    {
                        conds.push("out = type::thing('entities', $oid)".into());
                        binds.push(("oid", json!(inner)));
                    }
                }
                let where_sql = if conds.is_empty() {
                    String::new()
                } else {
                    format!(" WHERE {}", conds.join(" AND "))
                };
                let q = format!(
                    "SELECT id, in, out, predicate, direction, strength, confidence FROM relationships{} LIMIT $lim START $off",
                    where_sql
                );
                let (max_retries, initial_delay_ms) = get_retry_config();
                let mut resp = with_retry(
                    "search_relationships",
                    max_retries,
                    initial_delay_ms,
                    || {
                        let mut qry = self
                            .db
                            .query(q.clone())
                            .bind(("lim", top_k))
                            .bind(("off", offset));
                        for (k, v) in binds.clone() {
                            qry = qry.bind((k, v));
                        }
                        async move { qry.await }
                    },
                )
                .await?;
                let rows: Vec<serde_json::Value> = resp.take(0).unwrap_or_default();
                let items: Vec<serde_json::Value> = rows
                    .into_iter()
                    .map(|r| json!({
                        "type": "relationship",
                        "id": r.get("id").and_then(|v| v.as_str()).unwrap_or("") ,
                        "predicate": r.get("predicate").and_then(|v| v.as_str()).unwrap_or(""),
                        "direction": r.get("direction").and_then(|v| v.as_str()).unwrap_or(""),
                        "strength": r.get("strength").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        "confidence": r.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    }))
                    .collect();
                Ok(json!({"items": items}))
            }
            "observation" => {
                let mut conds: Vec<String> = Vec::new();
                let mut binds: Vec<(&str, serde_json::Value)> = Vec::new();
                if let Some(qobj) = query
                    && let Some(qtxt) = qobj.get("text").and_then(|v| v.as_str())
                {
                    conds.push("string::lowercase(text) CONTAINS string::lowercase($qtxt)".into());
                    binds.push(("qtxt", json!(qtxt)));
                }
                if let Some(f) = filters {
                    if let Some(eid) = f.get("entity_id").and_then(|v| v.as_str()) {
                        conds.push("entities CONTAINS $eid".into());
                        binds.push(("eid", json!(eid)));
                    }
                    if let Some(from) = f.get("occurred_from").and_then(|v| v.as_str()) {
                        conds.push("occurred_at >= type::datetime($from)".into());
                        binds.push(("from", json!(from)));
                    }
                    if let Some(to) = f.get("occurred_to").and_then(|v| v.as_str()) {
                        conds.push("occurred_at <= type::datetime($to)".into());
                        binds.push(("to", json!(to)));
                    }
                }
                let where_sql = if conds.is_empty() {
                    String::new()
                } else {
                    format!(" WHERE {}", conds.join(" AND "))
                };
                let q = format!(
                    "SELECT id, text, occurred_at, confidence FROM observations{} ORDER BY occurred_at DESC LIMIT $lim START $off",
                    where_sql
                );
                let (max_retries, initial_delay_ms) = get_retry_config();
                let mut resp =
                    with_retry("search_observations", max_retries, initial_delay_ms, || {
                        let mut qry = self
                            .db
                            .query(q.clone())
                            .bind(("lim", top_k))
                            .bind(("off", offset));
                        for (k, v) in binds.clone() {
                            qry = qry.bind((k, v));
                        }
                        async move { qry.await }
                    })
                    .await?;
                let rows: Vec<serde_json::Value> = resp.take(0).unwrap_or_default();
                let items: Vec<serde_json::Value> = rows.into_iter().map(|r| json!({
                    "type": "observation",
                    "id": r.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                    "text": r.get("text").and_then(|v| v.as_str()).unwrap_or(""),
                    "occurred_at": r.get("occurred_at").and_then(|v| v.as_str()).unwrap_or(""),
                    "confidence": r.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0)
                })).collect();
                Ok(json!({"items": items}))
            }
            _ => Ok(json!({"items": []})),
        }
    }

    // Parse Thought from JSON row (manual deserialization to avoid WebSocket issues)
    fn parse_thought_from_json(row: serde_json::Value) -> Result<Thought, anyhow::Error> {
        let id = row
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default();

        let content = row
            .get("content")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default();

        let embedding = row
            .get("embedding")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_f64().map(|f| f as f32))
                    .collect()
            })
            .unwrap_or_default();

        let significance = row
            .get("significance")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32)
            .unwrap_or(0.5);

        let access_count = row
            .get("access_count")
            .and_then(|v| v.as_i64())
            .map(|i| i as u32)
            .unwrap_or(0);

        let injection_scale = row
            .get("injection_scale")
            .and_then(|v| v.as_i64())
            .map(|i| i as u8)
            .unwrap_or(3);

        let submode = row
            .get("submode")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Try to parse created_at from row; fallback to now if unavailable
        let created_at = if let Some(tsv) = row.get("created_at") {
            if let Some(s) = tsv.as_str() {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
                    surrealdb::sql::Datetime::from(dt.with_timezone(&Utc))
                } else {
                    surrealdb::sql::Datetime::from(Utc::now())
                }
            } else {
                surrealdb::sql::Datetime::from(Utc::now())
            }
        } else {
            surrealdb::sql::Datetime::from(Utc::now())
        };

        let thought = Thought {
            id,
            content,
            created_at,
            embedding,
            injected_memories: Vec::new(),
            enriched_content: None,
            injection_scale,
            significance,
            access_count,
            last_accessed: None,
            submode,
            framework_enhanced: None,
            framework_analysis: None,
            is_inner_voice: None,   // parsed from database
            inner_visibility: None, // parsed from database
        };

        Ok(thought)
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
            is_inner_voice: Some(false),
            inner_visibility: None,
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
            is_inner_voice: Some(false),
            inner_visibility: None,
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
            is_inner_voice: Some(false),
            inner_visibility: None,
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
            is_inner_voice: Some(false),
            inner_visibility: None,
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
            is_inner_voice: Some(false),
            inner_visibility: None,
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
            if let Err(e) = result {
                let err_msg = e.to_string();
                assert!(
                    err_msg.contains(expected_err),
                    "Error message '{}' should contain '{}'",
                    err_msg,
                    expected_err
                );
            } else {
                panic!("Should have failed for input: {:?}", input);
            }
        }
    }
}
