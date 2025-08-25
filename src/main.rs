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

use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::auth::Root;
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use uuid::Uuid;

mod embeddings;
use embeddings::{Embedder, create_embedder};

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ThoughtMatch {
    thought: Thought,
    similarity_score: f32,
    orbital_distance: f32,
}

#[derive(Debug, Deserialize)]
struct ConvoThinkParams {
    content: String,
    injection_scale: Option<u8>,
    #[allow(dead_code)]
    submode: Option<String>,
    #[allow(dead_code)]
    tags: Option<Vec<String>>,
    significance: Option<f32>,
}

#[derive(Clone)]
struct SurrealMindServer {
    db: Arc<RwLock<Surreal<Client>>>,
    thoughts: Arc<RwLock<Vec<Thought>>>, // In-memory cache for fast retrieval
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
        let input_schema = rmcp::object!({
            "type": "object",
            "properties": {
                "content": {"type": "string", "description": "The thought content to store"},
                "injection_scale": {"type": "integer", "description": "Memory injection scale (0-5)", "minimum": 0, "maximum": 5},
                "submode": {"type": "string", "description": "Conversation submode", "enum": ["sarcastic", "philosophical", "empathetic", "problem_solving"]},
                "tags": {"type": "array", "items": {"type": "string"}, "description": "Additional tags"},
                "significance": {"type": "number", "description": "Significance weight (0.0-1.0)", "minimum": 0.0, "maximum": 1.0}
            },
            "required": ["content"]
        });

