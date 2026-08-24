use crate::server::SurrealMindServer;
use rmcp::{
    ErrorData as McpError,
    handler::server::ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, ToolsCapability,
    },
    service::{RequestContext, RoleServer},
};
use tracing::info;

impl ServerHandler for SurrealMindServer {
    fn get_info(&self) -> ServerInfo {
        // D4: preserve `tools.listChanged = false` explicitly rather than
        // letting it become absent on the wire. `ToolsCapability` is
        // `#[non_exhaustive]`, so it is built via `Default` and mutated
        // (field assignment on a non-exhaustive struct is legal; only
        // struct-literal construction is banned) rather than constructed
        // with a struct literal.
        let mut tools_capability = ToolsCapability::default();
        tools_capability.list_changed = Some(false);
        let capabilities = ServerCapabilities::builder()
            .enable_tools_with(tools_capability)
            .build();
        let server_info = Implementation::new("surreal-mind", env!("CARGO_PKG_VERSION"))
            .with_title("Surreal Mind")
            .with_description("Persistent cognition kernel for LegacyMind")
            .with_website_url("https://github.com/8agana/surreal-mind");
        ServerInfo::new(capabilities).with_server_info(server_info)
    }

    // D3: no `initialize` override. rmcp's default implementation negotiates
    // the response `protocol_version` against `supported_protocol_versions()`
    // instead of echoing whatever the client claims (which is what the
    // previous override did via `info.protocol_version =
    // request.protocol_version.clone()`). Narrowing
    // `supported_protocol_versions()` is deferred until protocol tests prove
    // it is required.

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
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
        let journal_schema_map = crate::schemas::journal_schema();
        let rethink_schema_map = crate::schemas::rethink_schema();
        let corrections_schema_map = crate::schemas::corrections_schema();
        let test_notification_schema_map = crate::schemas::test_notification_schema();

        let call_gem_schema = crate::schemas::call_gem_schema();
        let call_cc_schema = crate::schemas::call_cc_schema();
        let call_vibe_schema = crate::schemas::call_vibe_schema();
        let call_status_schema = crate::schemas::call_status_schema();
        let call_jobs_schema = crate::schemas::call_jobs_schema();
        let call_cancel_schema = crate::schemas::call_cancel_schema();

        // Output schemas (rmcp 0.11.0+)
        // Output schemas removed as they are no longer used or needed for simple tool defs

        let mut tools = vec![
            Tool::new(
                "think",
                "Unified thinking tool with automatic mode routing (Plan, Build, Debug, Stuck)",
                think_schema_map,
            )
            .with_title("Think"),
            Tool::new(
                "wander",
                "Explore the knowledge graph to form new connections, provide context, and verify information. Use this for curiosity-driven exploration, not goal-directed search.",
                wander_schema_map,
            )
            .with_title("Wander"),
            Tool::new(
                "maintain",
                "Maintenance operations for archival, cleanup, and health checks",
                maintain_schema_map,
            )
            .with_title("Maintain"),
            Tool::new(
                "journal",
                "Research thread management — create threads, add entries, view dashboard, update status. A looking glass over the KG for structured research.",
                journal_schema_map,
            )
            .with_title("Journal"),
            Tool::new(
                "rethink",
                "Mark records for revision or correction by federation members",
                rethink_schema_map,
            )
            .with_title("Rethink"),
            Tool::new(
                "corrections",
                "List correction events with optional target filter",
                corrections_schema_map,
            )
            .with_title("Corrections"),
            Tool::new(
                "test_notification",
                "Send a test logging notification to the client",
                test_notification_schema_map,
            )
            .with_title("Test Notification"),
            // (legacy think_search removed — use legacymind_search)
            Tool::new(
                "remember",
                "Create entities, relationships, or observations in the knowledge graph",
                remember_schema_map,
            )
            .with_title("Remember"),
            // (legacy memories_search removed — use legacymind_search)
            Tool::new(
                "howto",
                "Get detailed help and usage examples for available tools",
                howto_schema_map,
            )
            .with_title("How To"),
        ];

        tools.push(
            Tool::new(
                "call_gem",
                "Delegate a task to the configured Google CLI provider (Gemini or Antigravity)",
                call_gem_schema,
            )
            .with_title("Call Gem"),
        );

        tools.push(
            Tool::new(
                "call_cc",
                "Delegate a task to Claude Code CLI with full context and tracking",
                call_cc_schema,
            )
            .with_title("Call Claude Code"),
        );

        tools.push(
            Tool::new(
                "call_vibe",
                "Delegate a task to Vibe CLI with full context and tracking",
                call_vibe_schema,
            )
            .with_title("Call Vibe"),
        );

        tools.push(
            Tool::new(
                "search",
                "Unified search for entities, observations, and thoughts",
                search_schema_map,
            )
            .with_title("Search"),
        );

        tools.push(
            Tool::new(
                "call_status",
                "Check the status and results of a delegated agent job",
                call_status_schema,
            )
            .with_title("Call Status"),
        );

        tools.push(
            Tool::new(
                "call_jobs",
                "List active or completed delegated agent jobs",
                call_jobs_schema,
            )
            .with_title("Call Jobs"),
        );

        tools.push(
            Tool::new(
                "call_cancel",
                "Cancel an active delegated agent job",
                call_cancel_schema,
            )
            .with_title("Call Cancel"),
        );

        // (photography tools removed from this server)

        Ok(ListToolsResult {
            tools,
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResponse, McpError> {
        // D2: handler modules keep returning `CallToolResult`; convert to the
        // wire-level `CallToolResponse` exactly once, here at the router
        // boundary, rather than widening every handler's return type.
        let result: std::result::Result<CallToolResult, McpError> = match request.name.as_ref() {
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
            "journal" => self.handle_journal(request).await.map_err(|e| e.into()),
            "rethink" => self.handle_rethink(request).await.map_err(|e| e.into()),
            // Memory tools
            "remember" => self
                .handle_knowledgegraph_create(request)
                .await
                .map_err(|e| e.into()),
            "call_gem" => self.handle_call_gem(request).await.map_err(|e| e.into()),
            "call_cc" => self.handle_call_cc(request).await.map_err(|e| e.into()),
            "call_vibe" => self.handle_call_vibe(request).await.map_err(|e| e.into()),

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
            "howto" => self.handle_howto(request).await.map_err(|e| e.into()),
            "search" => self
                .handle_unified_search(request)
                .await
                .map_err(|e| e.into()),

            _ => Err(McpError {
                code: rmcp::model::ErrorCode::METHOD_NOT_FOUND,
                message: format!("Unknown tool: {}", request.name).into(),
                data: None,
            }),
        };
        result.map(Into::into)
    }
}
