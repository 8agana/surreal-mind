use crate::server::SurrealMindServer;
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
use tracing::info;

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

        // use crate::tools::unified_search::SearchQuery; // Removed as likely internal or unused in this scope
        // use crate::tools::unified_search::UnifiedSearchParams; // Unused

        use rmcp::model::Tool;

        // Input schemas
        let legacymind_think_schema_map = crate::schemas::legacymind_think_schema();
        let maintenance_ops_schema_map = crate::schemas::maintenance_ops_schema();
        let kg_create_schema_map = crate::schemas::kg_create_schema();
        let detailed_help_schema_map = crate::schemas::detailed_help_schema();
        let unified_schema = crate::schemas::unified_search_schema();
        let wander_schema_map = crate::schemas::wander_schema();

        let delegate_gemini_schema = crate::schemas::delegate_gemini_schema();
        let agent_job_status_schema = crate::schemas::agent_job_status_schema();
        let list_agent_jobs_schema = crate::schemas::list_agent_jobs_schema();
        let cancel_agent_job_schema = crate::schemas::cancel_agent_job_schema();

        // Output schemas (rmcp 0.11.0+)
        let legacymind_think_output = crate::schemas::legacymind_think_output_schema();
        let maintenance_ops_output = crate::schemas::maintenance_ops_output_schema();
        let memories_create_output = crate::schemas::memories_create_output_schema();
        let detailed_help_output = crate::schemas::detailed_help_output_schema();
        let unified_search_output = crate::schemas::legacymind_search_output_schema();

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
                name: "legacymind_wander".into(),
                title: Some("LegacyMind Wander".into()),
                description: Some("Interactively explore the knowledge graph via random, semantic, or meta traversals.".into()),
                input_schema: wander_schema_map,
                icons: None,
                annotations: None,
                output_schema: None, // Dynamic output, hard to schema-tize strictly or just JSON
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

        tools.push(Tool {
            name: "delegate_gemini".into(),
            title: Some("Delegate Gemini".into()),
            description: Some(
                "Delegate a prompt to Gemini CLI with persisted exchange tracking.".into(),
            ),
            input_schema: delegate_gemini_schema.clone(),
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
            name: "agent_job_status".into(),
            title: Some("Agent Job Status".into()),
            description: Some("Get status of an async agent job by job_id".into()),
            input_schema: agent_job_status_schema.clone(),
            icons: None,
            annotations: None,
            output_schema: None,
            meta: None,
        });

        tools.push(Tool {
            name: "list_agent_jobs".into(),
            title: Some("List Agent Jobs".into()),
            description: Some(
                "List async agent jobs with optional filtering by status and tool_name".into(),
            ),
            input_schema: list_agent_jobs_schema.clone(),
            icons: None,
            annotations: None,
            output_schema: None,
            meta: None,
        });

        tools.push(Tool {
            name: "cancel_agent_job".into(),
            title: Some("Cancel Agent Job".into()),
            description: Some("Cancel a running or queued async agent job".into()),
            input_schema: cancel_agent_job_schema.clone(),
            icons: None,
            annotations: None,
            output_schema: None,
            meta: None,
        });

        let manage_proposals_schema = crate::schemas::legacymind_manage_proposals_schema();
        let manage_proposals_output = crate::schemas::legacymind_manage_proposals_output_schema();

        tools.push(Tool {
            name: "legacymind_manage_proposals".into(),
            title: Some("Manage Knowledge Graph Proposals".into()),
            description: Some(
                "Review, approve, or reject pending knowledge graph changes from the Gardener."
                    .into(),
            ),
            input_schema: manage_proposals_schema,
            icons: None,
            annotations: None,
            output_schema: Some(manage_proposals_output),
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
            "legacymind_wander" => self.handle_wander(request).await.map_err(|e| e.into()),
            "maintenance_ops" => self
                .handle_maintenance_ops(request)
                .await
                .map_err(|e| e.into()),
            // Memory tools
            "memories_create" => self
                .handle_knowledgegraph_create(request)
                .await
                .map_err(|e| e.into()),
            "delegate_gemini" => self
                .handle_delegate_gemini(request)
                .await
                .map_err(|e| e.into()),

            "agent_job_status" => self
                .handle_agent_job_status(request)
                .await
                .map_err(|e| e.into()),
            "list_agent_jobs" => self
                .handle_list_agent_jobs(request)
                .await
                .map_err(|e| e.into()),
            "cancel_agent_job" => self
                .handle_cancel_agent_job(request)
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
            "legacymind_manage_proposals" => self
                .handle_manage_proposals(request)
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
