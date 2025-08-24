use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rmcp::{
    ErrorData as McpError,
    handler::server::ServerHandler,
    model::{
        CallToolRequestParam, CallToolResult, Implementation, InitializeRequestParam,
        InitializeResult, ListToolsResult, PaginatedRequestParam, ProtocolVersion,
        ServerCapabilities, ServerInfo, Tool, ToolsCapability,
    },
    service::{RequestContext, RoleServer, serve_server},
    transport::stdio,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::borrow::Cow;

use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::local::{Db, RocksDb};
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

mod embeddings;
use embeddings::{Embedder, create_embedder};

// Data models
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Thought {
    id: String,
    content: String,
    created_at: DateTime<Utc>,
    embedding: Vec<f32>,
    injected_memories: Vec<String>,
    enriched_content: Option<String>,
    injection_scale: u8,
    significance: f32,
    access_count: u32,
    last_accessed: Option<DateTime<Utc>>,
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
    db: Arc<RwLock<Surreal<Db>>>,
    thoughts: Arc<RwLock<Vec<Thought>>>, // In-memory cache for fast retrieval
    embedder: Arc<dyn Embedder>,
}

impl ServerHandler for SurrealMindServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(true),
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
        _request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        Ok(self.get_info())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
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

                info!("convo_think called with: {}", params.content);
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
        info!("Initializing SurrealDB with embedded RocksDB");

        let db_path = format!("{}/surreal_data", env!("CARGO_MANIFEST_DIR"));
        let db = Surreal::new::<RocksDb>(db_path)
            .await
            .context("Failed to create SurrealDB instance")?;

        db.use_ns("surreal_mind")
            .use_db("consciousness")
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
        let significance = params.significance.unwrap_or(0.5);

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
            created_at: Utc::now(),
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

        // Store thought in SurrealDB
        let db = self.db.write().await;
        let stored: Option<Thought> = db
            .create(("thoughts", thought.id.clone()))
            .content(thought.clone())
            .await
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Failed to store thought: {}", e).into(),
                data: None,
            })?;

        let stored_thought = stored.ok_or_else(|| McpError {
            code: rmcp::model::ErrorCode::INTERNAL_ERROR,
            message: "No thought returned from database".into(),
            data: None,
        })?;

        // Create bidirectional relationships with injected memories
        for memory in &relevant_memories {
            db.query(
                r#"
                RELATE $from->recalls->$to
                SET strength = $strength,
                    created_at = $created_at
            "#,
            )
            .bind(("from", format!("thoughts:{}", stored_thought.id)))
            .bind(("to", format!("thoughts:{}", memory.thought.id)))
            .bind(("strength", memory.similarity_score))
            .bind(("created_at", Utc::now()))
            .await
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Failed to create relationship: {}", e).into(),
                data: None,
            })?;
        }

        // Also keep in memory for fast retrieval
        let mut thoughts = self.thoughts.write().await;
        thoughts.push(stored_thought.clone());

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
                    relevant_memories.first().map(|m| m.orbital_distance).unwrap_or(0.0),
                    relevant_memories.last().map(|m| m.orbital_distance).unwrap_or(0.0)
                )
            }
        }))
    }

    async fn retrieve_memories_for_injection(
        &self,
        query_embedding: &[f32],
        injection_scale: u8,
    ) -> Result<Vec<ThoughtMatch>, McpError> {
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
        let now = Utc::now();
        let thoughts = self.thoughts.read().await;
        let mut matches = Vec::new();

        // If we have thoughts in memory, use them
        if !thoughts.is_empty() {
            for thought in thoughts.iter() {
                // Calculate cosine similarity (simplified)
                let similarity = self.cosine_similarity(query_embedding, &thought.embedding);

                // Calculate orbital distance
                let age_factor = (now - thought.created_at).num_seconds() as f32 / 86400.0;
                let access_factor = (thought.access_count as f32 + 1.0).ln() / 10.0;
                let orbital_distance = 1.0
                    - (age_factor * 0.4 + access_factor * 0.3 + thought.significance * 0.3)
                        .clamp(0.0, 1.0);

                if similarity > 0.5 && orbital_distance <= max_orbital_distance {
                    matches.push(ThoughtMatch {
                        thought: thought.clone(),
                        similarity_score: similarity,
                        orbital_distance,
                    });
                }
            }
        } else {
            // Fall back to querying SurrealDB
            let db = self.db.read().await;
            let results: Vec<Thought> = db.select("thoughts").await.map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Failed to query thoughts: {}", e).into(),
                data: None,
            })?;

            for thought in results {
                let similarity = self.cosine_similarity(query_embedding, &thought.embedding);
                let age_factor = (now - thought.created_at).num_seconds() as f32 / 86400.0;
                let access_factor = (thought.access_count as f32 + 1.0).ln() / 10.0;
                let orbital_distance = 1.0
                    - (age_factor * 0.4 + access_factor * 0.3 + thought.significance * 0.3)
                        .clamp(0.0, 1.0);

                if similarity > 0.5 && orbital_distance <= max_orbital_distance {
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

        Ok(matches.into_iter().take(5).collect())
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
        // Simplified cosine similarity
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

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if it exists
    dotenv::dotenv().ok();

    // Initialize tracing with env filter
    let filter =
        std::env::var("RUST_LOG").unwrap_or_else(|_| "surreal_mind=debug,rmcp=info".to_string());
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .init();

    info!("Starting Surreal Mind MCP Server with consciousness persistence");

    let server = SurrealMindServer::new().await?;
    let transport = stdio();

    serve_server(server, transport).await?;

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
        // Test that server can be created successfully
        let server = SurrealMindServer::new().await;
        assert!(server.is_ok(), "Server should initialize successfully");
    }

    #[test]
    fn test_thought_structure() {
        let thought = Thought {
            id: "test".to_string(),
            content: "test content".to_string(),
            created_at: chrono::Utc::now(),
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
