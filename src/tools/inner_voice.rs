//! inner_voice tool handler for storing private inner thoughts

use crate::error::{Result, SurrealMindError};
use crate::kg_extractor::HeuristicExtractor;
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;

/// Parameters for the inner_voice tool
#[derive(Debug, serde::Deserialize)]
pub struct InnerVoiceParams {
    pub content: String,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_u8_forgiving"
    )]
    pub injection_scale: Option<u8>,
    #[serde(default, deserialize_with = "crate::deserializers::de_option_tags")]
    pub tags: Option<Vec<String>>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_f32_forgiving"
    )]
    pub significance: Option<f32>,
    #[serde(default)]
    pub verbose_analysis: Option<bool>,
    #[serde(default)]
    pub inner_visibility: Option<String>,
    #[serde(default)]
    pub extract_to_kg: Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_f32_forgiving"
    )]
    pub session_hours: Option<f32>,
    #[serde(default)]
    pub dry_run: Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_f32_forgiving"
    )]
    pub confidence_min: Option<f32>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_u64_forgiving"
    )]
    pub max_nodes: Option<u64>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_u64_forgiving"
    )]
    pub max_edges: Option<u64>,
}

impl SurrealMindServer {
    /// Handle the inner_voice tool call
    pub async fn handle_inner_voice(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: InnerVoiceParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::Serialization {
                message: format!("Invalid parameters: {}", e),
            })?;

        // Redact content at info level to avoid logging private thoughts
        tracing::info!("inner_voice called (content_len={})", params.content.len());
        let dbg_preview: String = params.content.chars().take(50).collect(); // Shorter preview for privacy
        tracing::debug!("inner_voice content (first 50 chars): {}", dbg_preview);

        // Compute embedding
        let embedding = self.embedder.embed(&params.content).await.map_err(|e| {
            SurrealMindError::Embedding {
                message: e.to_string(),
            }
        })?;

        let injection_scale = params.injection_scale.unwrap_or(0) as i64; // default minimal
        let significance = params.significance.unwrap_or(0.4_f32) as f64;
        let inner_visibility = params
            .inner_visibility
            .unwrap_or_else(|| "context_only".to_string());

        // Check for auto-trigger from content pattern
        let auto_extract =
            std::env::var("AUTO_EXTRACT").unwrap_or_else(|_| "false".to_string()) == "true";
        let extract_to_kg = params.extract_to_kg.unwrap_or(false)
            || (auto_extract && params.content.to_lowercase().contains("prep for comp"));

        let session_hours = params.session_hours.unwrap_or(6.0) as f64;
        let dry_run = params.dry_run.unwrap_or(false);
        let confidence_min = params.confidence_min.unwrap_or(0.6) as f64;
        let max_nodes = params.max_nodes.unwrap_or(30) as usize;
        let max_edges = params.max_edges.unwrap_or(60) as usize;

        // Insert into SurrealDB and return plain string id
        let created_raw: Vec<serde_json::Value> = self
            .db
            .query("CREATE thoughts SET content = $content, created_at = time::now(), embedding = $embedding, injected_memories = [], enriched_content = NONE, injection_scale = $injection_scale, significance = $significance, access_count = 0, last_accessed = NONE, submode = NONE, framework_enhanced = NONE, framework_analysis = NONE, is_inner_voice = true, inner_visibility = $inner_visibility RETURN meta::id(id) as id;")
            .bind(("content", params.content.clone()))
            .bind(("embedding", embedding.clone()))
            .bind(("injection_scale", injection_scale))
            .bind(("significance", significance))
            .bind(("inner_visibility", inner_visibility.clone()))
            .await?
            .take(0)?;

        let thought_id = created_raw
            .first()
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Initialize result with basic data
        let mut result = json!({
            "thought_id": thought_id,
            "inner_voice": true,
            "visibility": inner_visibility,
            "memories_injected": 0
        });

