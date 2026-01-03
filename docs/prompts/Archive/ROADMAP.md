Surreal Mind MCP — Development Roadmap (2025-09)

This roadmap captures concrete next steps to evolve Surreal Mind from a solid MCP prototype into a robust, extensible service. It is organized by priority bands with crisp acceptance criteria so progress is measurable.

Status summary
- Transport: stdio is default and stable; HTTP transport exists with bearer auth, CORS, SSE keepalive, and metrics scaffold.
- Tooling: 10 tools exposed and implemented; list_tools is canonical; legacy think_* search variants removed in favor of unified surfaces.
- Data: SurrealDB schema is initialized at startup; LRU cache for thoughts; knowledge graph edges and retrieval exist with injection scaling and orbital weights.
- Embeddings: OpenAI provider implemented with simple rate limiting/retries; dimension hygiene checks are in place; typed config is loaded from TOML + env.
- Tests: Unit tests exist inside server; integration tests and protocol scripts present; DB-backed tests are opt-in via RUN_DB_TESTS.

Now (stabilization and DX)
1) Close doc drift between code and docs
- Update README to reference the current tool set and transports (no legacy search tool names).
- Document HTTP transport usage (path, auth, sample curl).
- Acceptance: make ci passes; README contains current tool names; quick-start for HTTP verified with curl.

2) Add a re-embedding maintenance flow (operator tool)
- Introduce a maintenance tool action to re-embed entities/observations when dimension/model changes.
- Persist embedding metadata snapshot (provider, model, dim) on each record if not already consistent; use it to target re-embed.
- Acceptance: RUN_DB_TESTS=1 scenario creates mixed-dim rows, re-embed tool reconciles them; server check_embedding_dims passes afterwards.

3) Improve error surfaces and tracing
- Wrap database and HTTP errors with context at boundaries; ensure rmcp::ErrorData messages are concise and user-safe.
- Add structured tracing fields for request ids, tool names, and durations; keep MCP_NO_LOG respected.
- Acceptance: Simulated failures produce actionable logs without leaking sensitive content; clippy with -D warnings clean.

4) Expand test coverage for tools and HTTP
- Add integration tests for unified_search and inner_voice parameter validation (no DB dependency by default).
- Add HTTP endpoint smoke tests behind a feature or env gate.
- Acceptance: cargo test --all shows new tests; RUN_DB_TESTS enables DB checks but default run remains offline.

Next (capabilities and performance)
5) Retrieval quality and performance
- Implement adjustable scoring that mixes cosine similarity with orbital signals (recency/access/significance) in a single ranking function.
- Add cache warming and size tuning via config; expose metrics on LRU hit rate.
- Acceptance: Configurable weights in TOML; unit tests cover scoring behavior; metrics endpoint reports cache stats.

6) Knowledge Graph moderation pipeline hardening
- Define a status lifecycle for KG candidates (staged→approved/rejected) with timestamps and actor info.
- Add indexes needed for frequent queries; ensure moderation tools enforce quotas and limits.
- Acceptance: Schema migration adds indexes; moderation list queries remain <150ms on 10k nodes in local tests.

7) Embedding provider flexibility
- Optional local provider (candle/BGE) behind a feature or config when no OpenAI key; keep FakeEmbedder only for tests.
- Pluggable provider trait already exists—add provider selection docs and sharp errors when misconfigured.
- Acceptance: With OPENAI_API_KEY missing, local provider path can run tests; dimensions validated per model.

8) HTTP transport polish and observability
- Finalize SSE/stream timeouts and back-pressure behavior.
- Add basic/bearer auth middleware tests; add Prometheus-friendly metrics mode with request/latency counters.
- Acceptance: ./src/http.rs metrics_mode="prom" exposes /metrics; wrk shows stable p50/p95 under moderate load.

Later (feature completeness and scale)
9) Federated domains and multi-tenant isolation
- Solidify secondary DB connection (photography) patterns; make domain selection first-class in tool params.
- Optional: move per-domain config into TOML arrays and validate at startup.
- Acceptance: Domain list endpoint (maintenance_ops) returns configured domains; tools operate per-domain cleanly.

10) Schema versioning and migrations
- Add a schema_version table/record and a lightweight migrator for indexes/fields.
- Provide a maintenance tool action to migrate between versions safely.
- Acceptance: migration from v1→v2 adds needed indexes without downtime; recorded in schema_version.

11) Safety, quotas, and rate limits
- Per-tool and per-client quotas (env-configurable) with friendly error messages when exceeded.
- Backoff/limit for embedding calls (already basic) upgraded to token-bucket; expose counters.
- Acceptance: Tests simulate quota exceedance; HTTP returns 429 with Retry-After when applicable.

12) Developer ergonomics and CI
- make ci remains green; add cargo deny and udeps; add minimal GitHub Actions matrix with fmt+clippy+tests.
- Add ./scripts/dev_db.sh and ./scripts/dev_run.sh helpers.
- Acceptance: New checks pass locally; dev scripts speed up local cycle.

Cross-cutting technical notes
- Keep list_tools authoritative; avoid duplicating tool names in log strings.
- Gate anything service-external (DB, HTTP) in tests using env vars so default CI is offline.
- Prefer parameter structs with serde validators; clamp ranges and provide clear MCP error messages for users.
- For embeddings, always store metadata (provider, model, dim) next to vectors to simplify audits and migrations.

Suggested backlog (granular tasks)
- [ ] README: replace legacy tool names and add HTTP examples.
- [ ] maintenance_ops: add action reembed_all { target: memories|thoughts|both, dry_run?: bool }.
- [ ] Server: store embedding_metadata on new writes (if missing) and backfill on first read.
- [ ] Tests: unit tests for unified_search param clamps; inner_voice default behavior.
- [ ] HTTP: add /metrics (basic) and a prometheus mode.
- [ ] Retrieval: new rank(score) = w_sim*cos + w_orbital*orbital_score; config-driven weights.
- [ ] Schema: add indexes for common lookups (status, created_at, subject/object ids).
- [ ] Provider: optional candle path with 384-dim BGE; document dimensionality mapping.
- [ ] Quotas: config knobs + middleware; return MCP-friendly errors.
- [ ] CI: cargo deny + udeps + minimal GitHub Actions.

How to use this roadmap
- Treat Now items as the active sprint. Create issues for each task and link to the acceptance criteria here.
- Reassess priorities weekly based on usage pain points (e.g., misconfig/errors vs. retrieval quality).
- Keep the roadmap small and living; update after each milestone.
