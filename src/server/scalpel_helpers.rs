//! Internal helper methods for Scalpel tool access to SurrealMind functionality

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use serde_json::{Value, json};
use uuid::Uuid;

impl SurrealMindServer {
    /// Internal think method for Scalpel
    pub async fn think_internal(&self, content: &str, tags: Vec<String>) -> Result<String> {
        let embedding = self.embedder.embed(content).await?;
        let thought_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        // Use raw SQL INSERT to match main think implementation
        self.db
            .query(
                "INSERT INTO thoughts {
                id: $id,
                content: $content,
                created_at: $created_at,
                embedding: $embedding,
                tags: $tags,
                origin: 'scalpel',
                significance: 0.5,
                access_count: 0,
                injected_memories: [],
                injection_scale: 0,
                enriched_content: NONE,
                submode: NONE,
                framework_enhanced: false,
                extracted_to_kg: false,
                embedding_model: $embed_model,
                embedding_dim: $embed_dim,
                embedding_provider: 'openai'
            }",
            )
            .bind(("id", format!("thoughts:{}", thought_id)))
            .bind(("content", content.to_string()))
            .bind(("created_at", now))
            .bind(("embedding", embedding))
            .bind(("tags", tags))
            .bind(("embed_model", "text-embedding-3-small".to_string()))
            .bind(("embed_dim", 1536i64))
            .await
            .map_err(|e| SurrealMindError::Database {
                message: e.to_string(),
            })?;

        Ok(format!("thoughts:{}", thought_id))
    }

    /// Internal search method for Scalpel
    pub async fn search_internal(&self, query: Value) -> Result<Value> {
        // Extract query text
        let query_text = query
            .get("text")
            .or_else(|| query.get("query"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if query_text.is_empty() {
            return Ok(json!({"results": [], "error": "No query text provided"}));
        }

        let embedding = self.embedder.embed(query_text).await?;
        let top_k = query.get("top_k").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

        // Vector search for thoughts - clone embedding for 'static requirement
        let emb_clone = embedding.clone();
        let results: Vec<Value> = self.db
            .query("SELECT id, content, created_at, vector::similarity::cosine(embedding, $emb) AS score FROM thoughts WHERE embedding != [] ORDER BY score DESC LIMIT $k")
            .bind(("emb", emb_clone))
            .bind(("k", top_k))
            .await
            .map_err(|e| SurrealMindError::Database {
                message: e.to_string(),
            })?
            .take(0)
            .unwrap_or_default();

        // Also search KG entities
        let emb_clone2 = embedding.clone();
        let kg_results: Vec<Value> = self.db
            .query("SELECT id, name, entity_type, observations, vector::similarity::cosine(embedding, $emb) AS score FROM kg_entities WHERE embedding != [] ORDER BY score DESC LIMIT $k")
            .bind(("emb", emb_clone2))
            .bind(("k", top_k))
            .await
            .map_err(|e| SurrealMindError::Database {
                message: e.to_string(),
            })?
            .take(0)
            .unwrap_or_default();

        Ok(json!({
            "thoughts": results,
            "entities": kg_results
        }))
    }

    /// Internal remember method for Scalpel
    pub async fn remember_internal(&self, kind: &str, data: Value) -> Result<String> {
        match kind {
            "entity" => {
                let name = data
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let entity_type = data
                    .get("entity_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("concept")
                    .to_string();
                let observations: Vec<String> = data
                    .get("observations")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                // Create embedding from name + observations
                let embed_text = format!("{}: {}", name, observations.join("; "));
                let embedding = self.embedder.embed(&embed_text).await?;
                let entity_id = Uuid::new_v4().to_string();

                self.db
                    .query(
                        "INSERT INTO kg_entities {
                        id: $id,
                        name: $name,
                        entity_type: $entity_type,
                        observations: $observations,
                        embedding: $embedding,
                        created_at: $created_at,
                        origin: 'scalpel'
                    }",
                    )
                    .bind(("id", format!("kg_entities:{}", entity_id)))
                    .bind(("name", name))
                    .bind(("entity_type", entity_type))
                    .bind(("observations", observations))
                    .bind(("embedding", embedding))
                    .bind(("created_at", chrono::Utc::now().to_rfc3339()))
                    .await
                    .map_err(|e| SurrealMindError::Database {
                        message: e.to_string(),
                    })?;

                Ok(format!("kg_entities:{}", entity_id))
            }
            "relationship" => {
                let from = data
                    .get("from")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let to = data
                    .get("to")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let rel_type = data
                    .get("relationship_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("related_to")
                    .to_string();

                if from.is_empty() || to.is_empty() {
                    return Ok("Error: 'from' and 'to' are required".to_string());
                }

                let rel_id = Uuid::new_v4().to_string();

                self.db
                    .query(
                        "INSERT INTO kg_relationships {
                        id: $id,
                        from_entity: $from,
                        to_entity: $to,
                        relationship_type: $rel_type,
                        created_at: $created_at,
                        origin: 'scalpel'
                    }",
                    )
                    .bind(("id", format!("kg_relationships:{}", rel_id)))
                    .bind(("from", from))
                    .bind(("to", to))
                    .bind(("rel_type", rel_type))
                    .bind(("created_at", chrono::Utc::now().to_rfc3339()))
                    .await
                    .map_err(|e| SurrealMindError::Database {
                        message: e.to_string(),
                    })?;

                Ok(format!("kg_relationships:{}", rel_id))
            }
            "observation" => {
                let entity = data
                    .get("entity")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let content = data
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                if entity.is_empty() || content.is_empty() {
                    return Ok("Error: 'entity' and 'content' are required".to_string());
                }

                // Add observation to entity
                self.db
                    .query("UPDATE $entity SET observations += $obs")
                    .bind(("entity", entity.clone()))
                    .bind(("obs", content))
                    .await
                    .map_err(|e| SurrealMindError::Database {
                        message: e.to_string(),
                    })?;

                Ok(format!("Added observation to {}", entity))
            }
            _ => Ok(format!("Unknown kind: {}", kind)),
        }
    }
}
