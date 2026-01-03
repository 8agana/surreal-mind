# Surreal Mind MCP — Code Cleanup and Improvement Plan
Date: 2025-09-02
Owner: Surreal Mind maintainers

This plan consolidates a deep scan of the repository and proposes pragmatic, incremental improvements that preserve current behavior while increasing clarity, testability, maintainability, and developer velocity.


## 0) Executive Summary
The MCP server is functional with a modularized entrypoint (src/main.rs → src/server/mod.rs) and a fairly rich typed configuration (src/config.rs). The server implements MCP handlers, SurrealDB schema bootstrap, retrieval + memory injection, a local Candle+BGE and OpenAI embedding stack, and exports tool schemas. The largest pain point is the size and responsibility density of src/server/mod.rs. There’s also inconsistency in how configuration and error types are used across modules (e.g., embeddings uses anyhow::Result while other areas use SurrealMindError), and minor doc drift with prior Nomic-centric guidance.

Codex review: Agreed with overall direction and sequencing; suggested tightening tests for injection behavior, unifying embeddings configuration sooner, reliability hardening for HTTP SQL re-embedding, adding an index checklist, security notes for future HTTP transport, helper functions to reduce SurrealQL duplication, and documenting an embeddings health check. These suggestions are incorporated below.


## 1) Current Architecture Snapshot
- Entry: src/main.rs
  - Loads typed config (Config::load), initializes tracing via config.runtime.log_level, constructs SurrealMindServer, serves over stdio (rmcp).

- Server: src/server/mod.rs (~900 lines)
  - Implements rmcp::handler::server::ServerHandler: get_info, initialize, list_tools, call_tool.
  - SurrealDB connectivity (ws), schema initialization for thoughts/recalls.
  - Retrieval/memory injection with LRU cache, cosine similarity.
  - Tool exposure via schemas in src/schemas.rs.
  - Unit tests: cosine similarity, defaults/clamping.

- Config: src/config.rs (~370 lines)
  - System/Embedding/Retrieval/Runtime typed configs with defaults and env loaders.
  - Submodes + orbital weights, runtime log level, etc.
  - Unit tests for loading and submode fallback.

- Embeddings: src/embeddings.rs, src/bge_embedder.rs
  - Embedder trait, OpenAIEmbedder with retry/backoff, Candle BGE-small local provider (via candle + hf-hub) through BGEEmbedder.
  - create_embedder() chooses provider via env (SURR_EMBED_PROVIDER, SURR_EMBED_MODEL, SURR_EMBED_DIM), uses dotenvy; logs model/dims.

- Error: src/error.rs
  - SurrealMindError domain enum with conversions to/from rmcp::ErrorData, serde_json::Error, surrealdb::Error, reqwest::Error, etc.

- Schemas: src/schemas.rs
  - JSON schemas for think_convo, think_plan/debug/build/stuck, inner_voice, search_thoughts, kg_* (create/search/moderate), detailed_help, maintenance_ops.

- Flavor: src/flavor.rs
  - Deterministic flavor tagging, with comprehensive unit tests.

- Library/utilities: src/lib.rs
  - Module exports and run_reembed() helper that re-embeds via Surreal HTTP SQL path; currently reads env directly (bypasses Config), uses embeddings::create_embedder().

- Tests: tests/
  - Rust integration tests and shell scripts for MCP flows. Some tests gate on RUN_DB_TESTS.

- Build/runtime:
  - Rust 2024 edition; Makefile with fmt/lint/ci targets; tracing logs enabled; cargo features minimal.


## 2) Key Strengths
- Clean entrypoint with typed configuration and tracing.
- Comprehensive server capability coverage for MCP (tools list stable, protocol version negotiation).
- Deterministic local embeddings option (Candle BGE) with clear provider selection and retries for OpenAI.
- Schema bootstrap for DB and LRU-based caching for retrieval.
- Good baseline of unit and integration tests and shell-based protocol scripts.


## 3) Pain Points and Risks
- Monolithic server module:
  - src/server/mod.rs couples protocol handling, DB initialization, retrieval logic, and memory injection; harder to navigate and evolve.

