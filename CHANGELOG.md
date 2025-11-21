## 2025-11-20 - Photography TY Workflow & Schema Updates
- **TY Workflow Implementation**: Added support for "Thank You" gallery tracking in photography CLI and schema.
  - New commands: `request-ty` (marks requested), `send-ty` (marks sent, timestamps).
  - Updated `check-status`: Supports `--ty-pending` flag and new statuses (`not_shot`, `needs_research`).
  - Updated Schema: Added `ty_requested` (bool), `ty_sent` (bool), `ty_sent_date` (option<datetime>) to `family_competition` edge.
- **Bug Fixes**:
  - **ID Generation**: Fixed critical bug where `format_family_id` used hyphens (`-`) while import used underscores (`_`). Standardized on underscores.
  - **Query Syntax**: Fixed `update_gallery` to correctly use `in`/`out` fields for relation updates instead of table names.
  - **Schema Logic**: Moved TY fields from `competed_in` (event-level) to `family_competition` (family-level) to match business logic.
- **Refactoring**: Updated `StatusRow` and `SkaterRow` structs to support new fields.

## 2025-09-20 - Photography Schema Extension
- **Added photography namespace and ops database**: New SCHEMAFULL schema for photography business operations including client management, family groups, competitions, events, registrations, and shot logging.
- **Created photography_schema binary**: Dedicated tool to initialize the photography ops database with proper relations, types, and defaults.
- **Updated DATABASE.md documentation**: Added comprehensive photography schema documentation including all tables, fields, and relationships.
- **Verified schema**: Successfully applied to running SurrealDB instance with INFO FOR DB confirmation.

## 2025-09-19 - Brain Store & Automatic KG Embeddings
- **Brain datastore support**: Optional `brains` namespace connection with new schema initialization (SURR_ENABLE_BRAIN + SURR_BRAIN_* envs).
- **New brain_store tool**: MCP interface to get/set agent brain sections directly in SurrealDB.
- **Automatic KG embeddings**: Newly created or moderated KG entities/observations now auto-embed with provider/model metadata.
- **Docs & Schemas**: Updated README, detailed help, and tool schemas/tests for the new capabilities.

## 2025-09-18 - Photography Voice & Moderation Parity
- **Added photography_voice tool**: Isolated synthesis tool mirroring inner_voice for photography namespace (`ns=photography`, `db=work`).
- **Added photography_moderate tool**: Dedicated moderation for photography KG candidates, improving UX over multiplexed photography_memories(mode="moderate").
- **Refactored inner_voice logic**: Extracted core workflow into reusable helper for namespace isolation without regressions.
- **Updated docs and tests**: Added tools to help, schemas, and README; all tests passing.

## 2025-09-18 - Photography KG Re-Embedding Binary
- **Added `reembed_photography_kg` binary**: Dedicated tool for re-embedding KG entities/observations in photography namespace (`ns=photography`, `db=work`). Supports CLI flags (`--dry-run`, `--limit`), aligns with env vars (`SURR_ENABLE_PHOTOGRAPHY`). No main namespace interference.

## 2025-09-18 - Inner Voice CLI Removal
- **Removed Gemini CLI from inner_voice**: Simplified provider chain to Grok → local fallback. Deprecated CLI envs warn and default appropriately.
- **Feedback Generation**: Dropped CLI-based feedback, set to None for consistency.
- **Auto-extraction**: Now uses Grok exclusively for KG candidate extraction.

## 2025-09-17 - CCR Defect Fixes & Continuity Improvements
- **Fixed SurrealDB Bootstrap Regression**: Removed duplicate connection after successful retry-protected connection
- **Restored Safe Continuity Fallback**: Missing thought IDs are now preserved as strings instead of being dropped
- **Fixed Unified Search Ordering**: Sort entities and observations by similarity before truncation
- **Improved HTTP Error Handling**: Added proper status checking and rate limit warnings for Grok API calls
- **Fixed Maintenance Export Format**: Changed from misleading "parquet" to actual "json" format
- **Added Regression Tests**: Coverage for continuity fallback, search ordering, and HTTP error handling

