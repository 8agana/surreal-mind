use crate::server::SurrealMindServer;
use crate::tools::memories_populate::{ExtractedMemory, MemoriesPopulateRequest};
use chrono::Utc;
use rmcp::{
    ErrorData as McpError,
    handler::server::ServerHandler,
    model::{
        Annotated, CallToolRequestParam, CallToolResult, Implementation, InitializeRequestParam,
        InitializeResult, ListToolsResult, PaginatedRequestParam, ProtocolVersion, RawContent,
        ServerCapabilities, ServerInfo, ToolsCapability,
    },
    service::{RequestContext, RoleServer},
};
use serde_json::json;
use tracing::info;
use uuid::Uuid;

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
                title: Some("Surreal Mind".to_string()),
                version: "0.1.0".to_string(),
                website_url: Some("https://github.com/8agana/surreal-mind".to_string()),
                icons: None,
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

        // Input schemas
        let legacymind_think_schema_map = crate::schemas::legacymind_think_schema();
        let maintenance_ops_schema_map = crate::schemas::maintenance_ops_schema();
        let kg_create_schema_map = crate::schemas::kg_create_schema();
        let kg_moderate_schema_map = crate::schemas::kg_moderate_schema();
        let detailed_help_schema_map = crate::schemas::detailed_help_schema();
        let inner_voice_schema_map = crate::schemas::inner_voice_schema();
        let unified_schema = crate::schemas::unified_search_schema();
        let curiosity_add_schema = crate::schemas::curiosity_add_schema();
        let curiosity_get_schema = crate::schemas::curiosity_get_schema();
        let curiosity_search_schema = crate::schemas::curiosity_search_schema();
        let memories_populate_schema_map = crate::schemas::memories_populate_schema();

        // Output schemas (rmcp 0.11.0+)
        let legacymind_think_output = crate::schemas::legacymind_think_output_schema();
        let maintenance_ops_output = crate::schemas::maintenance_ops_output_schema();
        let memories_create_output = crate::schemas::memories_create_output_schema();
        let memories_moderate_output = crate::schemas::memories_moderate_output_schema();
        let detailed_help_output = crate::schemas::detailed_help_output_schema();
        let inner_voice_output = crate::schemas::inner_voice_output_schema();
        let unified_search_output = crate::schemas::legacymind_search_output_schema();
        let memories_populate_output = crate::schemas::memories_populate_output_schema();

        let mut tools = vec![
            Tool {
                name: "legacymind_think".into(),
                title: Some("LegacyMind Think".into()),
                description: Some("Unified thinking tool with automatic mode routing".into()),
                input_schema: legacymind_think_schema_map.clone(),
                icons: None,
                annotations: None,
                output_schema: Some(legacymind_think_output),
                meta: None,
            },
            Tool {
                name: "maintenance_ops".into(),
                title: Some("Maintenance Operations".into()),
                description: Some("Maintenance operations for archival and cleanup".into()),
                input_schema: maintenance_ops_schema_map,
                icons: None,
                annotations: None,
                output_schema: Some(maintenance_ops_output),
                meta: None,
            },
            // (legacy think_search removed — use legacymind_search)
            Tool {
                name: "memories_create".into(),
                title: Some("Create Memories".into()),
                description: Some(
                    "Create entities and relationships in personal memory graph".into(),
                ),
                input_schema: kg_create_schema_map,
                icons: None,
                annotations: None,
                output_schema: Some(memories_create_output),
                meta: None,
            },
            // (legacy memories_search removed — use legacymind_search)
            Tool {
                name: "memories_moderate".into(),
                title: Some("Moderate Memories".into()),
                description: Some("Review and/or decide on memory graph candidates".into()),
                input_schema: kg_moderate_schema_map.clone(),
                icons: None,
                annotations: None,
                output_schema: Some(memories_moderate_output),
                meta: None,
            },
            Tool {
                name: "detailed_help".into(),
                title: Some("Detailed Help".into()),
                description: Some("Get detailed help for a specific tool".into()),
                input_schema: detailed_help_schema_map,
                icons: None,
                annotations: None,
                output_schema: Some(detailed_help_output),
                meta: None,
            },
        ];

        // Always list the tool (visibility), enforce gating inside the handler if disabled
        tools.push(Tool {
            name: "inner_voice".into(),
            title: Some("Inner Voice".into()),
            description: Some(
                "Retrieves and synthesizes relevant memories/thoughts into a concise answer; can optionally auto-extract entities/relationships into staged knowledge‑graph candidates for review.".into(),
            ),
            input_schema: inner_voice_schema_map.clone(),
            icons: None,
            annotations: None,
            output_schema: Some(inner_voice_output),
            meta: None,
        });

        tools.push(Tool {
            name: "curiosity_add".into(),
            title: Some("Curiosity Add".into()),
            description: Some("Add a curiosity entry (lightweight note) with optional tags/agent/topic/in_reply_to.".into()),
            input_schema: curiosity_add_schema.clone(),
            icons: None,
            annotations: None,
            output_schema: None,
            meta: None,
        });
        tools.push(Tool {
            name: "curiosity_get".into(),
            title: Some("Curiosity Get".into()),
            description: Some("Fetch recent curiosity entries (limit/since).".into()),
            input_schema: curiosity_get_schema.clone(),
            icons: None,
            annotations: None,
            output_schema: None,
            meta: None,
        });
        tools.push(Tool {
            name: "curiosity_search".into(),
            title: Some("Curiosity Search".into()),
            description: Some(
                "Search curiosity entries via embeddings with optional recency filter.".into(),
            ),
            input_schema: curiosity_search_schema.clone(),
            icons: None,
            annotations: None,
            output_schema: None,
            meta: None,
        });

        tools.push(Tool {
            name: "legacymind_search".into(),
            title: Some("LegacyMind Search".into()),
            description: Some(
                "Unified LegacyMind search: memories (default) + optional thoughts".into(),
            ),
            input_schema: unified_schema,
            icons: None,
            annotations: None,
            output_schema: Some(unified_search_output),
            meta: None,
        });
        tools.push(Tool {
            name: "memories_populate".into(),
            title: Some("Populate Memories".into()),
            description: Some(
                "Process unextracted thoughts with Gemini CLI to populate knowledge graph".into(),
            ),
            input_schema: memories_populate_schema_map,
            icons: None,
            annotations: None,
            output_schema: Some(memories_populate_output),
            meta: None,
        });
        // (photography tools removed from this server)

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
            // Unified thinking tool
            "legacymind_think" => self
                .handle_legacymind_think(request)
                .await
                .map_err(|e| e.into()),

            // Intelligence and utility
            "maintenance_ops" => self
                .handle_maintenance_ops(request)
                .await
                .map_err(|e| e.into()),
            // Memory tools
            "memories_create" => self
                .handle_knowledgegraph_create(request)
                .await
                .map_err(|e| e.into()),
            "memories_moderate" => self
                .handle_knowledgegraph_moderate(request)
                .await
                .map_err(|e| e.into()),
            // Inner voice retrieval
            // New canonical name
            "inner_voice" => self
                .handle_inner_voice_retrieve(request)
                .await
                .map_err(|e| e.into()),
            "curiosity_add" => self
                .handle_curiosity_add(request)
                .await
                .map_err(|e| e.into()),
            "curiosity_get" => self
                .handle_curiosity_get(request)
                .await
                .map_err(|e| e.into()),
            "curiosity_search" => self
                .handle_curiosity_search(request)
                .await
                .map_err(|e| e.into()),

            // Help
            "detailed_help" => self
                .handle_detailed_help(request)
                .await
                .map_err(|e| e.into()),
            "legacymind_search" => self
                .handle_unified_search(request)
                .await
                .map_err(|e| e.into()),
            "memories_populate" => self.handle_memories_populate(request).await,
            _ => Err(McpError {
                code: rmcp::model::ErrorCode::METHOD_NOT_FOUND,
                message: format!("Unknown tool: {}", request.name).into(),
                data: None,
            }),
        }
    }
}

