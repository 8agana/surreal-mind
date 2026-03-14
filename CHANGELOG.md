## [0.8.2] - 2026-03-12

### Fixed

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