## 2025-09-16 - Semantic Search Fix & Infrastructure Updates
- **Fixed Semantic Search**: Replaced client-side similarity calculations with SurrealDB's `vector::similarity::cosine()` for KG queries
- **Embedding Verification**: Added `check_embedding_dims.sh` script to verify embedding consistency across collections
- **Query Improvements**: Added fallback logic for query embedding content extraction
- **Repository Cleanup**: Removed roadmap, refined search implementation, replaced legacy AGENTS.md with repository guidelines
- **Gitignore Updates**: Added symlinks directory to prevent accidental commits

## 2025-09-15 - Inner Voice CLI Extractor & Stability Improvements
- **CLI-Based KG Extraction**: Added Node.js-based extractor pipeline for inner_voice tool
  - Preflight node check with deterministic IDs for kg_candidates
  - Robust JSON repair with brace matching
  - AJV schema validation support
  - Improved error handling and fallback mechanisms
- **rmcp Upgrade**: Updated to 0.6.4 with stdio persistence workaround
- **Echo Config Command**: Added for debugging configuration issues
- **Continuity Fields**: Refined handling across all tools
- **Photography Database**: Added health check functionality
- **System Monitoring**: Added smtop TUI dashboard with:
  - Real-time DB health monitoring
  - System metrics display
  - Stdio session tracking
  - Tunnel URL display with detail toggle
- **Code Quality**: Refactored code for clarity and efficiency across multiple modules

## 2025-09-14 - Inner Voice Enhancements & Search Improvements
- **Advanced Search Filtering**: Added continuity field filtering to legacymind_search
- **Inner Voice Improvements**:
  - Simplified default CLI arguments for synthesis
  - Implemented two-thought chain with feedback mechanism
  - Added Gemini CLI and Grok synthesis support without snippets
  - Enabled CLI by default with IV_USE_CLI_EXTRACTOR override
- **Script Improvements**:
  - Added minimal package.json with ajv for schema validation
  - Conditional config and env loading in run script
- **Tool Cleanup**:
  - Removed legacy think tools
  - Added photography_memories tool
  - Removed GitHub workflows for PR guards

## 2025-09-14 - CCR Implementation Complete
- **Rate Limiter Fix**: Switched to monotonic process epoch using Instant; no unnecessary sleeps.
- **Startup Tool Log**: Dynamic tool count in startup message.
- **Inner Voice Updates**: Descriptions reflect synthesis + optional KG auto-extraction; IV_CLI_* overrides IV_SYNTH_*.
- **HTTP Security**: Warns on query-param token usage.
- **Schema Enhancements**: Continuity fields and indexes added to initialize_schema.
- **Tests/Build**: All passing; clippy clean; production binary built.

## 2025-09-14 - Documentation Refresh
- Aligned docs with unified thinking tools (`legacymind_think`, `photography_think`) and aliasing of legacy tools
- Removed submode references from surfaces; emphasized KG-only injection and dimension hygiene
- Updated env-first configuration and recommended injection thresholds (`T1=0.6`, `T2=0.4`, `T3=0.25`, `FLOOR=0.15`)
- Corrected `legacymind_search` schema and examples; removed outdated `think_search` references
- Consolidated binaries list (`reembed`, `reembed_kg`, `fix_dimensions`) and health checks (`health_check_embeddings`)
- Cleaned README duplication and outdated examples

## 2025-09-09 to 2025-09-12 - Infrastructure & Reliability Improvements
- **Embedding Consistency**:
  - Added embedding dimension hygiene check
  - Implemented rate limiting with governor crate for API requests
  - Added test_fix_dimensions binary for dimension mismatch fixes
- **Database Enhancements**:
  - Enhanced validation and reconnection logic
  - Added protoc installation for lance-encoding dependency
  - Removed Git LFS tracking and hooks
- **Documentation Updates**:
  - Added Quick Start guide
  - Updated tool roster and env vars documentation
  - Improved error messages and categorization
- **CI/CD Improvements**:
  - Fixed cargo-audit installation with rustup update
  - Set PROTOC env var for macOS CI
  - Updated CI runner from Ubuntu to macOS