impl SurrealMindServer {
    async fn handle_memories_populate(
        &self,
        request: CallToolRequestParam,
    ) -> std::result::Result<CallToolResult, McpError> {
        let params: MemoriesPopulateRequest = serde_json::from_value(serde_json::Value::Object(
            request.arguments.unwrap_or_default(),
        ))
        .map_err(|e| McpError {
            code: rmcp::model::ErrorCode::INVALID_PARAMS,
            message: format!("Invalid parameters: {}", e).into(),
            data: None,
        })?;

        // Generate batch ID
        let extraction_batch_id = Uuid::new_v4().to_string();

        // Fetch unprocessed thoughts
        tracing::info!(
            "memories_populate: fetching thoughts with params: source={}, limit={}",
            params.source,
            params.limit
        );
        let thoughts = match self.fetch_thoughts_for_extraction(&params).await {
            Ok(t) => t,
            Err(e) => {
                return Ok(CallToolResult {
                    content: vec![Annotated::new(
                        RawContent::text(
                            json!({
                                "thoughts_processed": 0,
                                "entities_extracted": 0,
                                "relationships_extracted": 0,
                                "observations_extracted": 0,
                                "boundaries_extracted": 0,
                                "staged_for_review": 0,
                                "auto_approved": 0,
                                "extraction_batch_id": "",
                                "gemini_session_id": "",
                                "error": e.to_string()
                            })
                            .to_string(),
                        ),
                        None,
                    )],
                    is_error: Some(false),
                    meta: None,
                    structured_content: None,
                });
            }
        };
        tracing::info!(
            "memories_populate: found {} thoughts to process",
            thoughts.len()
        );
        if thoughts.is_empty() {
            // Return schema-conformant response even when no work to do
            let response_value = json!({
                "thoughts_processed": 0,
                "entities_extracted": 0,
                "relationships_extracted": 0,
                "observations_extracted": 0,
                "boundaries_extracted": 0,
                "staged_for_review": 0,
                "auto_approved": 0,
                "extraction_batch_id": "",
                "gemini_session_id": ""
            });
            return Ok(CallToolResult {
                content: vec![Annotated::new(
                    RawContent::text(response_value.to_string()),
                    None,
                )],
                is_error: Some(false),
                meta: None,
                structured_content: None,
            });
        }

        // Prepare prompt
        let thoughts_text = thoughts
            .iter()
            .map(|t| format!("ID: {}\nContent: {}", t.id, t.content))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");
        let prompt = format!(
            r#"
You are extracting knowledge graph entries from a collection of thoughts.

For each thought, identify:

1. **Entities** - People, projects, concepts, tools, systems
   - Format: {{ "name": "...", "type": "person|project|concept|tool|system", "description": "..." }}

2. **Relationships** - How entities connect
   - Format: {{ "from": "entity_name", "to": "entity_name", "relation": "...", "description": "..." }}

3. **Observations** - Insights, patterns, lessons learned
   - Format: {{ "content": "...", "context": "...", "tags": [...] }}

4. **Boundaries** - Things explicitly rejected or avoided (who_i_choose_not_to_be)
   - Format: {{ "rejected": "...", "reason": "...", "context": "..." }}

For each extraction, provide a confidence score (0.0-1.0).

Return JSON:
{{
  "entities": [...],
  "relationships": [...],
  "observations": [...],
  "boundaries": [...],
  "summary": "Brief summary of what was extracted"
}}

THOUGHTS TO PROCESS:
---
{}
---
"#,
            thoughts_text
        );

        // Initialize Gemini client
        let gemini = crate::gemini::GeminiClient::new();
        let tool_name = "memories_populate";

        // Try to resume session
        let session_id: Option<String> = if let Some(inherit_from) = &params.inherit_session_from {
            match crate::sessions::get_tool_session(&self.db, inherit_from.clone()).await {
                Ok(sid) => sid,
                Err(e) => {
                    return Ok(CallToolResult {
                        content: vec![Annotated::new(
                            RawContent::text(
                                json!({
                                    "thoughts_processed": 0,
                                    "entities_extracted": 0,
                                    "relationships_extracted": 0,
                                    "observations_extracted": 0,
                                    "boundaries_extracted": 0,
                                    "staged_for_review": 0,
                                    "auto_approved": 0,
                                    "extraction_batch_id": "",
                                    "gemini_session_id": "",
                                    "error": format!("DB error: {}", e)
                                })
                                .to_string(),
                            ),
                            None,
                        )],
                        is_error: Some(false),
                        meta: None,
                        structured_content: None,
                    });
                }
            }
        } else {
            match crate::sessions::get_tool_session(&self.db, tool_name.to_string()).await {
                Ok(sid) => sid,
                Err(e) => {
                    return Ok(CallToolResult {
                        content: vec![Annotated::new(
                            RawContent::text(
                                json!({
                                    "thoughts_processed": 0,
                                    "entities_extracted": 0,
                                    "relationships_extracted": 0,
                                    "observations_extracted": 0,
                                    "boundaries_extracted": 0,
                                    "staged_for_review": 0,
                                    "auto_approved": 0,
                                    "extraction_batch_id": "",
                                    "gemini_session_id": "",
                                    "error": format!("DB error: {}", e)
                                })
                                .to_string(),
                            ),
                            None,
                        )],
                        is_error: Some(false),
                        meta: None,
                        structured_content: None,
                    });
                }
            }
        };

        let gemini_response = match gemini.call(&prompt, session_id.as_deref()).await {
            Ok(resp) => {
                // Store new session
                match crate::sessions::store_tool_session(
                    &self.db,
                    tool_name.to_string(),
                    resp.session_id.clone(),
                )
                .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        return Ok(CallToolResult {
                            content: vec![Annotated::new(
                                RawContent::text(
                                    json!({
                                        "thoughts_processed": 0,
                                        "entities_extracted": 0,
                                        "relationships_extracted": 0,
                                        "observations_extracted": 0,
                                        "boundaries_extracted": 0,
                                        "staged_for_review": 0,
                                        "auto_approved": 0,
                                        "extraction_batch_id": "",
                                        "gemini_session_id": "",
                                        "error": format!("DB error: {}", e)
                                    })
                                    .to_string(),
                                ),
                                None,
                            )],
                            is_error: Some(false),
                            meta: None,
                            structured_content: None,
                        });
                    }
                }
                resp
            }
            Err(e) if session_id.is_some() => {
                // Reset on failure
                match crate::sessions::clear_tool_session(&self.db, tool_name.to_string()).await {
                    Ok(_) => {}
                    Err(db_err) => {
                        return Ok(CallToolResult {
                            content: vec![Annotated::new(
                                RawContent::text(
                                    json!({
                                        "thoughts_processed": 0,
                                        "entities_extracted": 0,
                                        "relationships_extracted": 0,
                                        "observations_extracted": 0,
                                        "boundaries_extracted": 0,
                                        "staged_for_review": 0,
                                        "auto_approved": 0,
                                        "extraction_batch_id": "",
                                        "gemini_session_id": "",
                                        "error": format!("DB error clearing session: {}", db_err)
                                    })
                                    .to_string(),
                                ),
                                None,
                            )],
                            is_error: Some(false),
                            meta: None,
                            structured_content: None,
                        });
                    }
                }
                let resp = match gemini.call(&prompt, None).await {
                    Ok(resp) => resp,
                    Err(gem_err) => {
                        return Ok(CallToolResult {
                            content: vec![Annotated::new(
                                RawContent::text(json!({
                                    "thoughts_processed": 0,
                                    "entities_extracted": 0,
                                    "relationships_extracted": 0,
                                    "observations_extracted": 0,
                                    "boundaries_extracted": 0,
                                    "staged_for_review": 0,
                                    "auto_approved": 0,
                                    "extraction_batch_id": "",
                                    "gemini_session_id": "",
                                    "error": format!("Gemini error after session reset: {}", gem_err)
                                }).to_string()),
                                None,
                            )],
                            is_error: Some(false),
                            meta: None,
                            structured_content: None,
                        });
                    }
                };
                // Store session after successful retry
                match crate::sessions::store_tool_session(
                    &self.db,
                    tool_name.to_string(),
                    resp.session_id.clone(),
                )
                .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        return Ok(CallToolResult {
                            content: vec![Annotated::new(
                                RawContent::text(
                                    json!({
                                        "thoughts_processed": 0,
                                        "entities_extracted": 0,
                                        "relationships_extracted": 0,
                                        "observations_extracted": 0,
                                        "boundaries_extracted": 0,
                                        "staged_for_review": 0,
                                        "auto_approved": 0,
                                        "extraction_batch_id": "",
                                        "gemini_session_id": "",
                                        "error": format!("DB error storing retry session: {}", e)
                                    })
                                    .to_string(),
                                ),
                                None,
                            )],
                            is_error: Some(false),
                            meta: None,
                            structured_content: None,
                        });
                    }
                }
                resp
            }
            Err(e) => {
                return Ok(CallToolResult {
                    content: vec![Annotated::new(
                        RawContent::text(
                            json!({
                                "thoughts_processed": 0,
                                "entities_extracted": 0,
                                "relationships_extracted": 0,
                                "observations_extracted": 0,
                                "boundaries_extracted": 0,
                                "staged_for_review": 0,
                                "auto_approved": 0,
                                "extraction_batch_id": "",
                                "gemini_session_id": "",
                                "error": format!("Gemini error: {}", e)
                            })
                            .to_string(),
                        ),
                        None,
                    )],
                    is_error: Some(false),
                    meta: None,
                    structured_content: None,
                });
            }
        };

        // Parse extraction results
        let extraction: serde_json::Value = match serde_json::from_str(&gemini_response.response) {
            Ok(val) => val,
            Err(e) => {
                return Ok(CallToolResult {
                    content: vec![Annotated::new(
                        RawContent::text(
                            json!({
                                "thoughts_processed": 0,
                                "entities_extracted": 0,
                                "relationships_extracted": 0,
                                "observations_extracted": 0,
                                "boundaries_extracted": 0,
                                "staged_for_review": 0,
                                "auto_approved": 0,
                                "extraction_batch_id": "",
                                "gemini_session_id": gemini_response.session_id,
                                "error": format!("Failed to parse Gemini response: {}", e)
                            })
                            .to_string(),
                        ),
                        None,
                    )],
                    is_error: Some(false),
                    meta: None,
                    structured_content: None,
                });
            }
        };

        // Process extractions (simplified counts)
        let extracted_at = Utc::now().to_rfc3339();
        let mut entities_extracted = 0;
        let relationships_extracted = 0;
        let observations_extracted = 0;
        let boundaries_extracted = 0;
        let mut staged_for_review = 0;
        let mut auto_approved = 0;

        // Entities processing
        if let Some(entities) = extraction.get("entities").and_then(|v| v.as_array()) {
            for entity in entities {
                if let Some(confidence) = entity.get("confidence").and_then(|c| c.as_f64()) {
                    let confidence = confidence as f32;
                    let memory = ExtractedMemory {
                        kind: "entity".to_string(),
                        data: entity.clone(),
                        confidence,
                        source_thought_ids: thoughts.iter().map(|t| t.id.clone()).collect(),
                        extraction_batch_id: extraction_batch_id.clone(),
                        extracted_at: extracted_at.clone(),
                        extraction_prompt_version: "extraction_v1".to_string(),
                    };

                    if params.auto_approve && confidence >= params.confidence_threshold {
                        if let Err(e) = self.create_memory(&memory, "kg_entities").await {
                            return Ok(CallToolResult {
                                content: vec![Annotated::new(
                                    RawContent::text(
                                        json!({
                                            "thoughts_processed": thoughts.len() as u32,
                                            "entities_extracted": entities_extracted,
                                            "relationships_extracted": relationships_extracted,
                                            "observations_extracted": observations_extracted,
                                            "boundaries_extracted": boundaries_extracted,
                                            "staged_for_review": staged_for_review,
                                            "auto_approved": auto_approved,
                                            "extraction_batch_id": extraction_batch_id,
                                            "gemini_session_id": gemini_response.session_id,
                                            "error": format!("Memory creation failed: {}", e)
                                        })
                                        .to_string(),
                                    ),
                                    None,
                                )],
                                is_error: Some(false),
                                meta: None,
                                structured_content: None,
                            });
                        }
                        auto_approved += 1;
                    } else {
                        if let Err(e) = self.stage_memory_for_review(&memory).await {
                            return Ok(CallToolResult {
                                content: vec![Annotated::new(
                                    RawContent::text(
                                        json!({
                                            "thoughts_processed": thoughts.len() as u32,
                                            "entities_extracted": entities_extracted,
                                            "relationships_extracted": relationships_extracted,
                                            "observations_extracted": observations_extracted,
                                            "boundaries_extracted": boundaries_extracted,
                                            "staged_for_review": staged_for_review,
                                            "auto_approved": auto_approved,
                                            "extraction_batch_id": extraction_batch_id,
                                            "gemini_session_id": gemini_response.session_id,
                                            "error": format!("Staging failed: {}", e)
                                        })
                                        .to_string(),
                                    ),
                                    None,
                                )],
                                is_error: Some(false),
                                meta: None,
                                structured_content: None,
                            });
                        }
                        staged_for_review += 1;
                    }
                    entities_extracted += 1;
                }
            }
        }

        // Mark thoughts as processed
        for thought in &thoughts {
            let sql = r#"
                UPDATE thoughts SET
                    extracted_to_kg = true,
                    extraction_batch_id = $batch_id
                WHERE id = $id
            "#;
            let _ = self
                .db
                .query(sql)
                .bind(("batch_id", extraction_batch_id.clone()))
                .bind(("id", thought.id.clone()))
                .await;
        }

        let response_value = json!({
            "thoughts_processed": thoughts.len() as u32,
            "entities_extracted": entities_extracted,
            "relationships_extracted": relationships_extracted,
            "observations_extracted": observations_extracted,
            "boundaries_extracted": boundaries_extracted,
            "staged_for_review": staged_for_review,
            "auto_approved": auto_approved,
            "extraction_batch_id": extraction_batch_id,
            "gemini_session_id": gemini_response.session_id,
        });

        // Use manual text content to avoid serialization issues with structured content
        Ok(CallToolResult {
            content: vec![Annotated::new(
                RawContent::text(response_value.to_string()),
                None,
            )],
            is_error: Some(false),
            meta: None,
            structured_content: None,
        })
    }

    async fn fetch_thoughts_for_extraction(
        &self,
        params: &MemoriesPopulateRequest,
    ) -> std::result::Result<Vec<crate::server::Thought>, McpError> {
        let sql = match params.source.as_str() {
            "unprocessed" => {
                r#"
                SELECT
                    string::concat(meta::id(id))               AS id,
                    content,
                    IF created_at != NONE THEN <string> created_at ELSE null END AS created_at,
                    embedding ?? []            AS embedding,
                    injected_memories ?? []   AS injected_memories,
                    injection_scale ?? 0      AS injection_scale,
                    significance ?? 0.0       AS significance,
                    access_count ?? 0         AS access_count,
                    IF last_accessed != NONE THEN <string> last_accessed ELSE null END AS last_accessed,
                    submode,
                    framework_enhanced ?? false AS framework_enhanced,
                    framework_analysis ?? {}  AS framework_analysis,
                    embedding_model,
                    embedding_provider,
                    embedding_dim,
                    IF embedded_at != NONE THEN <string> embedded_at ELSE null END AS embedded_at,
                    extracted_to_kg ?? false  AS extracted_to_kg,
                    IF extraction_batch_id != NONE THEN <string> extraction_batch_id ELSE null END AS extraction_batch_id
                FROM thoughts
                WHERE extracted_to_kg = false
                ORDER BY created_at ASC
                LIMIT $limit
            "#
            }
            "chain_id" => {
                r#"
                SELECT
                    string::concat(meta::id(id))               AS id,
                    content,
                    IF created_at != NONE THEN <string> created_at ELSE null END AS created_at,
                    embedding ?? []            AS embedding,
                    injected_memories ?? []   AS injected_memories,
                    injection_scale ?? 0      AS injection_scale,
                    significance ?? 0.0       AS significance,
                    access_count ?? 0         AS access_count,
                    IF last_accessed != NONE THEN <string> last_accessed ELSE null END AS last_accessed,
                    submode,
                    framework_enhanced ?? false AS framework_enhanced,
                    framework_analysis ?? {}  AS framework_analysis,
                    embedding_model,
                    embedding_provider,
                    embedding_dim,
                    IF embedded_at != NONE THEN <string> embedded_at ELSE null END AS embedded_at,
                    extracted_to_kg ?? false  AS extracted_to_kg,
                    IF extraction_batch_id != NONE THEN <string> extraction_batch_id ELSE null END AS extraction_batch_id
                FROM thoughts
                WHERE chain_id = $chain_id AND extracted_to_kg = false
                ORDER BY created_at ASC
                LIMIT $limit
            "#
            }
            "date_range" => {
                r#"
                SELECT
                    string::concat(meta::id(id))               AS id,
                    content,
                    IF created_at != NONE THEN <string> created_at ELSE null END AS created_at,
                    embedding ?? []            AS embedding,
                    injected_memories ?? []   AS injected_memories,
                    injection_scale ?? 0      AS injection_scale,
                    significance ?? 0.0       AS significance,
                    access_count ?? 0         AS access_count,
                    IF last_accessed != NONE THEN <string> last_accessed ELSE null END AS last_accessed,
                    submode,
                    framework_enhanced ?? false AS framework_enhanced,
                    framework_analysis ?? {}  AS framework_analysis,
                    embedding_model,
                    embedding_provider,
                    embedding_dim,
                    IF embedded_at != NONE THEN <string> embedded_at ELSE null END AS embedded_at,
                    extracted_to_kg ?? false  AS extracted_to_kg,
                    IF extraction_batch_id != NONE THEN <string> extraction_batch_id ELSE null END AS extraction_batch_id
                FROM thoughts
                WHERE created_at >= $since AND created_at <= $until AND extracted_to_kg = false
                ORDER BY created_at ASC
                LIMIT $limit
            "#
            }
            _ => {
                return Err(McpError {
                    code: rmcp::model::ErrorCode::INVALID_PARAMS,
                    message: "Invalid source".into(),
                    data: None,
                });
            }
        };

        let mut query = self.db.query(sql).bind(("limit", params.limit as i64));
        if params.source == "chain_id" {
            query = query.bind(("chain_id", params.chain_id.clone().unwrap_or_default()));
        } else if params.source == "date_range" {
            query = query
                .bind(("since", params.since.clone().unwrap_or_default()))
                .bind(("until", params.until.clone().unwrap_or_default()));
        }

        let mut result = query.await.map_err(|e| McpError {
            code: rmcp::model::ErrorCode::INTERNAL_ERROR,
            message: format!("DB query failed: {}", e).into(),
            data: None,
        })?;

        // Take raw rows as JSON for optional debugging before deserialization
        let raw_rows: Vec<serde_json::Value> = result.take(0).map_err(|e| {
            tracing::error!("Failed to fetch raw thoughts: {}", e);
            McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Result parsing failed: {}", e).into(),
                data: None,
            }
        })?;

        if std::env::var("SURR_DEBUG_MEMORIES_POPULATE_ROWS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            if let Some(sample) = raw_rows.first() {
                tracing::warn!("memories_populate sample row: {}", sample);
            } else {
                tracing::warn!("memories_populate: no rows returned");
            }
        }

        fn sanitize_none(value: serde_json::Value) -> serde_json::Value {
            match value {
                serde_json::Value::Object(mut map) => {
                    if map.len() == 1 && map.contains_key("None") {
                        serde_json::Value::Null
                    } else {
                        for v in map.values_mut() {
                            *v = sanitize_none(v.take());
                        }
                        serde_json::Value::Object(map)
                    }
                }
                serde_json::Value::Array(arr) => {
                    serde_json::Value::Array(arr.into_iter().map(sanitize_none).collect())
                }
                other => other,
            }
        }

        let thoughts: Vec<crate::server::Thought> = raw_rows
            .into_iter()
            .map(|v| {
                let cleaned = sanitize_none(v.clone());
                serde_json::from_value(cleaned.clone()).map_err(|e| {
                    tracing::error!("Failed to deserialize thought row {}: {}", cleaned, e);
                    McpError {
                        code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                        message: format!("Result parsing failed: {}", e).into(),
                        data: None,
                    }
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(thoughts)
    }

    async fn create_memory(
        &self,
        memory: &ExtractedMemory,
        table: &str,
    ) -> std::result::Result<(), McpError> {
        let sql = format!(
            r#"
            CREATE {} SET
                data = $data,
                confidence = $confidence,
                source_thought_ids = $source_thought_ids,
                extraction_batch_id = $extraction_batch_id,
                extracted_at = $extracted_at,
                extraction_prompt_version = $extraction_prompt_version
        "#,
            table
        );

        self.db
            .query(&sql)
            .bind(("data", memory.data.clone()))
            .bind(("confidence", memory.confidence))
            .bind(("source_thought_ids", memory.source_thought_ids.clone()))
            .bind(("extraction_batch_id", memory.extraction_batch_id.clone()))
            .bind(("extracted_at", memory.extracted_at.clone()))
            .bind((
                "extraction_prompt_version",
                memory.extraction_prompt_version.clone(),
            ))
            .await
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Memory creation failed: {}", e).into(),
                data: None,
            })?;
        Ok(())
    }

    async fn stage_memory_for_review(
        &self,
        memory: &ExtractedMemory,
    ) -> std::result::Result<(), McpError> {
        let table = match memory.kind.as_str() {
            "entity" => "kg_entity_candidates",
            "relationship" => "kg_edge_candidates",
            _ => "kg_entity_candidates",
        };

        let sql = format!(
            r#"
            CREATE {} SET
                status = 'pending',
                data = $data,
                confidence = $confidence,
                source_thought_ids = $source_thought_ids,
                extraction_batch_id = $extraction_batch_id,
                extracted_at = $extracted_at,
                extraction_prompt_version = $extraction_prompt_version
        "#,
            table
        );

        self.db
            .query(&sql)
            .bind(("data", memory.data.clone()))
            .bind(("confidence", memory.confidence))
            .bind(("source_thought_ids", memory.source_thought_ids.clone()))
            .bind(("extraction_batch_id", memory.extraction_batch_id.clone()))
            .bind(("extracted_at", memory.extracted_at.clone()))
            .bind((
                "extraction_prompt_version",
                memory.extraction_prompt_version.clone(),
            ))
            .await
            .map_err(|e| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Staging failed: {}", e).into(),
                data: None,
            })?;
        Ok(())
    }
}
