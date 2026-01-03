# Unified Critical Code Review (Merged) — Surreal Mind MCP

Date: 2025-09-09
Sources merged: Federation CCR (2025-09-08) + Warp (this review)
Scope: src/main.rs, src/server/mod.rs, src/config.rs, src/embeddings.rs, src/tools/*, src/http.rs, src/utils/*, tests/*, CI, docs
Guiding rules respected: env-first, no fake/deterministic embedder, SurrealDB over WebSocket, rmcp 0.6.x; warnings-as-errors; Docker avoided

Executive summary
- Overall: codebase is strong (Phases A/B/C landed, 41 tests passing), but several high-value fixes remain to eliminate footguns and drift. This merged CCR reconciles the Federation document with up-to-date code and adds net-new blockers Warp identified in the source.
- Immediate wins: fix HTTP SQL URL normalization, align MCP tool surface (legacy aliases vs unified), apply env overrides for DB ns/db/url, correct DB name default per rule, and stabilize CI to avoid spurious OpenAI calls. These are low-effort/high-impact.

Reconciliation notes (differences from Federation doc)
- LRU cache size "never set": Not applicable in current code.
  - Current implementation (server/mod.rs) bounds the cache with SURR_CACHE_MAX (default 5000) via NonZeroUsize. No unbounded leak.
- Some counts/locations (unwraps) may reflect an older snapshot; the general point stands: reduce panics in hot paths.

Unified prioritized list (blockers → high → medium → low)

Blockers (fix first)
1) HTTP SQL URL normalization broken when database_url uses ws:// or wss://
   - What/where: src/utils/db.rs — from_config builds base_url by prefixing http:// unless the string starts with http, yielding invalid "http://ws://..." when database_url is ws://…
   - Why: re-embed and any HTTP SQL path breaks under ws:// config (TOML default is ws://127.0.0.1:8000).
   - Minimal fix: normalize schemes: ws:// → http://, wss:// → https://, http(s) keep as-is, else default to http://host.
   - Tests/docs/CI: add a unit test for ws:// and http:// inputs; document scheme mapping in README.

2) MCP surface drift (tool names): legacy tools referenced in docs/tests but not listed/handled
   - What/where: src/server/mod.rs list_tools/call_tool expose legacymind_think but not think_convo/think_plan/think_debug/think_build/think_stuck; schemas/help still enumerate legacy names in places; scripts/tests expect legacy names.
   - Why: METHOD_NOT_FOUND for existing scripts; confusion between new unified tool and legacy contracts.
   - Minimal fix: either (A) re-expose legacy aliases forwarding to the unified engine (add to list_tools & call_tool) or (B) remove legacy names from schemas/docs/tests and migrate everything to legacymind_think. Pick one and make it consistent.
   - Tests/docs/CI: update tests/test_mcp_comprehensive.sh, test_detailed_mcp.sh, tool_schemas.rs, README/help.

3) Env-first drift: SURR_DB_URL / SURR_DB_NS / SURR_DB_DB overrides not applied to typed Config
   - What/where: src/config.rs Config::load reads TOML but doesn’t apply env overrides to system.database_*.
   - Why: Users expect env to win (your preference) without editing TOML.
   - Minimal fix: after TOML load, apply SURR_DB_URL / SURR_DB_NS / SURR_DB_DB if set. Validate scheme.
   - Tests/docs/CI: add a config unit test (env override precedence); document in README/env example.

4) DB name default mismatch vs rule
   - What/where: defaults use "consciousness"; your rule states "conciousness". This easily causes silent connection to the wrong DB.
   - Why: Footgun for personal setup; conflicts with rule file.
   - Minimal fix: either change defaults to match the rule (conciousness), or keep "consciousness" but explicitly set SURR_DB_DB=conciousness in .env and document prominently. Env override from item (3) reduces risk either way.
   - Tests/docs/CI: Highlight in README; ensure .env.example aligns with chosen default.

5) CI “openai” matrix will attempt network with a dummy key
   - What/where: .github/workflows/rust.yml: OPENAI_API_KEY=dummy-key-for-tests with SURR_EMBED_PROVIDER=openai.
   - Why: create_embedder treats that as a real key; CI may make outbound calls or fail unpredictably.
   - Minimal fix: drop the openai matrix (keep candle-only), or gate all OpenAI-path tests behind a real secret and skip otherwise.
   - Tests/docs/CI: Keep CI purely local/fenced by default.

High
6) Injection scale mismatch (debug=4) vs implemented clamp (0..3)
   - What/where: src/tools/thinking.rs sets debug default to 4; inject_memories clamps scale 0..3.
   - Why: debug scale silently reduced; mismatched docs vs behavior.
   - Minimal fix: set debug default to 3. Align docs and schemas.

7) Re-embed path depends on fixed HTTP base (fallout of blocker #1)
   - What/where: src/lib.rs run_reembed uses HttpSqlConfig; fails with ws:// URL until normalization fix.
   - Why: Maintenance flows should be reliable.
   - Minimal fix: fix #1; consider a small self-check/log at start of re-embed.

8) Schemas/help drift (legacy names) — see blocker #2
   - Minimal fix: once a direction is chosen, align schemas/help/README/tests.

9) Startup validation for embedding dimension hygiene
   - What/where: current checks exist in maintenance_ops; no preflight at startup.
   - Why: If a user switches provider/model, mixed dims linger until a manual health check.
   - Minimal fix: optional preflight in main.rs that logs a warning (or fails if SURR_EMBED_STRICT=1) when mixed dims are detected; advise re-embed.

10) Error handling upgrades (consistent, contextual)
   - What/where: main.rs, server/mod.rs, embeddings.rs — errors bubble without enough context in some paths.
   - Why: Speeds root-cause work during outages.
   - Minimal fix: use anyhow::Context or enrich SurrealMindError mapping consistently; avoid bare unwrap() in hot paths.

11) Rate limiting for OpenAI embeddings
   - What/where: src/embeddings.rs; no throttle; retries exist but not bucket-limiting.
   - Why: Bulk re-embed can hit 429; add token-bucket or simple governor.
   - Minimal fix: small in-process limiter; env-driven tokens/sec; disabled for Candle/local.

Medium
12) DB reconnection strategy missing (Ws client)
   - What/where: src/server/mod.rs Surreal::new::<Ws>(); no reconnect loop or health-check.
   - Why: Personal deployments benefit from auto-heal; otherwise manual restart.
   - Minimal fix: add a simple reconnect/backoff loop or health-check task; document a toggle if you prefer manual ops.

13) Config ergonomics and validation polish
   - What/where: src/config.rs; many runtime knobs (InnerVoiceConfig/RuntimeConfig) are good, but DB/env overrides and dim/provider validation can be stricter.
   - Why: Env-first clarity and fewer surprises.
   - Minimal fix: apply env overrides (blocker #3), validate provider/model/dim coherency, optionally split sections (DatabaseConfig, TransportConfig, EmbeddingConfig).

14) Tests/scripts out of sync with unified tool
   - What/where: tests/test_mcp_comprehensive.sh, tests/test_search.sh (think_search), tests/tool_schemas.rs, detailed_help.
   - Why: Avoid false negatives and METHOD_NOT_FOUND.
   - Minimal fix: update scripts/tests to whichever surface you choose (aliases vs unified).

15) Inner-voice defaults tuning (personal mode)
   - What/where: src/config.rs (InnerVoiceConfig), src/tools/inner_voice.rs.
   - Why: Planner already off-by-default; auto_extract_to_kg defaults to true; consider default=false for a quieter personal workflow.
   - Minimal fix: gate with an env profile or flip default per your preference.

Low
16) Doc consistency & Quick Start
   - What/where: README.md, AGENTS.md/WARP.md; ensure tool roster, env knobs, and defaults reflect the chosen approach and the env-first precedence.

17) Vector index coverage (future)
   - What/where: HNSW on thoughts; KG uses client-side cosine over capped sets; totally fine for now.
   - Why: Scales can revisit this.

Federation items (integrated and deduped)
- Clippy warnings (treat as errors): keep zero-warning standard.
- Prompt metrics TODO / legacy_storage conflict: either implement or prune dead code.
- Hypothesis verification tests/edges: broaden test coverage for low-confidence paths.
- Cognitive shapes roadmap: track as feature work (ties to veterans mission).
- Performance polish (SIMD cosine) — optional; measure first.

Disputed/Resolved from Federation
- LRU cache unbounded: already bounded (SURR_CACHE_MAX -> NonZeroUsize); no action required.

Action checklist (suggested sequence)
1) Fix HTTP SQL normalization (src/utils/db.rs) — tests for ws:///wss:///http:// inputs.
2) Decide and apply MCP surface alignment (re-expose legacy aliases OR migrate all references to legacymind_think). Update schemas/help/tests.
3) Apply env overrides for DB URL/ns/db in Config::load; document precedence in README.
4) Resolve DB name mismatch per your rule (default or .env); highlight in README.
5) Stabilize CI: drop openai job or gate with real secret; keep Candle-local CI.
6) Align injection defaults (debug=3); docs/schemas consistency.
7) Add optional startup dim-hygiene preflight warning; keep re-embed flow intact.
8) Improve contextual errors in main/server/embeddings; reduce unwraps in hot paths.
9) Add naive rate limiter for OpenAI calls; env-driven.
10) (Optional) Ws reconnect/health-check loop; behind a flag.

Appendix — File references (for quick nav)
- URL base bug: src/utils/db.rs (from_config, sql_url)
- MCP surface: src/server/mod.rs (list_tools, call_tool) + src/schemas.rs + src/tools/detailed_help.rs + tests/*
- Env overrides: src/config.rs (Config::load)
- DB name: src/config.rs default + surreal_mind.toml + .env.example
- CI: .github/workflows/rust.yml
- Injection defaults: src/tools/thinking.rs, inject_memories clamp in src/server/mod.rs
- Health checks: maintenance_ops; add preflight in main.rs if desired
- Rate limit: src/embeddings.rs

Notes
- All changes should be env-driven where control is needed (SURR_* knobs) to respect your preference to avoid code changes per environment.
- No Docker introduced; all local-first.

— End of merged CCR —

