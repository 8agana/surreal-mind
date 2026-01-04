## [0.1.2] - 2026-01-03

### Changed

- (2026-01-03) **`detailed_help`**: Updated the `detailed_help` tool to include all 9 tools, removed legacy aliases, and added help for the new tools.
- (2026-01-03) **`detailed_help` schema alignment**: Comprehensively updated `detailed_help` to match the exact runtime schemas of all tools (including `maintenance_ops`, `legacymind_search`, and `delegate_gemini` parameter updates). Added full documentation for the 3 async agent job tools (`agent_job_status`, `list_agent_jobs`, `cancel_agent_job`), bringing the total documented tool count to 12.

## [0.1.2] - 2026-01-02

### Fixed

- **kg_populate**: Initialize `embedding` field to NONE when creating kg_entities, kg_edges, kg_observations, and kg_boundaries. Previously records were created without the field, preventing kg_embed from finding them.
- **kg_embed**: Removed invalid `IS NOT DEFINED` syntax from WHERE clauses (SurrealDB doesn't support this operator). Since kg_populate now initializes all embeddings to NONE, simplified WHERE conditions work correctly for all cases: NULL, NONE, non-arrays, and empty arrays.
- **kg_embed SurrealDB syntax**: Removed invalid `NOT type::is::array(embedding)` and `(type::is::array(embedding) AND array::len(embedding) = 0)` patterns from WHERE clauses in SELECT and UPDATE queries for entities, observations, and edges. Simplified to use only `array::len(embedding) = 0` which safely handles all non-array types and empty arrays. The conditional SELECT already validates type safety with `IF type::is::array()` expressions.

## [Unreleased]

### Added

- (2026-01-03) **Smtop Admin-Ops Revamp**: Transformed `smtop` TUI into a comprehensive admin-ops console with actionable hotkeys for KG operations (kg_populate, kg_embed, reembed_kg), health checks, build+restart, fmt, and clippy. Added live command runner pane showing command status, duration, and tail output with stdout/stderr prefixes. Integrated ops results into combined logs for persistence. Preserved existing monitoring (service, cloudflared, sessions, DB, logs) while reflowing UI layout. Supports toggles for auto-restart, release bins, dry-run, and env overrides for batch size/limits. Commands run asynchronously without blocking TUI, with proper error handling and status feedback.

- (2026-01-02) **Streaming JSON Support for Gemini CLI**: Enhanced `delegate_gemini` tool with real-time streaming JSON event parsing for precise monitoring and hang detection. The implementation uses Gemini CLI's `--output-format stream-json` flag to receive newline-delimited JSON events (init, tool_use, tool_result, content, error, end) during execution, enabling granular tracking of tool execution and content generation.

- (2026-01-02) **Dual Timeout System**: Implemented sophisticated timeout management with two independent mechanisms:
  - **Inactivity Timeout**: Monitors overall output inactivity (default 120s, configurable via `GEMINI_TIMEOUT_MS`)
  - **Per-Tool Timeout**: Tracks individual tool execution times (default 300s, configurable via `GEMINI_TOOL_TIMEOUT_MS`)
  - The system can distinguish between "thinking" (active tools) and "hanging" (stuck tools) states

- (2026-01-02) **Stream Event Exposure**: Added optional `expose_stream` parameter to `delegate_gemini` tool that, when enabled, returns the complete sequence of streaming events in the response. This provides full visibility into the execution process for debugging and monitoring purposes.

- (2026-01-01) kg_populate run successful: 904 thoughts processed (Session 3) + 36 more (Session 4) = 940 total, 97.8%+ success rate. Knowledge graph extraction working: 660+ entities, 799+ edges, 1290+ observations, 259+ boundaries created. Shell aliases added: `kgpop` (kg_populate runner), `kgembed` (re-embedding workflow).
- (2025-12-20) Implemented `memories_populate` tool: Processes unextracted thoughts via Gemini CLI to populate knowledge graph, with session persistence, auto-approval, and batch tracking. Includes schema updates, session management, and integration with existing KG tables.
- (2025-12-19) Added `curiosity_add`, `curiosity_get`, `curiosity_search` tools for lightweight note-taking with semantic search.

### Fixed

- (2026-01-02) **Gemini CLI Monitoring**: Completely revamped timeout and hang detection logic using streaming JSON events instead of fragile heuristics. The new approach provides real-time visibility into tool execution and can precisely identify which specific tool/subtask is hanging, eliminating false timeouts during legitimate network waits.

- (2026-01-02) **Timeout Configuration**: Added proper environment variable support for both inactivity timeout (`GEMINI_TIMEOUT_MS`) and per-tool timeout (`GEMINI_TOOL_TIMEOUT_MS`) with sensible defaults (120s and 300s respectively).

- (2026-01-02) `delegate_gemini` worker now skips legacy queued jobs with missing prompt/task_name by filtering for non-empty string prompts and tolerating optional fields during claim/exec.
- (2026-01-02) `check_embedding_dims` deserialization corrected to avoid false startup warnings when embedding dimensions are consistent.
- (2025-12-26) Refactored `memories_populate` update logic to use native `db.update().merge()` SDK method instead of raw SQL queries. This definitively resolves record ID binding issues (UUID vs String) that were causing silent update failures.
- (2025-12-25) `memories_populate` now returns fully structured MCP output (no RawContent paths), records `extracted_at` and `thought_ids`, and defaults Gemini CLI to `gemini-3-pro-preview`; parsing now strips code fences and surfaces stdout snippets on error. Workspace fmt/clippy/tests all passing.
- (2025-12-24) Cleared clippy `collapsible_if` and `unnecessary_unwrap` across knowledge_graph, maintenance, http, binaries (smtop, reembed_kg, kg_dedupe_plan) and tests; workspace now clippy-clean with full test suite passing.
- (2025-12-23) Updated `detailed_help` documentation for `legacymind_think` to accurately reflect its return structure (flat JSON, not nested) and clarify that framework analysis is DB-only.

### Removed

- (2025-12-30) Removed `inner_voice` tool and all supporting code, tests, scripts, and documentation. The tool's retrieval + synthesis + auto-extract workflow has been replaced by `legacymind_search` + `delegate_gemini` combinations. Removed environment variables: `SURR_ENABLE_INNER_VOICE`, `SURR_DISABLE_INNER_VOICE`, `SURR_INNER_VOICE_*`, `SURR_IV_*`, `IV_ALLOW_GROK`, `INNER_VOICE_LOCAL_FALLBACK`. Removed Cargo dependencies: `blake3`, `unicode-normalization`. Removed scripts: `scripts/iv_extract.js`, `lib/iv_utils.js`. Updated tool roster to 9 tools.
- (2025-12-19) Fixed `recency_days` parameter in search tools - was being ignored, now properly filters by date.

### Changed

- (2026-01-02) **Async-Only Execution**: Converted `delegate_gemini` tool to async-only execution model. Removed synchronous execution path and `fire_and_forget` parameter. All calls now queue background jobs and return job IDs for status tracking. This simplifies the architecture and ensures consistent behavior.

- (2026-01-02) **Gemini CLI Integration**: Changed default output format from regular JSON to streaming JSON (`--output-format stream-json`) for real-time monitoring capabilities. This is a backward-compatible change that enhances functionality without breaking existing usage.

- (2026-T01-02) **Agent Response Structure**: Extended `AgentResponse` struct to optionally include streaming events when `expose_stream` is enabled. The new `stream_events` field is conditionally serialized to maintain backward compatibility.

- (2025-12-23) Database migration: Updated 552 thoughts from `extracted_to_kg = NONE` to `extracted_to_kg = false` to make them eligible for memories_populate processing.

### Known Issues

- (2025-12-25) None currently known. Monitor `memories_populate` on next live run to confirm `extracted_at` stamping persists.
