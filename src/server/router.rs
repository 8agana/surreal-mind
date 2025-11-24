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

        use rmcp::model::Tool;

        let legacymind_think_schema_map = crate::schemas::legacymind_think_schema();

        let maintenance_ops_schema_map = crate::schemas::maintenance_ops_schema();
        let kg_create_schema_map = crate::schemas::kg_create_schema();
        let kg_moderate_schema_map = crate::schemas::kg_moderate_schema();
        let brain_store_schema_map = crate::schemas::brain_store_schema();
        let detailed_help_schema_map = crate::schemas::detailed_help_schema();
        let inner_voice_schema_map = crate::schemas::inner_voice_schema();
        let unified_schema = crate::schemas::unified_search_schema();

        let mut tools = vec![
            Tool {
                name: "legacymind_think".into(),
                title: Some("LegacyMind Think".into()),
                description: Some("Unified thinking tool with automatic mode routing".into()),
                input_schema: legacymind_think_schema_map.clone(),
                icons: None,
                annotations: None,
                    output_schema: None,
                meta: None,
            },
            Tool {
                name: "maintenance_ops".into(),
                title: Some("Maintenance Operations".into()),
                description: Some("Maintenance operations for archival and cleanup".into()),
                input_schema: maintenance_ops_schema_map,
                icons: None,
                annotations: None,
                    output_schema: None,
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
                    output_schema: None,
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
                    output_schema: None,
                meta: None,
            },
            Tool {
                name: "detailed_help".into(),
                title: Some("Detailed Help".into()),
                description: Some("Get detailed help for a specific tool".into()),
                input_schema: detailed_help_schema_map,
                icons: None,
                annotations: None,
                    output_schema: None,
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
                output_schema: None,
                meta: None,
        });
        tools.push(Tool {
            name: "brain_store".into(),
            title: Some("Brain Store".into()),
            description: Some("Get or set persistent brain sections for agents".into()),
            input_schema: brain_store_schema_map,
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

            // Help
            "detailed_help" => self
                .handle_detailed_help(request)
                .await
                .map_err(|e| e.into()),
            "legacymind_search" => self
                .handle_unified_search(request)
                .await
                .map_err(|e| e.into()),
            "brain_store" => self.handle_brain_store(request).await.map_err(|e| e.into()),
            _ => Err(McpError {
                code: rmcp::model::ErrorCode::METHOD_NOT_FOUND,
                message: format!("Unknown tool: {}", request.name).into(),
                data: None,
            }),
        }
    }
}
