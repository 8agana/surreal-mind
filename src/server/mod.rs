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

        let convo_think_schema_map = crate::schemas::convo_think_schema();

        let tech_think_schema_map = crate::schemas::tech_think_schema();

        let inner_voice_schema_map = crate::schemas::inner_voice_schema();

        let maintenance_ops_schema_map = crate::schemas::maintenance_ops_schema();

        let search_thoughts_schema_map = crate::schemas::search_thoughts_schema();

        let kg_create_schema_map = crate::schemas::kg_create_schema();

        let kg_search_schema_map = crate::schemas::kg_search_schema();

        let kg_moderate_schema_map = crate::schemas::kg_moderate_schema();

        let detailed_help_schema_map = crate::schemas::detailed_help_schema();

        let tools = vec![
            Tool {
                name: "convo_think".into(),
                description: Some("Store conversational thoughts with memory injection".into()),
                input_schema: convo_think_schema_map,
                annotations: None,
                output_schema: None,
            },
            Tool {
                name: "tech_think".into(),
                description: Some("Technical reasoning with memory injection".into()),
                input_schema: tech_think_schema_map,
                annotations: None,
                output_schema: None,
            },
            Tool {
                name: "inner_voice".into(),
                description: Some("Private inner thoughts with visibility controls".into()),
                input_schema: inner_voice_schema_map,
                annotations: None,
                output_schema: None,
            },
            Tool {
                name: "maintenance_ops".into(),
                description: Some("Maintenance operations for archival and cleanup".into()),
                input_schema: maintenance_ops_schema_map,
                annotations: None,
                output_schema: None,
            },
            Tool {
                name: "search_thoughts".into(),
                description: Some("Search thoughts with similarity and graph expansion".into()),
                input_schema: search_thoughts_schema_map,
                annotations: None,
                output_schema: None,
            },
            Tool {
                name: "knowledgegraph_create".into(),
                description: Some("Create entities and relationships in the KG".into()),
                input_schema: kg_create_schema_map,
                annotations: None,
                output_schema: None,
            },
            Tool {
                name: "knowledgegraph_search".into(),
                description: Some("Search entities/relationships in the KG".into()),
                input_schema: kg_search_schema_map,
                annotations: None,
                output_schema: None,
            },
            Tool {
                name: "knowledgegraph_moderate".into(),
                description: Some("Review and/or decide on KG candidates".into()),
                input_schema: kg_moderate_schema_map,
                annotations: None,
                output_schema: None,
            },
            Tool {
                name: "detailed_help".into(),
                description: Some("Get detailed help for a specific tool".into()),
                input_schema: detailed_help_schema_map,
                annotations: None,
                output_schema: None,
            },
        ];

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
            "convo_think" => self.handle_convo_think(request).await.map_err(|e| e.into()),
            "tech_think" => self.handle_tech_think(request).await.map_err(|e| e.into()),
            "inner_voice" => self.handle_inner_voice(request).await.map_err(|e| e.into()),
            "maintenance_ops" => self
                .handle_maintenance_ops(request)
                .await
                .map_err(|e| e.into()),
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
            "knowledgegraph_moderate" => self
                .handle_knowledgegraph_moderate(request)
                .await
                .map_err(|e| e.into()),
            "detailed_help" => self
                .handle_detailed_help(request)
                .await
                .map_err(|e| e.into()),
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
            DEFINE FIELD is_summary ON TABLE thoughts TYPE option<bool>;
            DEFINE FIELD summary_of ON TABLE thoughts TYPE option<array<string>>;
            DEFINE FIELD pipeline ON TABLE thoughts TYPE option<string>;
            DEFINE FIELD status ON TABLE thoughts TYPE option<string>;
            DEFINE INDEX thoughts_status_idx ON TABLE thoughts FIELDS status;
            DEFINE INDEX idx_thoughts_created ON TABLE thoughts FIELDS created_at;

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
        "#;

        self.db.query(schema_sql).await.map_err(|e| McpError {
            code: rmcp::model::ErrorCode::INTERNAL_ERROR,
            message: format!("Schema init failed: {}", e).into(),
            data: None,
        })?;

        Ok(())
    }

    /// Calculate cosine similarity between two vectors
    #[allow(dead_code)]
    pub fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
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

    /// Perform KG-only memory injection: find similar KG entities and attach their IDs.
    pub async fn inject_memories(
        &self,
        thought_id: &str,
        embedding: &[f32],
        injection_scale: i64,
        submode: Option<&str>,
    ) -> crate::error::Result<(usize, Option<String>)> {
        if injection_scale <= 0 {
            return Ok((0, None));
        }

        // Orbital mechanics: determine limit and threshold from scale
        let scale = injection_scale.clamp(0, 3) as u8;
        let (limit, mut prox_thresh) = match scale {
            0 => (0usize, 1.0f32),
            1 => (5usize, 0.8f32),  // Mercury
            2 => (10usize, 0.6f32), // Venus
            _ => (20usize, 0.4f32), // Mars
        };
        if limit == 0 {
            return Ok((0, None));
        }

        // Optional: submode-aware retrieval tweaks
        // Guarded behind SURR_SUBMODE_RETRIEVAL=true to keep default behavior stable
        if std::env::var("SURR_SUBMODE_RETRIEVAL").ok().as_deref() == Some("true") {
            if let Some(sm) = submode {
                // Use lightweight profile deltas to adjust similarity threshold
                use crate::cognitive::profile::{Submode, profile_for};
                let profile = profile_for(Submode::from_str(sm));
                let delta = profile.injection.threshold_delta;
                // Clamp within [0.0, 0.99]
                prox_thresh = (prox_thresh + delta).clamp(0.0, 0.99);
            }
        }

        // Candidate pool size for KG entities
        let retrieve = std::env::var("SURR_KG_CANDIDATES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(200);

        // Fetch candidate entities (id, name, embedding, entity_type)
        let sql = format!(
            "SELECT meta::id(id) as id, name, data, embedding FROM kg_entities LIMIT {}",
            retrieve
        );
        let rows: Vec<serde_json::Value> = self.db.query(sql).await?.take(0)?;

        // Iterate, compute or reuse embeddings, score by cosine similarity
        let mut scored: Vec<(String, f32, String, String)> = Vec::new();
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
                    // Build text for entity embedding: name + type
                    let name_s = r.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let etype = r
                        .get("data")
                        .and_then(|d| d.get("entity_type"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let text = if etype.is_empty() {
                        name_s.to_string()
                    } else {
                        format!("{} ({})", name_s, etype)
                    };
                    let new_emb = self.embedder.embed(&text).await.unwrap_or_default();
                    if new_emb.len() == embedding.len() {
                        emb_opt = Some(new_emb.clone());
                        // Persist embedding for future fast retrieval (best-effort)
                        let _ = self
                            .db
                            .query("UPDATE type::thing($tb, $id) SET embedding = $emb RETURN meta::id(id) as id")
                            .bind(("tb", "kg_entities"))
                            .bind(("id", id.to_string()))
                            .bind(("emb", new_emb))
                            .await;
                    }
                }
                if let Some(emb_e) = emb_opt {
                    let sim = self.cosine_similarity(embedding, &emb_e);
                    if sim >= prox_thresh {
                        let name_s = r
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let etype = r
                            .get("data")
                            .and_then(|d| d.get("entity_type"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        scored.push((id.to_string(), sim, name_s, etype));
                    }
                }
            }
        }

        // Sort by similarity and take top by orbital limit
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let selected: Vec<(String, f32, String, String)> = scored.into_iter().take(limit).collect();
        let memory_ids: Vec<String> = selected.iter().map(|(id, _, _, _)| id.clone()).collect();

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

        Ok((memory_ids.len(), enriched))
    }
}
