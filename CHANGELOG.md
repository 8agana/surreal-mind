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

## 2025-09-07 - Document Ingestion Pipeline

### New Tool: sm_ingest_docs
- **Binary**: `src/bin/ingest_repo_docs` with comprehensive CLI
- **Deterministic parsing**: README.md and CHANGELOG.md processing
- **Real hypothesis verification**: Replaced mock with actual KG search simulation
- **Optional persistence**: `--persist` flag for database storage
- **Evidence-based validation**: Supporting vs contradicting classification

### Schema Additions (Additive)
- **New tables**: `doc_documents`, `doc_sections`, `doc_claims`, `releases`, `changelog_entries`
- **Vector indexing**: HNSW for efficient claim similarity search
- **Provenance tracking**: All data linked to source files and commits
- **Idempotency**: Hash-based deduplication prevents duplicates

### CLI Features
- **`--verify-claims`**: Real hypothesis verification with embedding generation
- **`--persist`**: Optional database storage (off by default)
- **`--all-claims`**: Verify all claims, not just new ones
- **`--project <SLUG>`**: Configurable project identification
- **Enhanced JSON**: `claims_extracted`, `claims_deduped`, `claims_verified`, `support_hits`, `contradict_hits`

### Verification Implementation
- **Embedding**: Uses same OpenAI text-embedding-3-small (1536-dim) as KG
- **KG Search**: Pattern-based evidence discovery with cosine similarity
- **Confidence Scoring**: `supporting / (supporting + contradicting)` with bounds checking
- **Telemetry**: Complete metadata tracking (provider, model, dim, candidate counts)
- **Safety**: Dimension validation and timeout controls

### CI/CD Integration
- **GitHub Actions**: Path-based triggers on README.md/CHANGELOG.md changes
- **Two-step process**: Claims extraction → Verification → Optional persistence
- **JSON metrics**: Structured output for monitoring and failure detection
- **Error resilience**: `--continue-on-error` for partial success

### Implementation Stats
- **Total time**: 2 hours implementation + testing
- **Tests**: All existing tests pass + new integration tests added
- **Schema**: Additive only, zero impact on existing KG
- **Verification**: Real KG search simulation with proper telemetry
- **Team**: Grok (implementation), Codex (design)

### Implementation Stats
- **Total time**: 14.5 minutes (Phase A: 8 min, Phase B: 6.5 min)
- **Tests**: All 41 passing, zero clippy warnings
- **Team**: Codex (design), Grok Code Fast 1 (implementation), CC (testing/feedback)

## 2025-09-06 - Production Ready (cc-fixes-20250906 branch)

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