        // KG Extraction Flow
        if extract_to_kg {
            tracing::info!("KG extraction triggered for inner_voice");

            // Query recent session thoughts
            let recent_query = format!(
                "SELECT meta::id(id) as id, content, created_at FROM thoughts WHERE created_at > time::now() - {}h AND is_inner_voice = true ORDER BY created_at ASC LIMIT 100",
                session_hours
            );

            let recent_raw: Vec<serde_json::Value> = self.db.query(recent_query).await?.take(0)?;
            let recent_texts: Vec<String> = recent_raw
                .iter()
                .filter_map(|r| r.get("content").and_then(|c| c.as_str()))
                .map(|s| s.to_string())
                .collect();

            let session_total = recent_texts.len();
            tracing::info!("🎯 KG extraction starting: {} session thoughts, confidence_min={:.2}, max_nodes={}, max_edges={}",
                         session_total, confidence_min, max_nodes, max_edges);

            if !recent_texts.is_empty() {
                // Extract knowledge
                let extractor = HeuristicExtractor::new();
                let extraction = extractor.extract(&recent_texts).await?;

                // Debug: Show what relationships were extracted
                tracing::info!("🔍 Raw relationships extracted: {}", extraction.relationships.len());
                for (idx, rel) in extraction.relationships.iter().enumerate() {
                    tracing::debug!("  [{}] '{}' -> '{}' [{}] (confidence: {:.2})",
                                  idx + 1, rel.source_name, rel.target_name, rel.rel_type, rel.confidence);
                }

                // Filter by confidence
                let filtered_entities: Vec<_> = extraction
                    .entities
                    .into_iter()
                    .filter(|e| e.confidence >= confidence_min as f32)
                    .take(max_nodes)
                    .collect();

                let relationships: Vec<_> = extraction
                    .relationships
                    .into_iter()
                    .filter(|r| r.confidence >= confidence_min as f32)
                    .take(max_edges)
                    .collect();

                // Debug: Show relationships after filtering
                tracing::info!("📊 Relationships after confidence filtering: {}", relationships.len());
                for (idx, rel) in relationships.iter().enumerate() {
                    tracing::debug!("  Filtered [{}] '{}' -> '{}' [{}] (confidence: {:.2})",
                                  idx + 1, rel.source_name, rel.target_name, rel.rel_type, rel.confidence);
                }

                if !dry_run {
                    // Upsert entities to KG
                    tracing::debug!("Starting KG extraction: {} entities, {} relationships", filtered_entities.len(), relationships.len());
                    let mut entity_ids = Vec::new();
                    for entity in &filtered_entities {
                        tracing::debug!("Processing entity: {} (type: {})", entity.name, entity.entity_type);
                        let existing: Vec<serde_json::Value> = self.db
                            .query("SELECT meta::id(id) as id FROM kg_entities WHERE name = $name AND data.entity_type = $type LIMIT 1")
                            .bind(("name", entity.name.clone()))
                            .bind(("type", entity.entity_type.clone()))
                            .await?
                            .take(0)?;

                        let entity_id = if existing.is_empty() {
                            // Create new entity
                            tracing::debug!("Creating new entity: {} (type: {})", entity.name, entity.entity_type);
                            let created: Vec<serde_json::Value> = self.db
                                .query("CREATE kg_entities SET created_at = time::now(), name = $name, data = $data RETURN meta::id(id) as id, name, data")
                                .bind(("name", entity.name.clone()))
                                .bind(("data", entity.properties.clone()))
                                .await?
                                .take(0)?;
                            let new_id = created
                                .first()
                                .and_then(|c| c.get("id"))
                                .and_then(|id| id.as_str())
                                .unwrap_or("")
                                .to_string();
                            tracing::debug!("Created entity with ID: {}", new_id);
                            new_id
                        } else {
                            // Use existing
                            let existing_id = existing
                                .first()
                                .and_then(|e| e.get("id"))
                                .and_then(|id| id.as_str())
                                .unwrap_or("")
                                .to_string();
                            tracing::debug!("Using existing entity ID: {}", existing_id);
                            existing_id
                        };
                        entity_ids.push(entity_id);
                    }

                    // Create relationships
                    tracing::info!("🎯 Starting relationship creation for {} relationships", relationships.len());
                    let mut relationship_ids = Vec::new();
                    for (rel_idx, relationship) in relationships.iter().enumerate() {
                        tracing::info!("🔍 Processing relationship {}: '{}' -> '{}' [{}]",
                                     rel_idx + 1, relationship.source_name, relationship.target_name, relationship.rel_type);

                        // Debug: Show all entities available for matching
                        tracing::debug!("📋 Available entities for matching:");
                        for (idx, entity) in filtered_entities.iter().enumerate() {
                            tracing::debug!("  [{}] {} (id: {})", idx, entity.name, entity_ids.get(idx).unwrap_or(&"N/A".to_string()));
                        }

                        // Find entity IDs (case-insensitive matching)
                        let source_match = filtered_entities
                            .iter()
                            .enumerate()
                            .find(|(_, e)| e.name.to_lowercase() == relationship.source_name.to_lowercase());

                        let source_id = if let Some((idx, entity)) = source_match {
                            tracing::debug!("✅ Source match found: '{}' matches entity '{}' at index {}", relationship.source_name, entity.name, idx);
                            entity_ids.get(idx).cloned().unwrap_or_default()
                        } else {
                            tracing::warn!("❌ Source match failed: '{}' not found in entities", relationship.source_name);
                            String::new()
                        };

                        let target_match = filtered_entities
                            .iter()
                            .enumerate()
                            .find(|(_, e)| e.name.to_lowercase() == relationship.target_name.to_lowercase());

                        let target_id = if let Some((idx, entity)) = target_match {
                            tracing::debug!("✅ Target match found: '{}' matches entity '{}' at index {}", relationship.target_name, entity.name, idx);
                            entity_ids.get(idx).cloned().unwrap_or_default()
                        } else {
                            tracing::warn!("❌ Target match failed: '{}' not found in entities", relationship.target_name);
                            String::new()
                        };

                        tracing::debug!("🔗 Relationship IDs: source='{}', target='{}'", source_id, target_id);

                        if !source_id.is_empty() && !target_id.is_empty() {
                            tracing::debug!("Creating relationship in database: {} -> {} [{}]", source_id, target_id, relationship.rel_type);
                            let created: Vec<serde_json::Value> = self.db
                                .query("CREATE kg_edges SET created_at = time::now(), source = $source, target = $target, rel_type = $rel_type, data = $data RETURN meta::id(id) as id, source, target, rel_type")
                                .bind(("source", source_id.clone()))
                                .bind(("target", target_id.clone()))
                                .bind(("rel_type", relationship.rel_type.clone()))
                                .bind(("data", relationship.properties.clone()))
                                .await?
                                .take(0)?;

                            if let Some(created_rel) = created.first() {
                                let rel_id = created_rel
                                    .get("id")
                                    .and_then(|id| id.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                tracing::debug!("Created relationship with ID: {}", rel_id);
                                relationship_ids.push(rel_id);
                            } else {
                                tracing::warn!("No relationship created for: {} -> {} [{}]", source_id, target_id, relationship.rel_type);
                            }
                        } else {
                            tracing::warn!("Skipping relationship creation - empty IDs: source='{}', target='{}'", source_id, target_id);
                        }
                    }

                    tracing::info!("📊 Relationship creation summary: {} attempted, {} created", relationships.len(), relationship_ids.len());

                    // Generate structured entity list for return
                    let entities_return: Vec<serde_json::Value> = filtered_entities
                        .iter()
                        .enumerate()
                        .map(|(idx, entity)| {
                            let id = entity_ids.get(idx).cloned().unwrap_or_default();
                            json!({
                                "id": id,
                                "name": entity.name,
                                "entity_type": entity.entity_type
                            })
                        })
                        .collect();

                    // Generate structured relationship list for return
                    let relationships_return: Vec<serde_json::Value> = relationships
                        .iter()
                        .enumerate()
                        .filter_map(|(idx, _)| {
                            relationship_ids.get(idx).map(|rel_id: &String| {
                                let rel = &relationships[idx];
                                let source_id = filtered_entities
                                    .iter()
                                    .position(|e| e.name == rel.source_name)
                                    .and_then(|idx| entity_ids.get(idx))
                                    .cloned()
                                    .unwrap_or_default();
                                json!({
                                    "id": rel_id.clone(),
                                    "source_id": source_id,
                                    "target_id": filtered_entities.iter()
                                        .position(|e| e.name == rel.target_name)
                                        .and_then(|idx| entity_ids.get(idx))
                                        .cloned()
                                        .unwrap_or_default(),
                                    "rel_type": rel.rel_type
                                })
                            })
                        })
                        .collect();

                    // Add KG extraction results to response
                    result["extracted"] = json!({
                        "handoff": extraction.synthesis,
                        "entities": entities_return,
                        "relationships": relationships_return,
                        "created": {
                            "entities": filtered_entities.len(),
                            "relationships": relationships.len()
                        },
                        "session": {
                            "from": format!("{} hours ago", session_hours),
                            "to": "now",
                            "total_thoughts": session_total
                        }
                    });
                } else {
                    // Dry run - just include summary
                    result["extracted"] = json!({
                        "handoff": format!("DRY RUN: {}", extraction.synthesis),
                        "entities": filtered_entities.len(),
                        "relationships": relationships.len(),
                        "session": {
                            "from": format!("{} hours ago", session_hours),
                            "to": "now",
                            "total_thoughts": session_total
                        }
                    });
                }
            } else {
                result["extracted"] = json!({
                    "handoff": "No recent thoughts found in session window",
                    "entities": [],
                    "relationships": [],
                    "created": {"entities": 0, "relationships": 0},
                    "session": {
                        "from": format!("{} hours ago", session_hours),
                        "to": "now",
                        "total_thoughts": 0
                    }
                });
            }
        }

        Ok(CallToolResult::structured(result))
    }
}