        Ok(ListToolsResult {
            tools: vec![Tool::new(
                Cow::Borrowed("convo_think"),
                Cow::Borrowed("Store thoughts with memory injection"),
                Arc::new(input_schema),
            )],
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
            _ => Err(McpError {
                code: rmcp::model::ErrorCode::METHOD_NOT_FOUND,
                message: format!("Unknown tool: {}", request.name).into(),
                data: None,
            }),
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

        // Initialize schema
        let server = Self {
            db: Arc::new(RwLock::new(db)),
            thoughts: Arc::new(RwLock::new(Vec::new())),
            embedder,
        };

        server.initialize_schema().await?;

        Ok(server)
    }

    async fn initialize_schema(&self) -> Result<(), McpError> {
        info!("Initializing consciousness graph schema");

        let db = self.db.read().await;

        // Define thoughts table
        db.query(
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

            DEFINE INDEX created_at_idx ON TABLE thoughts COLUMNS created_at;
            DEFINE INDEX significance_idx ON TABLE thoughts COLUMNS significance;
        "#,
        )
        .await
        .map_err(|e| McpError {
            code: rmcp::model::ErrorCode::INTERNAL_ERROR,
            message: format!("Failed to initialize schema: {}", e).into(),
            data: None,
        })?;

        // Define relationships table
        db.query(
            r#"
            DEFINE TABLE recalls SCHEMAFULL;
            DEFINE FIELD in ON TABLE recalls TYPE record<thoughts>;
            DEFINE FIELD out ON TABLE recalls TYPE record<thoughts>;
            DEFINE FIELD strength ON TABLE recalls TYPE float;
            DEFINE FIELD created_at ON TABLE recalls TYPE datetime;
        "#,
        )
        .await
        .map_err(|e| McpError {
            code: rmcp::model::ErrorCode::INTERNAL_ERROR,
            message: format!("Failed to define relationships: {}", e).into(),
            data: None,
        })?;

        info!("Schema initialized successfully");
        Ok(())
    }

    async fn create_thought_with_injection(
        &self,
        params: ConvoThinkParams,
    ) -> Result<serde_json::Value, McpError> {
        let injection_scale = params.injection_scale.unwrap_or(3); // Default Mars level
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
                message: "significance must be between 0.0 and 1.0".into(),
                data: None,
            });
        }

        // Generate real embedding using Nomic
        let embedding = self
            .embedder
            .embed(&params.content)
            .await
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Failed to generate embedding: {}", e).into(),
                data: None,
            })?;

        // Retrieve relevant memories based on injection scale
        let relevant_memories = self
            .retrieve_memories_for_injection(&embedding, injection_scale)
            .await?;

        debug!(
            "Retrieved {} memories for injection at scale {}",
            relevant_memories.len(),
            injection_scale
        );

        // Create enriched content
        let enriched_content =
            self.enrich_content_with_memories(&params.content, &relevant_memories);

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
        };

        // Store thought in SurrealDB with all fields in a single operation to avoid SCHEMAFULL NONE issues
        let db = self.db.write().await;

        // Create record using SurrealQL with server-side datetime to avoid JSON enum serialization issues
        let req = db
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
                    access_count: $access_count
                }"#,
            )
            .bind(("id", thought.id.clone()))
            .bind(("content", thought.content.clone()))
            .bind(("embedding", thought.embedding.clone()))
            .bind(("injected_memories", thought.injected_memories.clone()))
            .bind(("enriched_content", thought.enriched_content.clone()))
            .bind(("injection_scale", thought.injection_scale))
            .bind(("significance", thought.significance))
            .bind(("access_count", thought.access_count));

        req.await.map_err(|e| {
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
            for i in 0..relevant_memories.len() {
                q.push_str(&format!(
                    "RELATE $from{0}->recalls->$to{0} SET strength = $strength{0}, created_at = time::now();\n",
                    i
                ));
                q.push_str(&format!(
                    "RELATE $to{0}->recalls->$from{0} SET strength = $strength{0}, created_at = time::now();\n",
                    i
                ));
            }
            let mut req = db.query(q);
            for (i, memory) in relevant_memories.iter().enumerate() {
                req = req
                    .bind((format!("from{}", i), format!("thoughts:{}", thought.id)))
                    .bind((
                        format!("to{}", i),
                        format!("thoughts:{}", memory.thought.id),
                    ))
                    .bind((format!("strength{}", i), memory.similarity_score));
            }
            req.await.map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Failed to create relationships: {}", e).into(),
                data: None,
            })?;
        }

        // Also keep in memory for fast retrieval
        let mut thoughts = self.thoughts.write().await;
        thoughts.push(thought.clone());

        // Compute explicit min/max for summary
        let (min_dist, max_dist) = relevant_memories
            .iter()
            .fold((1.0_f32, 0.0_f32), |(minv, maxv), m| {
                (minv.min(m.orbital_distance), maxv.max(m.orbital_distance))
            });

        // Create response
        Ok(json!({
            "thought_id": thought.id,
            "memories_injected": relevant_memories.len(),
            "enriched_content": enriched_content,
            "injection_scale": injection_scale,
            "orbital_distances": relevant_memories.iter()
                .map(|m| m.orbital_distance)
                .collect::<Vec<_>>(),
            "memory_summary": if relevant_memories.is_empty() {
                "No relevant memories found".to_string()
            } else {
                format!("Injected {} memories from orbital distances {:.2} to {:.2}",
                    relevant_memories.len(),
                    min_dist,
                    max_dist
                )
            }
        }))
    }

    async fn retrieve_memories_for_injection(
        &self,
        query_embedding: &[f32],
        injection_scale: u8,
    ) -> Result<Vec<ThoughtMatch>, McpError> {
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

        // Calculate orbital distance threshold based on injection scale
        let max_orbital_distance = match injection_scale {
            0 => return Ok(Vec::new()), // No injection
            1 => 0.2,                   // Mercury - only hottest memories
            2 => 0.4,                   // Venus/Earth - recent context
            3 => 0.6,                   // Mars - foundational significance
            4 => 0.8,                   // Jupiter/Saturn - distant connections
            5 => 1.0,                   // Neptune/Pluto - everything relevant
            _ => 0.6,                   // Default to Mars
        };

        // Try to get from in-memory first, fall back to DB if needed
        let now_ts = Utc::now().timestamp();
        let thoughts = self.thoughts.read().await;
        let mut matches = Vec::new();

        // Helper to compute orbital distance with clearer semantics
        let compute_distance = |created_ts: i64, access_count: u32, significance: f32| {
            // Recency closeness: recent → 1.0, old → 0.0 (normalize by 30 days)
            let age_days = (now_ts - created_ts) as f32 / 86_400.0;
            let recency_closeness = (1.0 - (age_days / 30.0)).clamp(0.0, 1.0);
            // Access closeness: more accesses → closer (cap at 1.0)
            let access_closeness = ((access_count as f32 + 1.0).ln() / 5.0).clamp(0.0, 1.0);
            let significance_closeness = significance.clamp(0.0, 1.0);
            let closeness =
                recency_closeness * 0.4 + access_closeness * 0.3 + significance_closeness * 0.3;
            (1.0 - closeness).clamp(0.0, 1.0)
        };

        // If we have thoughts in memory, use them
        if !thoughts.is_empty() {
            for thought in thoughts.iter() {
                // Calculate cosine similarity (simplified)
                let similarity = self.cosine_similarity(query_embedding, &thought.embedding);

                // Calculate orbital distance (smaller = closer)
                let orbital_distance = compute_distance(
                    thought.created_at.timestamp(),
                    thought.access_count,
                    thought.significance,
                );

                if similarity > sim_thresh && orbital_distance <= max_orbital_distance {
                    // Update access metadata in DB (reflect usage)
                    let dbw = self.db.write().await;
                    let _ = dbw
                        .query("UPDATE type::thing('thoughts', $id) SET access_count = $ac, last_accessed = time::now()")
                        .bind(("id", thought.id.clone()))
                        .bind(("ac", thought.access_count + 1))
                        .await;

                    matches.push(ThoughtMatch {
                        thought: thought.clone(),
                        similarity_score: similarity,
                        orbital_distance,
                    });
                }
            }
        } else {
            // Fall back to querying SurrealDB (bounded)
            let db = self.db.read().await;
            // Configurable limit for fallback query
            let limit: usize = std::env::var("SURR_DB_LIMIT")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .map(|v| v.clamp(50, 5000))
                .unwrap_or(500);
            let mut resp = db
                .query(format!(
                    "SELECT * FROM thoughts ORDER BY created_at DESC LIMIT {}",
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

            for mut thought in results {
                let similarity = self.cosine_similarity(query_embedding, &thought.embedding);
                let orbital_distance = compute_distance(
                    thought.created_at.timestamp(),
                    thought.access_count,
                    thought.significance,
                );

                if similarity > sim_thresh && orbital_distance <= max_orbital_distance {
                    // Update access metadata in DB
                    let dbw = self.db.write().await;
                    let _ = dbw
                        .query("UPDATE type::thing('thoughts', $id) SET access_count = $ac, last_accessed = time::now()")
                        .bind(("id", thought.id.clone()))
                        .bind(("ac", thought.access_count + 1))
                        .await;
                    // Reflect in local copy
                    thought.access_count += 1;
                    thought.last_accessed = Some(surrealdb::sql::Datetime::from(Utc::now()));

                    matches.push(ThoughtMatch {
                        thought,
                        similarity_score: similarity,
                        orbital_distance,
                    });
                }
            }
        }

        // Sort by combined score
        matches.sort_by(|a, b| {
            let score_a = a.similarity_score * 0.6 + (1.0 - a.orbital_distance) * 0.4;
            let score_b = b.similarity_score * 0.6 + (1.0 - b.orbital_distance) * 0.4;
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(matches.into_iter().take(top_k).collect())
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
                "\n- Memory {}: {} (similarity: {:.2}, orbital: {:.2})",
                i + 1,
                preview,
                memory.similarity_score,
                memory.orbital_distance
            ));
        }
        enriched.push(']');

        enriched
    }

    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        // Ensure we compute over the same span for dot and norms
        let len = a.len().min(b.len());
        if len == 0 {
            return 0.0;
        }
        let a2 = &a[..len];
        let b2 = &b[..len];
        let dot: f32 = a2.iter().zip(b2.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a2.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b2.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if it exists
    dotenvy::dotenv().ok();

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
        // Test parameter parsing
        let args = serde_json::json!({
            "content": "test thought",
            "injection_scale": 3,
            "significance": 0.8
        });

        let params: ConvoThinkParams = serde_json::from_value(args).unwrap();
        assert_eq!(params.content, "test thought");
        assert_eq!(params.injection_scale, Some(3));
        assert_eq!(params.significance, Some(0.8));
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

    #[tokio::test]
    async fn test_embeddings_functionality() {
        use crate::embeddings::{Embedder, FakeEmbedder};

        let embedder = FakeEmbedder::new(768);
        assert_eq!(embedder.dimensions(), 768);

        // Test that embedding generation is deterministic for same input
        let text = "test content";
        let embedding1 = embedder.embed(text).await.unwrap();
        let embedding2 = embedder.embed(text).await.unwrap();
        assert_eq!(embedding1, embedding2);
        assert_eq!(embedding1.len(), 768);
    }
}