- Config inconsistency:
  - Some modules (lib.rs::run_reembed) read env directly instead of using typed Config.
  - Embedding selection lives in embeddings.rs and duplicates some env parsing independent of Config.

- Error/result inconsistency:
  - embeddings.rs returns anyhow::Result while the repo defines SurrealMindError. Mixed error styles reduce uniformity and complicate mapping to MCP errors.

- Documentation drift:
  - Guidelines mention Nomic/FakeEmbedder; code provides OpenAI and Candle BGE. Need to reconcile docs and code.

- Test coverage gaps:
  - Limited direct tests for memory injection selection and boundary conditions.
  - No unit tests for embeddings provider selection matrix (env permutations).

- Operational concerns:
  - Potential sensitive config exposure via info logs (embedding provider/model/dims are OK; ensure no keys).
  - Backfill run_reembed uses HTTP SQL path; ensure timeouts and partial failure reporting are sufficient.


## 4) Cleanup & Improvement Plan (Phased)
Priorities emphasize zero behavior changes initially; reorganization only.

Phase 1 — Quick Wins (1–2 days)
1. Server module organization (no behavior changes):
   - Create submodules under src/server/ and move code from mod.rs accordingly:
     - server/handlers.rs — MCP ServerHandler implementation (get_info, initialize, list_tools, call_tool)
     - server/schema.rs — initialize_schema and table/index DDL
     - server/retrieval.rs — cosine_similarity, inject_memories and retrieval helpers
     - server/types.rs — Thought, ThoughtMatch, KGMemory, parameter structs
     - server/cache.rs — LRU cache wrapper and types (if needed)
   - Keep mod.rs as a façade re-exporting pub use items; preserve public API and unit tests. Add #[path = "..."] if you prefer staged moves. ✓

2. Configuration cohesion:
   - Introduce a Config handle into SurrealMindServer state (if not already) and thread it through where env is used. ✓
   - Add a lightweight shim create_embedder_with(config: &Config) alongside create_embedder() to start removing env parsing duplication. Use the shim in new/refactored code; keep existing env-based function for compatibility. ✓

3. Error type alignment:
   - Add wrapper/helper functions in embeddings.rs to map anyhow::Error to SurrealMindError at call sites that require domain errors. Keep public signatures stable for now to avoid ripples. ✓

4. Logging hygiene:
   - Ensure no API keys are logged. Keep provider/model/dims logging. Consider reducing emojis in logs for CI readability (optional). ✓

5. Docs at a glance:
   - Add a short section to README.md (or README_ACCURATE.md) clarifying embeddings providers (OpenAI vs. Candle BGE) and env variables (SURR_EMBED_*), noting that Nomic support is currently not active in code. Include a brief “health_check_embeddings” note in README quickstart pointing to existing implementation. ✓

6. Low-risk helpers to reduce SurrealQL duplication:
   - Introduce small helper fns for repeated SurrealQL snippets (e.g., update_thought_embedding_meta) to cut copy/paste errors. Keep usage limited to new/refactored paths to avoid behavior drift. ✓

7. Index checklist (acceptance for Phase 1):
   - Ensure creation/verification of idx_thoughts_embedding_dim (or equivalent) is covered in schema.rs and validated during startup. ✓

Phase 1.5 — Reliability touch-ups (targeted, low risk)
8. run_reembed HTTP SQL hardening:
   - Add timeouts (already present) and explicit retries/backoff on failed HTTP SQL POSTs (select/update), with bounded attempts and jitter. Make retry counts configurable via env (e.g., SURR_SQL_RETRIES). Include clearer error messages and partial progress reporting. ✓

9. Security note (docs stub):
   - Add a short doc stub outlining auth expectations if an HTTP transport is introduced later (e.g., bearer tokens, TLS, and not mixing stdio and HTTP without explicit opt-in). ✓