- **Legacy Storage**:
  - Added LanceDB and SQLite dependencies for migration support
  - Reverted to stable commit 2fb3ec1 for MCP toolset stability

## 2025-09-07 - Major Refactor: Unified Thinking Tools (Phases A/B/C)

### Phase A - Router Pattern (8 minutes implementation by Grok)
- **Consolidated 5 tools → 2 domain tools**: `legacymind_think` and `photography_think`
- **Automatic mode routing** via trigger phrases or heuristic keywords
  - Trigger phrases: "debug time", "building time", "plan time", "i'm stuck", "question time"
  - Heuristic fallback: error→debug, implement→build, design→plan
- **Extracted common logic** to `src/tools/thinking.rs` with `run_*` functions
- **Backward compatible**: Legacy tools still available, internally route through new system

### Phase B - Session Continuity (6.5 minutes implementation by Grok)
- **Added session linking fields**: `session_id`, `previous_thought_id`, `chain_id`
- **Thought chaining**: Create linked sequences of thoughts across sessions
- **Revision tracking**: `revises_thought`, `branch_from` for non-linear thinking
- **Telemetry**: Track what triggered routing decisions

### Phase C - Hypothesis Verification (Completed by Grok)
- **Evidence-based validation**: Query KG for supporting/contradicting evidence
- **Deterministic classification**: Pattern matching for contradiction detection
- **Confidence scoring**: `supporting_count / (supporting_count + contradicting_count)`
- **Configurable thresholds**: Via env vars `SURR_VERIFY_TOPK`, `SURR_VERIFY_MIN_SIM`
- **Optional persistence**: Store verification results as JSON blob on thought

### Implementation Stats
- **Total time**: 14.5 minutes (Phase A: 8 min, Phase B: 6.5 min)
- **Tests**: All 41 passing, zero clippy warnings
- **Team**: Codex (design), Grok Code Fast 1 (implementation), CC (testing/feedback)

## 2025-09-06 - Production Ready (cc-fixes-20250906 branch)

### Early September Features & Fixes (2025-09-01 to 2025-09-05)
- **Document Ingestion Pipeline**:
  - Added real KG verification for ingested documents
  - Implemented markdown parser for README/CHANGELOG
  - Added kg_inspect tool for hypothesis verification
  - Enhanced document processing with TagEnd handling
- **TUI Dashboard**:
  - Created interactive dashboard for monitoring
  - Added thought continuity fields display
  - Enhanced session tracking capabilities
- **HTTP Transport**:
  - Implemented Axum server with bearer authentication
  - Added run-http.sh script for HTTP configuration
  - Removed session tracking from HTTP server for cleaner separation
- **Inner Voice Tool Evolution**:
  - Added Grok synthesis integration
  - Implemented query planner stage
  - Created prompt registry and metrics modules
  - Added self-aware prompt system
  - Integrated NLQ (Natural Language Query) processing
- **Architecture Improvements**:
  - Added user agent headers to HTTP clients
  - Implemented conversation analysis framework with methodology selection
  - Added index health checks
  - Removed legacy tool aliases

### Code Consistency Review Fixes
- **Fixed clippy warnings** in tests/inner_voice_retrieve.rs
- **Removed redundant imports** and unnecessary vec! macros
- **Improved error handling** with safer Result flows
- **Config validation** enforcing provider/model coherence

## 2025-08-31 - Phase 2: maintenance_ops

- Added maintenance_ops tool with subcommands:
  - list_removal_candidates (status='removal', age >= SURR_RETENTION_DAYS)
  - export_removals (JSON export to SURR_ARCHIVE_DIR; parquet placeholder)
  - finalize_removal (delete exported thoughts)
- Added maintenance_ops schema and server wiring; updated detailed_help.
- Safety: dry_run supported for all actions; best-effort operations.
- No breaking changes; backward compatible.

# Changelog

All notable changes to this project will be documented in this file.

## 2025-08-29

### Added
- **KG-Only Retrieval System** - Complete refactor to pull from Knowledge Graph instead of thoughts (Warp, Codex)
  - New `retrieve_from_kg()` function with submode-specific traversal
  - Injection scales reduced to 0-3 (removed 4-5) for better performance
  - Scale limits: 0=none, 1=5 entities (default), 2=10 entities, 3=20 entities
