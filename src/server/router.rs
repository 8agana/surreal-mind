use crate::server::SurrealMindServer;
use rmcp::{
    ErrorData as McpError,
    handler::server::ServerHandler,
    model::{
        CacheScope, CallToolRequestParams, CallToolResponse, CallToolResult, Implementation,
        ListPromptsResult, ListResourceTemplatesResult, ListResourcesResult, ListToolsResult,
        PaginatedRequestParams, ProtocolVersion, ResultType, ServerCapabilities, ServerInfo, Tool,
        ToolsCapability,
    },
    service::{RequestContext, RoleServer},
};
use tracing::info;

const LIST_TTL_MS: u64 = 300_000;

fn uses_2026_list_shape(context: &RequestContext<RoleServer>) -> bool {
    context
        .protocol_version()
        .is_some_and(|version| version >= ProtocolVersion::V_2026_07_28)
}

fn list_shape_fields(draft_2026: bool) -> (Option<ResultType>, Option<u64>, Option<CacheScope>) {
    if draft_2026 {
        (
            Some(ResultType::COMPLETE),
            Some(LIST_TTL_MS),
            Some(CacheScope::Public),
        )
    } else {
        (None, None, None)
    }
}

/// Serialize list results in the protocol era the request actually uses.
///
/// Claude Code's inline client declares `2026-07-28` on every request and
/// requires `resultType`, `ttlMs`, and `cacheScope`. Older session clients can
/// reject those same fields as unknown. The request context is therefore the
/// type marker; one unconditional wire shape cannot serve both eras safely.
fn list_tools_result(tools: Vec<Tool>, draft_2026: bool) -> ListToolsResult {
    let (result_type, ttl_ms, cache_scope) = list_shape_fields(draft_2026);
    ListToolsResult {
        result_type,
        ttl_ms,
        cache_scope,
        tools,
        ..Default::default()
    }
}

/// Build the full, ordered list of MCP tools this server exposes.
///
/// Pure: touches no server state (`self`) and no database. `list_tools` below
/// (the actual `tools/list` wire handler) delegates to this directly, so a
/// DB-free test calling this function exercises the real registration path,
/// not a hand-maintained duplicate of it (see `tests/tool_roster_db_free.rs`).
pub fn build_tool_list() -> Vec<Tool> {
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
            "search",
            "Unified search for entities, observations, and thoughts",
            search_schema_map,
        )
        .with_title("Search"),
    );

    // (photography tools removed from this server)

    tools
}

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
    // request.protocol_version.clone()`).

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListResourcesResult, McpError> {
        let (result_type, ttl_ms, cache_scope) = list_shape_fields(uses_2026_list_shape(&context));
        Ok(ListResourcesResult {
            result_type,
            ttl_ms,
            cache_scope,
            ..Default::default()
        })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListResourceTemplatesResult, McpError> {
        let (result_type, ttl_ms, cache_scope) = list_shape_fields(uses_2026_list_shape(&context));
        Ok(ListResourceTemplatesResult {
            result_type,
            ttl_ms,
            cache_scope,
            ..Default::default()
        })
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListPromptsResult, McpError> {
        let (result_type, ttl_ms, cache_scope) = list_shape_fields(uses_2026_list_shape(&context));
        Ok(ListPromptsResult {
            result_type,
            ttl_ms,
            cache_scope,
            ..Default::default()
        })
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        info!("tools/list requested");
        Ok(list_tools_result(
            build_tool_list(),
            uses_2026_list_shape(&context),
        ))
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

#[cfg(test)]
mod list_tools_result_tests {
    use super::*;

    #[test]
    fn list_tools_result_matches_protocol_era() {
        let tool = Tool::new(
            "probe",
            "probe tool for protocol-era regression coverage",
            std::sync::Arc::new(serde_json::Map::new()),
        );
        let legacy = list_tools_result(vec![tool.clone()], false);

        assert_eq!(legacy.ttl_ms, None);
        assert_eq!(legacy.cache_scope, None);
        assert_eq!(legacy.result_type, None);

        let wire = serde_json::to_value(&legacy).expect("ListToolsResult must serialize");
        for draft_field in ["resultType", "ttlMs", "cacheScope"] {
            assert!(
                wire.get(draft_field).is_none(),
                "pre-2026 tools/list must omit draft field {draft_field}"
            );
        }

        let draft = list_tools_result(vec![tool], true);
        assert_eq!(draft.result_type, Some(ResultType::COMPLETE));
        assert_eq!(draft.ttl_ms, Some(LIST_TTL_MS));
        assert_eq!(draft.cache_scope, Some(CacheScope::Public));
        let wire = serde_json::to_value(&draft).expect("ListToolsResult must serialize");
        assert_eq!(wire["resultType"], serde_json::json!("complete"));
        assert_eq!(wire["ttlMs"], serde_json::json!(LIST_TTL_MS));
        assert_eq!(wire["cacheScope"], serde_json::json!("public"));
    }
}
