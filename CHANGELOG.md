# Changelog

All notable changes to this project will be documented in this file.

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

