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
  - Thought creation (`convo_think`, `tech_think`, `inner_voice`)
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

