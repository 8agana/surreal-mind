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
        let think_schema_map = crate::schemas::think_schema();
        let maintain_schema_map = crate::schemas::maintain_schema();
        let remember_schema_map = crate::schemas::remember_schema();
        let howto_schema_map = crate::schemas::howto_schema();
        let search_schema_map = crate::schemas::search_schema();
        let wander_schema_map = crate::schemas::wander_schema();
        let rethink_schema_map = crate::schemas::rethink_schema();
        let corrections_schema_map = crate::schemas::corrections_schema();
        let test_notification_schema_map = crate::schemas::test_notification_schema();

        let call_gem_schema = crate::schemas::call_gem_schema();
        let call_codex_schema = crate::schemas::call_codex_schema();
        let call_cc_schema = crate::schemas::call_cc_schema();
        let call_status_schema = crate::schemas::call_status_schema();
        let call_jobs_schema = crate::schemas::call_jobs_schema();
        let call_cancel_schema = crate::schemas::call_cancel_schema();

        // Output schemas (rmcp 0.11.0+)
        // Output schemas removed as they are no longer used or needed for simple tool defs

        let mut tools = vec![
            Tool {
                name: "think".into(),
                title: Some("Think".into()),
                description: Some("Unified thinking tool with automatic mode routing (Plan, Build, Debug, Stuck)".into()),
                input_schema: think_schema_map.clone(),
                icons: None,
                annotations: None,
                output_schema: None,
                meta: None,
            },
            Tool {
                name: "wander".into(),
                title: Some("Wander".into()),
                description: Some("Explore the knowledge graph to form new connections, provide context, and verify information. Use this for curiosity-driven exploration, not goal-directed search.".into()),
                input_schema: wander_schema_map,
                icons: None,
                annotations: None,
                output_schema: None,
                meta: None,
            },
            Tool {
                name: "maintain".into(),
                title: Some("Maintain".into()),
                description: Some("Maintenance operations for archival, cleanup, and health checks".into()),
                input_schema: maintain_schema_map,
                icons: None,
                annotations: None,
                output_schema: None,
                meta: None,
            },
            Tool {
                name: "rethink".into(),
                title: Some("Rethink".into()),
                description: Some("Mark records for revision or correction by federation members".into()),
                input_schema: rethink_schema_map,
                icons: None,
                annotations: None,
                output_schema: None,
                meta: None,
            },
            Tool {
                name: "corrections".into(),
                title: Some("Corrections".into()),
                description: Some("List correction events with optional target filter".into()),
                input_schema: corrections_schema_map,
                icons: None,
                annotations: None,
                output_schema: None,
                meta: None,
            },
            Tool {
                name: "test_notification".into(),
                title: Some("Test Notification".into()),
                description: Some("Send a test logging notification to the client".into()),
                input_schema: test_notification_schema_map,
                icons: None,
                annotations: None,
                output_schema: None,
                meta: None,
            },
            // (legacy think_search removed — use legacymind_search)
            Tool {
                name: "remember".into(),
                title: Some("Remember".into()),
                description: Some("Create entities, relationships, or observations in the knowledge graph".into()),
                input_schema: remember_schema_map,
                icons: None,
                annotations: None,
                output_schema: None,
                meta: None,
            },
            // (legacy memories_search removed — use legacymind_search)
            Tool {
                name: "howto".into(),
                title: Some("How To".into()),
                description: Some("Get detailed help and usage examples for available tools".into()),
                input_schema: howto_schema_map,
                icons: None,
                annotations: None,
                output_schema: None,
                meta: None,
            },
        ];

        tools.push(Tool {
            name: "call_gem".into(),
            title: Some("Call Gem".into()),
            description: Some(
                "Delegate a task to Gemini CLI with full context and tracking".into(),
            ),
            input_schema: call_gem_schema.clone(),
            icons: None,
            annotations: None,
            output_schema: None,
            meta: None,
        });

        tools.push(Tool {
            name: "call_codex".into(),
            title: Some("Call Codex".into()),
            description: Some("Delegate a task to Codex CLI with full context and tracking".into()),
            input_schema: call_codex_schema.clone(),
            icons: None,
            annotations: None,
            output_schema: None,
            meta: None,
        });

        tools.push(Tool {
            name: "call_cc".into(),
            title: Some("Call Claude Code".into()),
            description: Some(
                "Delegate a task to Claude Code CLI with full context and tracking".into(),
            ),
            input_schema: call_cc_schema.clone(),
            icons: None,
            annotations: None,
            output_schema: None,
            meta: None,
        });

        tools.push(Tool {
            name: "search".into(),
            title: Some("Search".into()),
            description: Some("Unified search for entities, observations, and thoughts".into()),
            input_schema: search_schema_map,
            icons: None,
            annotations: None,
            output_schema: None,
            meta: None,
        });

        tools.push(Tool {
            name: "call_status".into(),
            title: Some("Call Status".into()),
            description: Some("Check the status and results of a delegated agent job".into()),
            input_schema: call_status_schema.clone(),
            icons: None,
            annotations: None,
            output_schema: None,
            meta: None,
        });

        tools.push(Tool {
            name: "call_jobs".into(),
            title: Some("Call Jobs".into()),
            description: Some("List active or completed delegated agent jobs".into()),
            input_schema: call_jobs_schema.clone(),
            icons: None,
            annotations: None,
            output_schema: None,
            meta: None,
        });

        tools.push(Tool {
            name: "call_cancel".into(),
            title: Some("Call Cancel".into()),
            description: Some("Cancel an active delegated agent job".into()),
            input_schema: call_cancel_schema.clone(),
            icons: None,
            annotations: None,
            output_schema: None,
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
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        // Route to appropriate tool handler
        match request.name.as_ref() {
            // Unified thinking tool
            "think" => self
                .handle_legacymind_think(request)
                .await
                .map_err(|e| e.into()),

            // Test tool
            "test_notification" => self
                .handle_test_notification(request, context)
                .await
                .map_err(|e| e.into()),

            // Intelligence and utility
            "wander" => self.handle_wander(request).await.map_err(|e| e.into()),
            "corrections" => self.handle_corrections(request).await.map_err(|e| e.into()),
            "maintain" => self
                .handle_maintenance_ops(request)
                .await
                .map_err(|e| e.into()),
            "rethink" => self.handle_rethink(request).await.map_err(|e| e.into()),
            // Memory tools
            "remember" => self
                .handle_knowledgegraph_create(request)
                .await
                .map_err(|e| e.into()),
            "call_gem" => self
                .handle_delegate_gemini(request)
                .await
                .map_err(|e| e.into()),
            "call_codex" => self.handle_call_codex(request).await.map_err(|e| e.into()),
            "call_cc" => self.handle_call_cc(request).await.map_err(|e| e.into()),

            "call_status" => self
                .handle_agent_job_status(request)
                .await
                .map_err(|e| e.into()),
            "call_jobs" => self
                .handle_list_agent_jobs(request)
                .await
                .map_err(|e| e.into()),
            "call_cancel" => self
                .handle_cancel_agent_job(request)
                .await
                .map_err(|e| e.into()),

            // Help
            "howto" => self
                .handle_detailed_help(request)
                .await
                .map_err(|e| e.into()),
            "search" => self
                .handle_unified_search(request)
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