Phase 2 — Consolidation (1 week)
10. Config unification for embeddings:
   - Extend Config to carry embedding provider/model/dim/timeout/retries.
   - Update embeddings::create_embedder() to be a thin wrapper around create_embedder_with(Config) so code uses Config as the single source of truth; retain env override via Config::load().

11. Error unification:
   - Convert embeddings.rs interfaces to return crate::error::Result<T> and map lower-level errors into SurrealMindError variants with context.

12. Tests and determinism:
   - Add unit tests for embeddings selection (matrix of SURR_EMBED_PROVIDER x OPENAI_API_KEY presence x SURR_EMBED_MODEL/dim override). Use stubs/mocks for network.
   - Add unit tests for inject_memories():
     - Mixed-KG cases and “no matches → floor fallback,” using a tiny fixture with 2–3 fake entities; assert inject_memories returns 0/5/10 by scale.
     - Empty cache/path; DB fallback limit respected; injection_scale clamp behavior; similarity thresholds; submode/tool defaulting.
     - Edge cases: zero-length embeddings; mismatched dimensions should be rejected early.

13. Server module clarity:
   - After splitting files, ensure each submodule has independent unit tests (cfg(test)).
   - Document public functions with rustdoc summarizing invariants and expected inputs/outputs.

14. run_reembed enhancements:
   - Add more detailed dry-run reporting; include timing and batch stats at end.
   - Make HTTP client/retry/backoff fully configurable via Config, centralizing defaults.

Phase 3 — Improvements (2–4 weeks)
15. DB abstraction and query layer:
   - Encapsulate Surreal queries in a small repository layer (server/db.rs) to reduce scattering of SSQL strings; add typed DTOs and conversion.

16. Performance & caching:
   - Make LRU cache size configurable; add hit/miss metrics (log at debug).
   - Consider precomputing normalized vectors to avoid per-call normalization.

17. Observability:
   - Add tracing spans for tool calls (call_tool) with tool name and sanitized params; add debug-level previews with redaction markers.
   - Add feature flag to suppress all logging for clean JSON-only streams (MCP_NO_LOG).

18. Security & robustness:
   - Validate all inbound tool params via schema + manual clamps; add explicit error codes for validation vs. internal failures (already partially implemented).
   - Scrub any potential leakage of secrets in error messages.
   - If HTTP transport is added, implement auth/TLS per the doc stub and include integration tests.

19. Documentation alignment:
   - Either reintroduce Nomic/FakeEmbedder per earlier docs or update guidelines to reflect OpenAI+Candle reality; include migration notes if behavior changes.


## 5) Suggested Directory Layout (after Phase 1)
- src/server/
  - mod.rs (facade)
  - handlers.rs
  - schema.rs
  - retrieval.rs
  - types.rs
  - cache.rs (optional)

No public API changes required; only internal code motion.


## 6) Backlog (Optional / Future)
- Feature: Pluggable tool registry to add/remove tool handlers at runtime (config-driven).
- Feature: Structured metrics output (OpenTelemetry or JSON diagnostics).
- Feature: Background maintenance tasks (periodic re-embed, index health checks).


## 7) Acceptance & Non-goals
- Phase 1 must not change behavior; only structure and documentation. All tests must continue to pass (cargo test --all; RUN_DB_TESTS-gated tests only when enabled).
- Index checklist satisfied: idx_thoughts_embedding_dim verified/created at startup.
- Avoid introducing new external service dependencies by default; keep local Candle BGE path working for offline testing.


## 8) Quickstart for Maintainers
- Run: make ci
- Local embeddings: SURR_EMBED_PROVIDER=candle cargo run
- OpenAI embeddings: export OPENAI_API_KEY=...; optional: SURR_EMBED_MODEL=text-embedding-3-small; SURR_EMBED_DIM=1536
- Start SurrealDB service for full flows: surreal start --user root --pass root --bind 127.0.0.1:8000
- Run protocol smoke: ./tests/test_simple.sh
- Health check: see README section “Embeddings health_check_embeddings” for a quick validation routine.


## 9) Tracking
Create a GitHub Project (or Kanban) with three columns for the phases above and break work into small PRs (≤300 LOC) to keep review speed high.