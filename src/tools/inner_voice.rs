//surreal-mind-inner-voice-refactor/src/tools/inner_voice.rs
//! inner_voice tool handler for RAG retrieval, synthesis, and optional KG staging

use crate::error::{Result, SurrealMindError};
use crate::kg_extractor::HeuristicExtractor;
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;

/// Parameters for the inner_voice tool (RAG)
#[derive(Debug, serde::Deserialize)]
pub struct InnerVoiceParams {
    pub content: String,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_u64_forgiving"
    )]
    pub top_k: Option<u64>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_f32_forgiving"
    )]
    pub sim_thresh: Option<f32>,
    #[serde(default)]
    pub stage_kg: Option<bool>,
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
    #[serde(default)]
    pub save: Option<bool>,
    #[serde(default)]
    pub auto_mark_removal: Option<bool>,
}

impl SurrealMindServer {
    /// Handle the inner_voice tool call (RAG + optional KG staging + save)
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
        let dbg_preview: String = params.content.chars().take(50).collect();
        tracing::debug!("inner_voice content (first 50 chars): {}", dbg_preview);

        // Defaults from env or params
        let top_k = params.top_k.unwrap_or_else(|| {
            std::env::var("SURR_TOP_K")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5)
        }) as usize;
        let sim_thresh = params.sim_thresh.unwrap_or_else(|| {
            std::env::var("SURR_SIM_THRESH")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.3) // Lowered from 0.5 for better retrieval
        });
        let stage_kg = params.stage_kg.unwrap_or(false);
        let confidence_min = params.confidence_min.unwrap_or(0.6);
        let max_nodes = params.max_nodes.unwrap_or(30) as usize;
        let max_edges = params.max_edges.unwrap_or(60) as usize;
        let save = params.save.unwrap_or(true);
        let auto_mark_removal = params.auto_mark_removal.unwrap_or(false);

        // Embed query
        let query_embedding = self.embedder.embed(&params.content).await.map_err(|e| {
            SurrealMindError::Embedding {
                message: e.to_string(),
            }
        })?;

        // Retrieve top thoughts with similarity
        let limit: usize = std::env::var("SURR_DB_LIMIT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000); // Increased from 500 to ensure more candidates
        let retrieved: Vec<serde_json::Value> = self
            .db
            .query("SELECT meta::id(id) as id, content, embedding FROM thoughts LIMIT $limit")
            .bind(("limit", limit as i64))
            .await?
            .take(0)?;

        tracing::debug!("Retrieved {} thoughts for inner_voice RAG", retrieved.len());

        let mut scored_sources: Vec<(String, f32, String)> = vec![];
        for row in &retrieved {
            if let (Some(id), Some(content), Some(emb_arr)) = (
                row.get("id").and_then(|v| v.as_str()),
                row.get("content").and_then(|v| v.as_str()),
                row.get("embedding").and_then(|v| v.as_array()),
            ) {
                let emb: Vec<f32> = emb_arr
                    .iter()
                    .filter_map(|v| v.as_f64())
                    .map(|f| f as f32)
                    .collect();
                if emb.len() == query_embedding.len() {
                    let sim = SurrealMindServer::cosine_similarity(&query_embedding, &emb);
                    if sim >= sim_thresh {
                        let excerpt = content.chars().take(200).collect();
                        scored_sources.push((id.to_string(), sim, excerpt));
                    }
                }
            }
        }
        // Sort by similarity desc, take top_k
        scored_sources.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored_sources.truncate(top_k);

        // Synthesize answer: simple extractive summary from top 3 sources
        let synthesized_answer = if !scored_sources.is_empty() {
            let mut synthesis = String::new();
            for (i, (_id, _sim, excerpt)) in scored_sources.iter().enumerate().take(3) {
                if i > 0 {
                    synthesis.push('\n');
                }
                synthesis.push_str(excerpt);
            }
            synthesis.chars().take(600).collect()
        } else {
            "No relevant thoughts found.".to_string()
        };

        let source_ids: Vec<String> = scored_sources.iter().map(|(id, _, _)| id.clone()).collect();

        // Save synthesized answer as summary thought if enabled
        let mut saved_thought_id: Option<String> = None;
        if save {
            // Get embedding metadata for tracking
            let (provider, model, dim) = self.get_embedding_metadata();

            let created_raw: Vec<serde_json::Value> = self
                .db
                .query("CREATE thoughts SET content = $synth, created_at = time::now(), embedding = $embedding, injected_memories = [], enriched_content = NONE, injection_scale = 0, significance = 0.5, access_count = 0, last_accessed = NONE, submode = NONE, framework_enhanced = NONE, framework_analysis = NONE, is_summary = true, summary_of = $source_ids, pipeline = 'inner_voice', status = 'active', embedding_provider = $provider, embedding_model = $model, embedding_dim = $dim, embedded_at = time::now() RETURN meta::id(id) as id;")
                .bind(("synth", synthesized_answer.clone()))
                .bind(("embedding", query_embedding.clone()))
                .bind(("source_ids", source_ids.clone()))
                .bind(("provider", provider))
                .bind(("model", model))
                .bind(("dim", dim))
                .await?
                .take(0)?;
            saved_thought_id = created_raw
                .first()
                .and_then(|v| v.get("id"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }

        // If staging KG, run extractor on retrieved sources
        let mut pending_entities = 0;
        let mut pending_relationships = 0;
        let mut marked_for_removal = 0;
        if stage_kg && !scored_sources.is_empty() {
            let source_texts: Vec<String> = scored_sources
                .iter()
                .map(|(_, _, excerpt)| excerpt.clone())
                .collect();
            let extractor = HeuristicExtractor::new();
            let extraction = extractor.extract(&source_texts).await?;

            // Stage entities
            for entity in extraction
                .entities
                .into_iter()
                .filter(|e| e.confidence >= confidence_min)
                .take(max_nodes)
            {
                self.db.query("CREATE kg_entity_candidates SET name = $name, entity_type = $etype, data = $data, confidence = $conf, source_thought_id = $sid")
                    .bind(("name", entity.name))
                    .bind(("etype", entity.entity_type))
                    .bind(("data", entity.properties))
                    .bind(("conf", entity.confidence as f64))
                    .bind(("sid", source_ids.join(",")))
                    .await?;
                pending_entities += 1;
            }

            // Stage relationships
            for rel in extraction
                .relationships
                .into_iter()
                .filter(|r| r.confidence >= confidence_min)
                .take(max_edges)
            {
                self.db.query("CREATE kg_edge_candidates SET source_name = $src, target_name = $tgt, rel_type = $rtype, data = $data, confidence = $conf, source_thought_id = $sid")
                    .bind(("src", rel.source_name))
                    .bind(("tgt", rel.target_name))
                    .bind(("rtype", rel.rel_type))
                    .bind(("data", rel.properties))
                    .bind(("conf", rel.confidence as f64))
                    .bind(("sid", source_ids.join(",")))
                    .await?;
                pending_relationships += 1;
            }

            // Auto-mark for removal if enabled
            if auto_mark_removal && !source_ids.is_empty() {
                self.db
                    .query("UPDATE thoughts SET status = 'removal' WHERE id IN $ids")
                    .bind(("ids", source_ids.clone()))
                    .await?;
                marked_for_removal = source_ids.len();
            }
        }

        // Return result
        let result = json!({
            "synthesized_answer": synthesized_answer,
            "saved_thought_id": saved_thought_id,
            "sources": scored_sources.into_iter().map(|(id, sim, excerpt)| json!({
                "thought_id": id,
                "similarity": sim,
                "excerpt": excerpt
            })).collect::<Vec<_>>(),
            "staged": {
                "pending_entities": pending_entities,
                "pending_relationships": pending_relationships
            },
            "marked_for_removal": marked_for_removal
        });

        Ok(CallToolResult::structured(result))
    }
}
