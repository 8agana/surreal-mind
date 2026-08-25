## [Unreleased] - rmcp 3.1.4 upgrade (implementation candidate, not yet installed)

Branch `codex/rmcp-3.1.4`, isolated worktree cut from `874d229c8a4cd7494d452b9d44fa6d56c2e85ffb`. Full design, decisions (D1-D11), and risk register: `docs/tasks/20260823-rmcp-3.1.4-upgrade/`. **Package version intentionally left at `0.8.2`** — a dependency-only major bump is not, on its own, a recorded reason to change the crate version (see the upgrade doc's Constraint 9); a version bump remains available as its own decision at the deployment gate.

### Changed

- **rmcp `0.16.0` → `3.1.4`** (exact pin, all four existing feature flags — `macros`, `transport-io`, `transport-streamable-http-server`, `transport-worker` — preserved unchanged). Transitive lockfile deltas: `rmcp-macros` matched to `3.1.4`; `sse-stream` `0.2.1` → `0.2.5`; `darling`/`darling_core`/`darling_macro` `0.23.0` → `0.24.1` (rmcp-macros' proc-macro dependency); a second `syn` major (`3.0.4`, alongside the existing `2.0.114` used elsewhere in the tree) and `indexmap` pulled in as new direct/transitive dependencies of rmcp; a second `base64` version (`0.23.1`, alongside the existing `0.22.1`) because rmcp 3.1.4 moved to a newer `base64` major while every other consumer in the tree is still on `0.22`. No other dependency changed.
- **`Tool` construction**: all 16 `list_tools` entries now use `Tool::new(name, description, input_schema).with_title(title)` instead of the removed 0.16 struct-literal form (`execution` field no longer exists; `Tool` became `#[non_exhaustive]`). Names, titles, descriptions, input schemas, and registration order are unchanged.
- **`CallToolRequestParams` construction**: every call site (router self-calls in `maintenance.rs`, `kg_wander.rs`, and the test suite) now uses `CallToolRequestParams::new(name).with_arguments(args)` instead of the removed struct-literal form (the `task` field no longer exists on this type).
- **`get_info()`**: rebuilt with `ServerCapabilities::builder().enable_tools_with(...)` and `Implementation::new(...).with_title(...).with_description(...).with_website_url(...)`, since `ServerCapabilities`, `ToolsCapability`, `Implementation`, and `InitializeResult` (aka `ServerInfo`) are all `#[non_exhaustive]` in 3.1.4. `tools.listChanged` is still explicitly serialized as `false` (built via `ToolsCapability::default()` then field-mutated, since non-exhaustive structs still permit field assignment — only literal construction is banned).
- **Protocol negotiation**: removed the custom `initialize` override in `src/server/router.rs` that echoed back whatever `protocol_version` the client requested verbatim. rmcp 3.1.4's default `initialize` now negotiates the response version against `supported_protocol_versions()` (left at rmcp's own default, `ProtocolVersion::KNOWN_VERSIONS`) instead. An unsupported/future version can no longer be falsely accepted.
- **`call_tool` return type**: `ServerHandler::call_tool` now returns `CallToolResponse` (a new `Complete`/`InputRequired`/`Task` enum introduced for the SEP-2663 tasks extension and MRTR). Every one of the 15 tool-handler modules keeps returning `CallToolResult` unchanged; the router converts once, at the boundary, via `CallToolResult`'s `Into<CallToolResponse>`.
- **Content extraction**: `kg_wander`'s `execute_wander` and the equivalent test-suite assertions now match `rmcp::model::ContentBlock::Text(text)` directly instead of the removed `RawContent::Text` behind a `.raw` field — 3.1.4 dropped the `Content { raw, annotations }` wrapper in favor of `ContentBlock` as the direct enum in `CallToolResult::content`.
- **Streamable HTTP config** (`src/http.rs`): `StreamableHttpServerConfig` is `#[non_exhaustive]` in 3.1.4 and the old `stateful_mode: bool` field is gone, replaced by `legacy_session_mode: bool` (rmcp's closest equivalent, and also its own default). The service is now built via `StreamableHttpServerConfig::default().with_legacy_session_mode(true).with_sse_keep_alive(...).with_json_response(false).with_allowed_hosts(...).with_stateless_protocol_metadata_required(false)`, preserving prior behavior (`stateful_mode: true` → `legacy_session_mode: true`) while adding the new explicit Host allowlist. `allowed_origins` (Origin validation) is intentionally left at rmcp's empty/disabled default — see the new `SURR_HTTP_ALLOWED_HOSTS` entry below and the security note under Deferred.
- **`test_notification` / SEP-2577 logging deprecation bridge**: rmcp 3.1.4 deprecates the entire logging-notification surface (`LoggingLevel`, `LoggingMessageNotificationParam`, `notify_logging_message`) per SEP-2577, but keeps it functional. `test_notification` is a public tool this upgrade does not remove; `src/tools/test_notification.rs` carries the sole `#[allow(deprecated)]` bridge in the codebase, scoped to the `use` import and the handler function body only — the rest of the workspace remains warning-clean under `-D warnings`. Retiring or replacing this tool is a separate follow-up (not part of this upgrade — see upgrade doc D8).
- **Stale tool-count log**: `main.rs`'s startup log said "Loaded 15 MCP tools" and omitted `journal` from the list; corrected to 16 and the full name list. `howto.rs`'s hand-maintained overview-mode roster (`src/tools/howto.rs`, the `tools` vec used when `howto` is called with no `tool` argument) was separately missing `test_notification`; added. `tests/tool_schemas.rs`'s synthetic 14-tool expectation list was also stale (missing `journal` and `call_vibe`); corrected to the real 16.

### Fixed

- **Claude Code 2.1.241 `tools/list` compatibility (SEP-2549 `ttlMs`/`cacheScope`)**: production was briefly upgraded to this branch (`f4869de`) and Claude Code 2.1.241 reported `tools fetch failed — Invalid result for tools/list: expected number at path ttlMs (received undefined)` / `cacheScope expected public|private`; rollback to rmcp 0.16.0 restored `Connected`. Root cause: `src/server/router.rs`'s `list_tools` built `ListToolsResult { tools, ..Default::default() }`, leaving the new SEP-2549 `ttl_ms`/`cache_scope` fields `None`; rmcp's `paginated_result!` macro marks both `#[serde(skip_serializing_if = "Option::is_none")]`, so they were silently omitted from the wire — valid under rmcp's own backward-compat contract for peers below protocol `2026-07-28`, but this server doesn't narrow `supported_protocol_versions()` (still rmcp's default `KNOWN_VERSIONS`, which includes `2026-07-28`), so it negotiates `2026-07-28` with any client that offers it. Claude Code 2.1.241's `2026-07-28` response schema treats both fields as *required*, stricter than rmcp's own leniency; rmcp 0.16.0 predates SEP-2549 and protocol `2026-07-28` entirely, so it never offered that version and never hit this. Fix: `list_tools` now routes through a new `list_tools_result()` helper (`src/server/router.rs`) that explicitly calls `.with_ttl_ms(300_000)` and `.with_cache_scope(CacheScope::Public)` — 5-minute TTL (the roster is static; `tools.listChanged` is already `false`) and `Public` scope (the roster is identical for every caller of this single-tenant server). Tool names/order/descriptions/input schemas are unchanged; only the two new metadata fields are now always present on the wire. Reproduced pre-fix and verified fixed against an isolated candidate (Studio, port 18792, ephemeral token) using Claude Code 2.1.241 with a strict `--mcp-config`/`--strict-mcp-config` temp config; new regression test `server::router::list_tools_result_tests::list_tools_result_serializes_required_sep2549_fields` asserts both fields serialize correctly. 2026-08-24.

### Added

- **`RuntimeConfig::http_allowed_hosts`** (`src/config.rs`) and `SURR_HTTP_ALLOWED_HOSTS`: rmcp 3.1.4 added `Host`-header validation to Streamable HTTP as a DNS-rebinding defense, defaulting to loopback-only. The secure loopback set (`localhost`, `127.0.0.1`, `::1`) is always retained; an unset env var uses that set alone; a valid comma-separated configured value **extends and deduplicates** the loopback set (never replaces it); a present-but-empty, whitespace-only, or malformed (stray/leading/trailing comma) value fails startup loudly rather than silently degrading to loopback-only or allow-all. 9 new focused unit tests cover absent/default, extension, loopback retention, whitespace, duplicates, and every malformed-input shape (10 `#[test]` functions total in `src/config.rs`, including the pre-existing `test_config_loading`). The production value (expected: loopback plus `mcp.samataganaphotography.com`, evidenced out-of-band via the Cloudflare `Host`-forwarding measurement in the upgrade doc) is deployment configuration (launchd/env file), not a Rust constant, and is not embedded in this branch.
- **Protocol/tool-contract test coverage** (`tests/mcp_protocol.rs`): `test_list_tools_protocol` now asserts the exact 16-name, ordered tool contract plus a title/description/schema spot-check (TOOL-01/02), instead of only checking that `search` is present. New `test_initialize_protocol_negotiation` drives real `initialize` handshakes for a supported legacy version, the current latest version, and a synthesized unsupported future version, asserting negotiation behavior (never echoing an unsupported version as accepted) and that `tools.listChanged` still serializes as explicit `false` in every case (PROTO-01/02/03/05). New `test_notification_protocol_bridge` drives a real `test_notification` tool call through the protocol harness and asserts both the response and the client-observable notification arrive (TOOL-06).
- **`tests/stdio_smoke.rs`** (STDIO-01): stdio is `SURR_TRANSPORT`'s default and `main.rs`'s default transport path, so it gets a runtime witness rather than compile-only coverage. Spawns the built binary as a subprocess over piped stdio with a disposable config, sends newline-delimited JSON-RPC `initialize` + `notifications/initialized` + `tools/list`, and asserts a negotiated protocol version, `tools.listChanged: false`, and the exact 16-tool list — with no extraneous bytes on stdout before the JSON.
- All new tests requiring a live server (`mcp_protocol.rs` additions, `test_wander.rs`/`mcp_integration.rs` content-extraction fixes, `stdio_smoke.rs`) follow the existing `RUN_DB_TESTS=1`-gated pattern and were compiled and lint-checked (`cargo check --all-targets --features db_integration`, `cargo clippy ... -D warnings`) but **not executed** in this implementation pass — no verified disposable database/namespace was available in this session, and the upgrade's constraints forbid running database-writing tests against the production namespace. Execution against a real disposable namespace is a Testing-phase gate (see `docs/tasks/20260823-rmcp-3.1.4-upgrade/rmcp-3.1.4-upgrade-testing.md`).

### Deferred

- **Origin validation** (`allowed_origins`) remains at rmcp's empty/disabled default for this migration (upgrade doc D7) — the current accepted client-`Origin` set has not been measured. Bearer auth and the new Host allowlist still constrain the endpoint. Measuring real client `Origin` headers and enabling this sibling DNS-rebinding defense is a separate follow-up task.
- **`test_notification` / SEP-2577 retirement or replacement** is a separate follow-up task (upgrade doc D8); this upgrade only ports it forward without normalizing the deprecation.
- **Pre-existing dirty-worktree reconciliation** (`src/tools/maintenance.rs` and several documentation files modified in the source Studio worktree before this branch was cut) is a separate, explicitly-scoped pre-merge task (upgrade doc D11 / impl doc Phase 8) and is **not** included in this branch.

## [0.8.2] - 2026-03-12

### Fixed

- **`maintain embed_pending` persistence accounting**: Fixed pending-thought retries so they select plain `meta::id(id)` record keys before `type::record('thoughts', $id)` updates, verify that each update actually persisted a complete embedding with the expected dimension before counting success, refresh embedding metadata on retry, and report the real post-run pending count instead of subtracting successful attempts from an already-updated count.
- **Codex federation identity support**: Added `codex` as a valid `journal` author and `wander`/`rethink` attention-routing target so Codex-authored KG work preserves its own provenance instead of defaulting to `cc`.
- **REMini KG consolidation planning**: `gem_rethink` now writes structured pending merge state (`mode`, `loser_id`, `winner_id`) when it can identify a merge target, and `kg_consolidate` consumes that structure before falling back to legacy reasoning-text parsing. `kg_consolidate` also ignores unresolved non-merge correction history instead of reporting it as failed dedup work.
- **KG dedupe planner alias awareness**: `kg_dedupe_plan` now excludes entities already marked as aliases/canonicalized from candidate queries, so post-apply duplicate-group counts reflect remaining real work instead of re-counting already-merged losers.
- **KG dedupe planner datetime decoding**: `kg_dedupe_plan` now string-casts `created_at` in its entity query (`type::string(created_at)`) so the planner can read SurrealDB 3.x datetime results without failing with `Expected any, got datetime`.
- **Registry test stability**: Removed brittle global-size assertions in registry/cancel tests and switched to UUID-scoped assertions to avoid cross-test interference from shared global registry state.
- **Server version consistency**: `server_info.version` now reads from `env!("CARGO_PKG_VERSION")` so MCP runtime metadata stays in sync with crate version.
- **agent_job_status robustness**: Optional `started_at`/`completed_at` values now emit `null` (not stringified `NONE`) and integration tests serialize shared DB/server access to eliminate parallel cross-test flakiness.

## [0.8.1] - 2026-03-07

### Removed

- **call_warp Tool**: Removed `call_warp` delegation tool from MCP surface. The `WarpClient` remains available in the codebase for potential future use. Federation now uses three delegation paths: `call_gem`, `call_cc`, and `call_vibe`.

## [0.8.0] - 2026-03-05

### SurrealDB 2.x → 3.x Migration

Emergency migration after `brew upgrade` installed SurrealDB 3.0.1, which could not read the v2 SurrealKV manifest format (`Unsupported manifest format version: 0`). All data (2,393 thoughts, full KG, photography namespace) preserved and restored.

### Changed

- **surrealdb Crate 2.0 → 3.0**: Updated Rust client crate across 28 source files. API changes include query response handling, `WsClient` type usage, and record ID deserialization.
- **`type::thing` → `type::record`**: Renamed across 49 occurrences in 15+ files. SurrealDB 3.x renamed this function; old name produces parse errors.
- **Schema `FLEXIBLE` Keyword**: Moved from before `TYPE` to after (e.g., `TYPE option<object> FLEXIBLE`). SurrealDB 3.x reversed the keyword order.
- **Schema `DEFINE FIELD id` Removal**: Removed `DEFINE FIELD id ON TABLE ... TYPE record<...>` from `correction_events` and `agent_exchanges` tables. SurrealDB 3.x rejects explicit `record<table>` type on `id` fields.
- **SurrealDB Plist**: Added `--user root --pass root` to `com.legacymind.surrealdb.plist` ProgramArguments for fresh instance authentication after data directory wipe and reimport.
- **Memory Injection Retrieval Path**: Switched `inject_memories` from Rust-side cosine scoring over fetched raw embeddings to DB-side scoring using `vector::similarity::cosine(...)` with scalar result fields only (`id`, `name`, `entity_type/description`, `similarity`). This reduces payload size, avoids SurrealDB 3.x WS decode edge cases, and keeps threshold/floor behavior unchanged.
- **think Debug Telemetry**: Added step-level timing logs around `think` execution (`continuity`, `CREATE`, embedding call, `UPDATE`, framework update, candidate fetch, injection persist) to make future runtime stalls diagnosable without invasive tracing.

### Fixed

- **REMini launchd Environment**: Added explicit PATH environment variable to `dev.legacymind.remini.plist` to ensure homebrew binaries (`/opt/homebrew/bin`) are accessible to scheduled maintenance tasks. Fixed failing `wander` (gemini CLI not found) and `health` (surreal CLI not found) tasks. Also corrected typo in `SURR_ENV_FILE` path.
- **think Tool Hang on SurrealDB 3.x**: Resolved silent `think` stalls after migration to SurrealDB 3.x. Root cause was websocket deserialization failure when memory injection fetched raw embedding arrays from KG tables (`Failed to decode fb value`). `think` now completes and returns MCP responses reliably in stdio and HTTP flows.
- **Test Compatibility with rmcp 0.16**: Updated integration/smoke tests to use `CallToolRequestParams` shape with required `meta` and `task` fields. This resolves `cargo clippy --all-targets` build failures caused by outdated request initializers.
- **Git Push Divergence Resolved**: Fixed local `master` branch divergence from `origin/master` by fetching remote changes and rebasing local commits. Successfully pushed 3 local commits integrating 1 remote commit without conflicts, resolving non-fast-forward rejection.
- **SurrealDB 3.x Query/Type Follow-ups (`search`, `wander`, `corrections`)**: Fixed missed migration issues by replacing legacy record checks in `unified_search` (`type::is::record(...)` → `meta::tb(...) IS NOT NONE`), removing `SELECT *` in `wander` in favor of explicit field projections, and string-casting datetime outputs (`created_at`, `marked_at`, `timestamp`) with `type::string(...)` to resolve runtime decode errors like `Expected any, got datetime`.
- **`search` Chain-ID Hang**: Fixed `unified_search` stalling when `chain_id` was provided. Root cause was repeated inline subqueries (`SELECT ... FROM thoughts WHERE chain_id = $cid`) inside entity/relationship/observation filters. Search now resolves chain thought IDs once per request and reuses a bound `$chain_ids` list, eliminating the pathological query plan and returning promptly.

### Migration Process

1. Downloaded SurrealDB 2.6.3 binary to export existing data with `--v3` compatibility flag
2. Exported all 6 databases across 4 namespaces (surreal_mind, photography, legacymind, test)
3. Backed up v2 data directory, started fresh 3.0.1 instance
4. Imported smaller databases directly, fixed consciousness export (removed `DEFINE FIELD id TYPE record<>` lines, changed `SCHEMAFULL` → `SCHEMALESS` for tables with extra fields)
5. Updated surrealdb Rust crate, fixed all compile errors, replaced `type::thing` → `type::record`
6. Fixed `FLEXIBLE` keyword positioning in schema.rs
7. Resolved think tool hang caused by WS deserialization of raw embedding arrays

### Removed

- **call_codex Tool**: Removed `call_codex` delegation tool from MCP surface. The `CodexClient` remains available in the codebase for potential future use. Federation now uses three delegation paths: `call_gem`, `call_cc`, and `call_warp`.
- **Dead Directories**: Removed `models/` (260MB BGE model weights - Candle/local embedding support was removed), `.idea/` (JetBrains), `.aiassistant/` (JetBrains AI), `.agent/` (Gemini rules), `.venv-convert/` (46MB one-off Python venv).
- **Stale Files**: Removed `.rc-prep` (September 2024 RC marker), `docs/QUICKSTART.md` (referenced old tool names).
- **One-off Scripts**: Cleaned `scripts/` - removed `check_chain_id_usage.py`, `diagnose_entity_data.py`, `test_chain_id.py`, `test_kg.py`, `test-sleep-gemini.sh`, `package.json`, and `migration/` subproject (1.3GB target dir). Photography scripts (`backup_database.py`, `cleanup_duplicates.py`, `investigate_duplicates.py`) moved to photography-mind.
- **TUI Binary**: Removed `smtop` dashboard; rely on `/metrics` or external observability.

### Removed

- **Photography Scripts**: Deleted `scripts/import_skater_requests.py` and `scripts/validate_contacts.py` - these belong in photography-mind, not surreal-mind.
- **Deprecated Shell Tests**: Removed 8 shell test scripts that referenced deprecated tools (`think_search`, `think_convo`): `simple_test.sh`, `test_with_data.sh`, `debug_search_low_thresh.sh`, `debug_search.sh`, `test_search.sh`, `test_mcp_comprehensive.sh`, `test_detailed_mcp.sh`, `test_simplified_output.sh`. Kept 4 valid scripts: `test_simple.sh`, `test_mcp.sh`, `test_stdio_persistence.sh`, `check_version.sh`.

### Changed

- **Tool File Naming**: Renamed `delegate_gemini.rs` → `call_gem.rs` and `detailed_help.rs` → `howto.rs` for consistency with tool names. Handler methods also renamed (`handle_delegate_gemini` → `handle_call_gem`, `handle_detailed_help` → `handle_howto`).
- **Memory Injection Retrieval Path**: Switched `inject_memories` from Rust-side cosine scoring over fetched raw embeddings to DB-side scoring using `vector::similarity::cosine(...)` with scalar result fields only (`id`, `name`, `entity_type/description`, `similarity`). This reduces payload size, avoids SurrealDB 3.x WS decode edge cases, and keeps threshold/floor behavior unchanged.
- **think Debug Telemetry**: Added step-level timing logs around `think` execution (`continuity`, `CREATE`, embedding call, `UPDATE`, framework update, candidate fetch, injection persist) to make future runtime stalls diagnosable without invasive tracing.
- **Repository Hygiene Pass**: Completed `cargo fmt` and `cargo clippy --all-targets` with clean results after migration fixes. Also removed temporary debug/test artifacts created during incident triage.
- **Documentation Sync**: Updated `README.md`, `docs/AGENTS/{arch,setup,connections}.md`, and `docs/DEPENDENCIES.md` to reflect SurrealDB 3.x baseline, current tool naming (`call_gem`), and current runtime environment variable expectations.
- **call_codex Tool**: Refactored to synchronous execution - returns response directly in MCP call instead of async job queue. Removed worker polling pattern for simpler, more reliable operation.
- **CodexClient**: Added `--skip-git-repo-check` flag for execution in any directory. Fixed NDJSON parser to handle Codex's `item.aggregated_output` format and `thread_id` extraction.
- **Codex Model Configuration**: Default model and available models dropdown now read from environment variables (`CODEX_MODEL` and `CODEX_MODELS`) instead of hardcoded - no rebuild required to change model list.
- **call_gem Native Resume**: Added `resume_session_id` and `continue_latest` parameters. Gemini CLI auto-saves all sessions - use `continue_latest: true` for `--resume` (latest) or `resume_session_id` for specific session.

### Added

- **test_notification Tool**: New tool for testing MCP notification capabilities (`peer.notify_logging_message`). Sends a logging message with a specified level to the client.
- **call_cc Tool**: New tool for delegating tasks to Claude Code CLI. Synchronous execution with `--output-format stream-json`. Model selection via `ANTHROPIC_MODEL`/`ANTHROPIC_MODELS` env vars. Supports `--resume <id>` and `-c` (continue latest) for session management.
- **call_warp Tool**: New tool for delegating tasks to Warp CLI. Multi-model access through single interface: Claude (haiku/sonnet/opus), GPT-5/Codex (with reasoning levels: -low/-medium/-high/-xhigh/-max), and auto modes (auto/auto-efficient/auto-genius). One-shot executor—no resume/session support. Required: `prompt`, `cwd`. Optional: `model`, `timeout_ms`, `max_response_chars`, `task_name`, `mode`.
- **Observe Mode**: All `call_*` tools support a `mode` parameter with values `"execute"` (default) or `"observe"`. In observe mode, the delegated agent is instructed to analyze and report only—no file modifications. (Note: `call_codex` was later removed.)
- **Response Truncation**: Added `max_response_chars` parameter to all `call_*` tools (default 100KB). Prevents oversized responses from overwhelming clients. Set to `0` for no limit.
- **Federation Context**: All `call_*` tools now prepend a `[FEDERATION CONTEXT]` header to prompts, informing the delegated agent it's being invoked as a subagent by surreal-mind MCP.

### Fixed

- **delegate_gemini Worker**: Fixed job stealing bug - worker now filters by `tool_name = 'delegate_gemini'` to prevent claiming jobs from other tools like call_codex.
- **CodexClient Session Resume**: Fixed CLI argument ordering per v0.79.0+ docs. Resume is a subcommand of exec with strict ordering: `codex exec resume <id> "prompt" [flags]`. Prompt now placed before flags.
- **Search NULL vs NONE**: Fixed `unified_search.rs` to use `IS NOT NONE` instead of `IS NOT NULL` for SurrealDB 2.x compatibility. Thoughts with uninitialized embeddings were causing `vector::similarity::cosine()` errors.
- **REMini Timeout**: Added `--timeout` flag (default 3600s = 1 hour per task). Uses spawn + polling instead of blocking `.output()` to prevent runaway tasks from hanging indefinitely.
- **wander ID normalization**: `wander` now accepts `entity:` / `observation:` / `thought:` aliases and validates record existence before querying, preventing `meta::id()` type errors when starting from entity IDs.
- **wander meta::id() serialization**: Fixed critical bug where `wander` tool failed with "invalid type: enum" serialization error. Updated all SQL queries to properly use `meta::id(id) as id` to convert Thing objects to strings, ensuring JSON serialization compatibility. This affects 12 query statements across all wander modes (random, semantic, meta, marks).

### Removed

- **PersistedAgent Wrapper**: Removed fake memory/statefulness layer that concatenated previous exchanges into prompts. The `persisted.rs` module and related `agent_exchanges`/`tool_sessions` DB writes are removed.
- **call_codex Async Worker**: Removed background job queue pattern in favor of synchronous execution.
- **call_gem Async Worker**: Removed background job queue pattern in favor of synchronous execution. Tool now returns response directly.

---

### Added

- **call_codex Tool**: Added Codex CLI delegation with async job tracking, resume options, and stream metadata capture.
- **Graceful Embedding Degradation**: Thoughts are now saved before embedding, preventing data loss when the OpenAI embedding API is unavailable. Failed embeddings can be retried later via `maintain embed_pending`. Adds `embedding_status` field to thoughts table (values: `pending`, `complete`, `failed`).
- **Phase 1: Schema & Data Model**: Implemented the initial schema for the REMini & Correction System, adding Mark fields (`marked_for`, `mark_type`, `mark_note`, `marked_at`, `marked_by`) to thoughts, kg_entities, and kg_observations tables, and creating the CorrectionEvent table with fields for provenance tracking.
- **Phase 2: rethink Tool - Mark Mode**: Implemented the `rethink` MCP tool with mark creation capability.
- **Phase 3: wander --mode marks**: Added capability to surface and filter marks in the `wander` tool.
- **Phase 4: rethink Tool - Correct Mode**: Implemented full correction provenance with CorrectionEvent tracking and derivative cascading.
- **Phase 5: gem_rethink Process**: Created a specialized binary for autonomous background correction processing by Gemini.
- **Phase 6: REMini Wrapper**: Implemented a unified maintenance orchestrator (`remini` CLI) to manage background tasks.
- **Phase 7: Forensic Queries**: Added `--forensic` flag to the `search` tool to expose correction chains and provenance data.
- **Phase 8: Confidence Decay**: (Foundation) Added confidence fields and decay tracking logic to the core schemas.
- **Phase 9: Corrections Tool**: Integrated the standalone `corrections` tool and mapped it into the `maintain` surface.

### Removed

- **Scalpel Tool**: Fully removed the scalpel tool and local delegation infrastructure to free port 8111 and improve reliability. Scalpel was unreliable on the 32GB Studio; use remote `call_gem` for delegation instead.
- **Scalpel Environment Variables**: Removed all scalpel-related environment variables (`SURR_SCALPEL_MODEL`, `SURR_SCALPEL_ENDPOINT`, `SURR_SCALPEL_MAX_TOKENS`, `SURR_SCALPEL_TIMEOUT_MS`) from `.env` and `.env.example` files.

### Changed

- **Thought Persistence**: Avoid writing empty embeddings during initial thought creation so HNSW indexing doesn't reject the record; embedding is only set after a valid vector is produced.
- **Thought Schema**: Set `thoughts.embedding` to `option<array<float>>` with `DEFINE FIELD OVERWRITE` so the migration applies on startup; initial create uses `embedding: NONE` to pass schema validation before embedding is computed.
- **Thought Create Validation**: Thought creation now returns `meta::id` and checks the response to surface DB errors instead of failing silently.
- **Scalpel Configuration**: Removed hardcoded default model from `src/clients/local.rs`. The `SURR_SCALPEL_MODEL` environment variable is now **mandatory**. This prevents silent failures/mismatches by forcing explicit configuration in `.env`.
- **Documentation**: Added Scalpel configuration section to `.env.example`.
