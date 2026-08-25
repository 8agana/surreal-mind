use std::borrow::Cow;

use crate::server::SurrealMindServer;
use rmcp::{
    ErrorData as McpError,
    handler::server::ServerHandler,
    model::{
        CacheScope, CallToolRequestParams, CallToolResponse, CallToolResult, Implementation,
        ListToolsResult, PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerInfo,
        Tool, ToolsCapability,
    },
    service::{RequestContext, RoleServer},
};
use tracing::info;

/// How long clients may treat a `tools/list` response as fresh (SEP-2549).
///
/// The tool roster is fixed for the life of the process — `get_info()`
/// advertises `tools.listChanged = false` — so a short, non-zero TTL is safe
/// and avoids clients re-fetching on every turn.
const TOOLS_LIST_TTL_MS: u64 = 300_000; // 5 minutes

/// Protocol revisions SurrealMind implements completely.
///
/// rmcp 3.1.4 knows the draft `2026-07-28` revision, but its default empty
/// `resources/list`, `resources/templates/list`, and `prompts/list` responses
/// omit cache metadata that the draft requires. Advertising every revision the
/// SDK knows therefore overstates this server's contract. Keep negotiation at
/// the latest fully implemented revision until the whole draft surface is
/// covered, rather than patching one response at a time.
const SUPPORTED_PROTOCOL_VERSIONS: &[ProtocolVersion] = &[
    ProtocolVersion::V_2024_11_05,
    ProtocolVersion::V_2025_03_26,
    ProtocolVersion::V_2025_06_18,
    ProtocolVersion::V_2025_11_25,
];

/// Build the `tools/list` result with SEP-2549 cache metadata populated.
///
/// rmcp 3.1.4's `ListToolsResult::default()` leaves `ttl_ms`/`cache_scope`
/// as `None`, which `serde` then omits from the wire entirely
/// (`skip_serializing_if = "Option::is_none"`). That is valid per rmcp's own
/// backward-compat contract for peers on protocol versions older than
/// `2026-07-28`. Before SurrealMind narrowed `supported_protocol_versions()`,
/// it negotiated `2026-07-28` with any client that offered it. Claude Code
/// 2.1.241 is one such client, and its
/// `tools/list` response schema for that protocol version treats `ttlMs`
/// and `cacheScope` as *required* — stricter than rmcp's own leniency —
/// so an omitted field fails client-side validation with "tools fetch
/// failed" and the server never connects (measured 2026-08-24; rmcp 0.16.0
/// predates SEP-2549 and protocol `2026-07-28` entirely, so it never offered
/// that version and never hit this). The tool roster is identical for every
/// caller of this single-tenant server, so `CacheScope::Public` is correct
/// per the SEP-2549 semantics ("any client or intermediary may cache and
/// serve the response to any user").
fn list_tools_result(tools: Vec<Tool>) -> ListToolsResult {
    ListToolsResult {
        tools,
        ..Default::default()
    }
    .with_ttl_ms(TOOLS_LIST_TTL_MS)
    .with_cache_scope(CacheScope::Public)
}

impl ServerHandler for SurrealMindServer {
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(SUPPORTED_PROTOCOL_VERSIONS)
    }

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
    // request.protocol_version.clone()`). `supported_protocol_versions()` is
    // intentionally narrowed above: the server negotiates only revisions it
    // implements across every method.

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        info!("tools/list requested");

        // use crate::tools::unified_search::SearchQuery; // Removed as likely internal or unused in this scope
        // use crate::tools::unified_search::UnifiedSearchParams; // Unused

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

        Ok(list_tools_result(tools))
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

#[cfg(test)]
mod list_tools_result_tests {
    use super::*;

    /// Regression test for the 2026-08-24 Claude Code 2.1.241 compatibility
    /// break: a `tools/list` response must serialize `ttlMs` as a JSON
    /// number and `cacheScope` as `"public"`/`"private"`, never omit them.
    /// Prior to the fix, `ListToolsResult { tools, ..Default::default() }`
    /// left both `None`, which serde drops from the wire entirely, and
    /// Claude Code's schema for the negotiated `2026-07-28` protocol version
    /// rejects that as `expected number at path ttlMs (received undefined)`.
    #[test]
    fn list_tools_result_serializes_required_sep2549_fields() {
        let tool = Tool::new(
            "probe",
            "probe tool for cache-metadata regression coverage",
            std::sync::Arc::new(serde_json::Map::new()),
        );
        let result = list_tools_result(vec![tool]);

        assert_eq!(
            result.ttl_ms,
            Some(TOOLS_LIST_TTL_MS),
            "ttl_ms must be populated, not left at the None default"
        );
        assert_eq!(
            result.cache_scope,
            Some(CacheScope::Public),
            "cache_scope must be populated, not left at the None default"
        );

        let wire = serde_json::to_value(&result).expect("ListToolsResult must serialize");
        assert_eq!(
            wire["ttlMs"],
            serde_json::json!(TOOLS_LIST_TTL_MS),
            "ttlMs must be a JSON number on the wire, never absent"
        );
        assert_eq!(
            wire["cacheScope"],
            serde_json::json!("public"),
            "cacheScope must be the string \"public\" on the wire, never absent"
        );
    }
}
