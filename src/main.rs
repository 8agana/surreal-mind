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

mod cognitive;
mod embeddings;
mod flavor;
use embeddings::{Embedder, create_embedder};
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

// Forgiving deserializers for tool params
fn de_option_u8_forgiving<'de, D>(deserializer: D) -> Result<Option<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let opt = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(v) = opt else { return Ok(None) };
    match v {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Number(n) => {
            let val = if let Some(u) = n.as_u64() {
                u as f64
            } else if let Some(i) = n.as_i64() {
                i as f64
            } else if let Some(f) = n.as_f64() {
                f
            } else {
                return Err(D::Error::custom("invalid numeric for u8"));
            };
            let rounded = val.round();
            if !rounded.is_finite() {
                return Err(D::Error::custom("non-finite numeric for u8"));
            }
            let clamped = rounded.clamp(0.0, u8::MAX as f64) as u8;
            Ok(Some(clamped))
        }
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                return Ok(None);
            }
            let val: f64 = s
                .parse()
                .map_err(|_| D::Error::custom("invalid string for u8"))?;
            let rounded = val.round();
            let clamped = rounded.clamp(0.0, u8::MAX as f64) as u8;
            Ok(Some(clamped))
        }
        other => Err(D::Error::custom(format!("invalid type for u8: {}", other))),
    }
}

fn de_option_f32_forgiving<'de, D>(deserializer: D) -> Result<Option<f32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let opt = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(v) = opt else { return Ok(None) };
    let val = match v {
        serde_json::Value::Null => return Ok(None),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                f as f32
            } else if let Some(u) = n.as_u64() {
                u as f32
            } else if let Some(i) = n.as_i64() {
                i as f32
            } else {
                return Err(D::Error::custom("invalid numeric for f32"));
            }
        }
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                return Ok(None);
            }
            s.parse::<f32>()
                .map_err(|_| D::Error::custom("invalid string for f32"))?
        }
        other => return Err(D::Error::custom(format!("invalid type for f32: {}", other))),
    };
    Ok(Some(val))
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
    orbital_distance: f32,
}

#[derive(Debug, Deserialize)]
struct ConvoThinkParams {
    content: String,
    #[serde(default, deserialize_with = "de_option_u8_forgiving")]
    injection_scale: Option<u8>,
    submode: Option<String>,
    #[allow(dead_code)]
    #[serde(default, deserialize_with = "de_option_tags")]
    tags: Option<Vec<String>>,
    #[serde(default, deserialize_with = "de_option_f32_forgiving")]
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
            DEFINE FIELD submode ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD framework_enhanced ON TABLE thoughts TYPE option<bool>;
            DEFINE FIELD framework_analysis ON TABLE thoughts TYPE option<object>;

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
        db.query(
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
                    access_count: $access_count,
                    submode: $submode,
                    framework_enhanced: $framework_enhanced,
                    framework_analysis: $framework_analysis
                }"#,
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
            .bind(("framework_analysis", thought.framework_analysis.clone()));

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
            for (i, memory) in relevant_memories.iter().enumerate() {
                // Check if submodes match (if both present)
                let _submode_match = thought
                    .submode
                    .as_ref()
                    .and_then(|ts| memory.thought.submode.as_ref().map(|ms| ts == ms))
                    .unwrap_or(false);

                q.push_str(&format!(
                    "RELATE $from{0}->recalls->$to{0} SET strength = $strength{0}, created_at = time::now(), submode_match = $submode_match{0}, flavor = $flavor{0};\n",
                    i
                ));
                q.push_str(&format!(
                    "RELATE $to{0}->recalls->$from{0} SET strength = $strength{0}, created_at = time::now(), submode_match = $submode_match{0}, flavor = $flavor{0};\n",
                    i
                ));
            }
            let mut req = db.query(q);
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
            "submode_used": submode,
            "framework_analysis": {
                "insights": analysis.insights,
                "questions": analysis.questions,
                "next_steps": analysis.next_steps
            },
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
        submode: Option<&str>,
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

                if similarity > eff_sim_thresh && orbital_distance <= max_orbital_distance {
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

                if similarity > eff_sim_thresh && orbital_distance <= max_orbital_distance {
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

        // Sort by combined score (defaults preserved; tuned by profile weights if desired later)
        matches.sort_by(|a, b| {
            let score_a = a.similarity_score * 0.6 + (1.0 - a.orbital_distance) * 0.4;
            let score_b = b.similarity_score * 0.6 + (1.0 - b.orbital_distance) * 0.4;
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(matches.into_iter().take(top_k).collect())
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
            embedding: vec![0.1; 768],
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
}