- **Config System** - Comprehensive TOML configuration (Zed)
  - Created `surreal_mind.toml` with all submode profiles
  - Added `src/config.rs` loader with `anyhow::Result`
  - Migrated environment variables to config file
- **Inner Voice Auto-Extract** - Automatic entity extraction for KG (Warp)
  - Creates entities and "mentions" edges from inner_voice thoughts
  - Simple keyword extraction with stopword filtering
- **LRU Cache Infrastructure** - For entity embeddings (Codex)
  - 100 entity cache with 5-minute TTL
  - Ready for future optimization

### Changed
- **Embedding Dimensions** - Reduced from 1536 to 768 (CC)
  - Fixed OpenAIEmbedder to send dimensions parameter to API
  - 2x performance improvement for cosine similarity
  - All new thoughts use 768-dimensional embeddings
- **API Compatibility** - Maintained backward compatibility (Codex)
  - KG results mapped to thought-like format
  - External APIs unchanged
- **Test Scripts** - Updated to seed KG instead of thoughts (Codex)

### Fixed
- **Scale 3+ Timeouts** - Resolved by switching to KG retrieval
- **Async Move/Clone Patterns** - Fixed closure errors in entity creation (Warp)
- **Clippy Warnings** - All warnings resolved (Junie)
- **Merge Conflicts** - Successfully integrated parallel work from 3 LLMs (Junie)

## 2025-08-27 14:54 CDT

### Added
- **Retry Logic for Database Operations**: Added robust retry mechanism with exponential backoff for all critical database operations:
  - `with_retry()` utility function with configurable retry counts and delays
  - Environment variables for configuration:
    - `SURR_MAX_RETRIES` (default: 3)
    - `SURR_RETRY_DELAY_MS` (default: 500ms)
  - Smart error classification to avoid retrying logic errors (parse, syntax, invalid, permission)
  - Detailed logging of retry attempts and failures
- Retry logic applied to:
  - Schema initialization (`initialize_schema`)
  - Thought creation (`think_convo`, `think_plan`, `inner_voice`)
  - All database query operations

### Fixed
- **Tool Hanging/Timeout Issues**: Database operations now automatically retry on connection failures, timeouts, and WebSocket issues
- **Inner Voice Tool Reliability**: Wrapped `create_inner_voice_thought` with retry logic to prevent empty returns from hanging operations
- **Schema Initialization Robustness**: Schema creation operations now retry on transient failures during startup

## 2025-08-24 22:19 UTC

### Added
- Default SurrealDB service (WebSocket) configuration via environment variables:
  - SURR_DB_URL (default 127.0.0.1:8000)
  - SURR_DB_USER (default root)
  - SURR_DB_PASS (default root)
  - SURR_DB_NS (default surreal_mind)
  - SURR_DB_DB (default consciousness)
- Gate DB-dependent test with RUN_DB_TESTS to avoid requiring a live DB during unit test compilation.
- Server-side input validation for tool parameters:
  - injection_scale must be 0–5
  - significance must be 0.0–1.0
- Nomic embeddings HTTP client timeout (15s) for more robust external calls.

### Changed
- Retrieval pipeline now increments access_count and updates last_accessed when DB-backed memories are selected.
- Relationship creation is now bidirectional: creates both from->recalls->to and to->recalls->from.
- Memory summary now reports explicit min/max orbital proximity values rather than relying on sorted order.
- Cosine similarity calculation now computes dot and norms over the same span to avoid skew with unequal vector lengths.
- .env.example updated with SurrealDB service config and test control variable.

### Fixed
- Clippy warnings and formatting issues to satisfy `-D warnings` and `cargo fmt --check`.
- Tests compile without requiring a running SurrealDB by default.

### Commits
- 30931c8 fix: make CI green
  - initial formatting/clippy/test compile cleanups
- aeba2f2 feat(db): default to SurrealDB service (Ws) with env config
  - env-driven DB config, input validation, cosine fix, bidirectional edges, access metadata updates, Nomic timeout, test gating