use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;

/// Parameters for the wander tool
#[derive(Debug, serde::Deserialize)]
pub struct WanderParams {
    /// Optional starting point thought/entity ID. If None, starts random.
    pub current_thought_id: Option<String>,

    /// Traversal mode: "random", "semantic", "meta"
    pub mode: String,

    /// IDs to avoid (breadcrumbs/history) to prevent loops
    #[serde(default)]
    pub visited_ids: Vec<String>,

    /// Whether to prioritize recent memories
    #[serde(default)]
    pub recency_bias: bool,
}

impl SurrealMindServer {
    /// Handle the wander tool call
    pub async fn handle_wander(&self, request: CallToolRequestParam) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: WanderParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::InvalidParams {
                message: format!("Invalid parameters: {}", e),
            })?;

        // 1. Determine current context
        let current_node = if let Some(id) = &params.current_thought_id {
            // Validate existence
            // If ID doesn't look like a record ID (no colon), try to find it in known tables
            let q = if id.contains(':') {
                "SELECT meta::id(id) as id, * FROM $id"
            } else {
                "SELECT meta::id(id) as id, * FROM thoughts, kg_entities, kg_observations 
                  WHERE id = type::thing('thoughts', $id) 
                     OR id = type::thing('kg_entities', $id) 
                     OR id = type::thing('kg_observations', $id)
                  LIMIT 1"
            };

            let res: Vec<serde_json::Value> =
                self.db.query(q).bind(("id", id.clone())).await?.take(0)?;

            if res.is_empty() {
                // If ID invalid, fall back to random start
                None
            } else {
                Some(res[0].clone())
            }
        } else {
            // Auto-seed context if mode requires it (Semantic/Meta)
            // This enables "Start Wandering" without needing a specific ID
            if params.mode == "semantic" || params.mode == "meta" {
                let q = if params.recency_bias {
                    "SELECT meta::id(id) as id, * FROM thoughts ORDER BY created_at DESC LIMIT 1"
                } else {
                    "SELECT meta::id(id) as id, * FROM thoughts, kg_entities, kg_observations ORDER BY rand() LIMIT 1"
                };

                let res: Vec<serde_json::Value> = self.db.query(q).await?.take(0)?;
                res.first().cloned()
            } else {
                None
            }
        };

        // 2. Traversal Logic
        let (next_node, affordances) = match params.mode.as_str() {
            "random" => self.wander_random(&params.visited_ids).await?,
            "semantic" => {
                self.wander_semantic(&current_node, &params.visited_ids)
                    .await?
            }
            "meta" => self.wander_meta(&current_node, &params.visited_ids).await?,
            _ => {
                return Err(SurrealMindError::Validation {
                    message: format!("Unknown mode: {}", params.mode),
                });
            }
        };

        // 3. Construct Response
        let guidance = "DISCOVERY GUIDANCE: You are in 'Curiosity Mode'. Use this discovery to better the Knowledge Graph. Ask yourself: Is this information accurate? What context is missing? Are there related entities or observations you can link? Use the 'remember' tool to commit improvements or 'think' to reason about the connection.";
        let response = json!({
            "previous_id": params.current_thought_id,
            "current_node": next_node,
            "mode_used": params.mode,
            "affordances": affordances,
            "guidance": guidance,
            "message": if next_node.is_none() { "Dead end or invalid start." } else { "Wander step complete." }
        });

        Ok(CallToolResult::structured(response))
    }

    /// Mode: Random - Jump to a completely random node
    async fn wander_random(
        &self,
        visited: &[String],
    ) -> Result<(Option<serde_json::Value>, Vec<String>)> {
        // We select from thoughts, kg_entities, or kg_observations
        // For simplicity, let's union or just pick one table randomly?
        // Let's query all three and pick one. But that's expensive.
        // Better: Select from a random table.

        // Simple heuristic: just query one of them. Or UNION?
        // Simple heuristic: just query one of them. Or UNION?
        // Providing true randomness across tables in Surreal is tricky without a unified view.
        // Let's just pick one table at random in Rust logic? No, let's just query limits.

        // Actually, let's use a UNION-like approach or just simple fallback order.
        // "SELECT * FROM thoughts, kg_entities, kg_observations ORDER BY rand() LIMIT 1" (SurrealDB might support comma separated targets? Yes.)

        let q = "SELECT meta::id(id) as id, * FROM thoughts, kg_entities, kg_observations WHERE meta::id(id) NOT IN $visited ORDER BY rand() LIMIT 1";
        let res: Vec<serde_json::Value> = self
            .db
            .query(q)
            .bind(("visited", visited.to_vec()))
            .await?
            .take(0)?;

        let node = res.first().cloned();
        let affordances = vec![
            "semantic".to_string(),
            "meta".to_string(),
            "random".to_string(),
        ];

        Ok((node, affordances))
    }

    /// Mode: Semantic - Nearest neighbor to current node
    async fn wander_semantic(
        &self,
        current: &Option<serde_json::Value>,
        visited: &[String],
    ) -> Result<(Option<serde_json::Value>, Vec<String>)> {
        let current = match current {
            Some(c) => c,
            None => return self.wander_random(visited).await, // Fallback if no start node
        };

        // Check if context has embedding
        let embedding = current.get("embedding").cloned();
        if embedding.is_none() || embedding.as_ref().unwrap().is_null() {
            // No embedding? Can't do semantic. Fallback to meta or random.
            return self.wander_meta(&Some(current.clone()), visited).await;
        }

        // Search for nearest neighbors
        // Use a threshold to ensure relevance, but loose enough for wandering
        // Note: Casting id to string for comparison safety
        // Note: Must filter for valid embeddings to avoid vector function errors
        let q = "SELECT meta::id(id) as id, *, vector::similarity::cosine(embedding, $emb) as sim 
                 FROM thoughts, kg_entities, kg_observations 
                 WHERE meta::id(id) NOT IN $visited 
                 AND <string>meta::id(id) != $current_id
                 AND embedding != NONE 
                 AND type::is::array(embedding)
                 ORDER BY sim DESC LIMIT 1";

        let current_id = current
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let res: Vec<serde_json::Value> = self
            .db
            .query(q)
            .bind(("emb", embedding))
            .bind(("visited", visited.to_vec()))
            .bind(("current_id", current_id))
            .await?
            .take(0)?;

        let node = res.first().cloned();
        // Affordances: if we found something, maybe suggest meta?
        let affordances = vec!["random".to_string(), "meta".to_string()];
        Ok((node, affordances))
    }

    /// Mode: Meta - Navigate via tags, origin, or other metadata
    async fn wander_meta(
        &self,
        current: &Option<serde_json::Value>,
        visited: &[String],
    ) -> Result<(Option<serde_json::Value>, Vec<String>)> {
        let current = match current {
            Some(c) => c,
            None => return self.wander_random(visited).await,
        };

        // Strategy: Find nodes that share tags or source_thought_id
        // 1. Extract tags
        let tags_val = current.get("tags").cloned();
        let current_id = current
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // If meta navigation isn't possible, fallback to random
        if tags_val.is_none() {
            return self.wander_random(visited).await;
        }

        // Query: Overlap in tags
        // "SELECT * FROM ... WHERE tags CONTAINSANY $tags ..."
        let q = "SELECT meta::id(id) as id, * FROM thoughts, kg_entities, kg_observations 
                 WHERE meta::id(id) NOT IN $visited 
                 AND <string>meta::id(id) != $current_id
                 AND (tags CONTAINSANY $tags OR data.tags CONTAINSANY $tags)
                 ORDER BY rand() LIMIT 1";

        let res: Vec<serde_json::Value> = self
            .db
            .query(q)
            .bind(("tags", tags_val))
            .bind(("visited", visited.to_vec()))
            .bind(("current_id", current_id))
            .await?
            .take(0)?;

        let node = res.first().cloned();
        if node.is_none() {
            // If no tag overlap, try random fallback
            return self.wander_random(visited).await;
        }

        let affordances = vec!["semantic".to_string(), "random".to_string()];
        Ok((node, affordances))
    }
}
