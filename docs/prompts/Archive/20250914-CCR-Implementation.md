# CCR Implementation Plan — Surreal Mind MCP

Date: 2025-09-14
Based on: fixes/20250913-CCR.md (commit e52f196563c2)
Owner: Codex (with Sam)

## Objectives
- Ship targeted fixes that directly address the CCR findings without scope creep.
- Keep defaults safe, avoid regressions, and stay env‑first with a clean rollback path.

## Phasing Overview
- Phase 1 — Quick Wins (low‑risk, high value)
- Phase 2 — Tests & Clippy Remediation
- Phase 3 — Hardening & Housekeeping
- Phase 4 — Optional Schema/Index Ergonomics

## Phase 1 — Quick Wins
1) Fix OpenAI embedder rate limiter
   - Problem: `Instant::now().elapsed()` always ~0 → unnecessary sleeps each call.
   - Action:
     - Switch to monotonic ms since process start or epoch ms; compare to stored atomic.
     - Update after each call; only sleep when `now < last + interval`.
     - Add unit test to verify no sleep when interval elapsed.
   - Files: `src/embeddings.rs`
   - Acceptance:
     - Clippy clean; unit test for limiter behavior passes.
     - p50 embedding latency drops (sanity via logs if enabled).

2) Align startup tool log with actual roster
   - Problem: `src/main.rs` prints legacy think_* tools not present in `list_tools`.
   - Action: Replace static string with a concise line or remove; ensure no drift with `list_tools`.
   - Files: `src/main.rs`
   - Acceptance: Startup log matches the tools returned by `list_tools`.

3) Update inner_voice description text
   - Problem: Tool description says retrieval‑only; handler performs synthesis + optional extraction.
   - Action: Update description in `list_tools` (server) and confirm `detailed_help` summary matches.
   - Files: `src/server/mod.rs`, `src/tools/detailed_help.rs`
   - Acceptance: `tools/list` and `detailed_help` describe synthesis with grounding and optional KG auto‑extraction.

4) Warn on query‑param token usage
   - Problem: `allow_token_in_url=true` can leak tokens in logs/proxies.
   - Action: Emit single WARN at startup when enabled; mask in any debug prints.
   - Files: `src/http.rs` (or `src/main.rs` during HTTP startup)
   - Acceptance: One startup WARN when enabled; default remains off.

5) Remove placeholder assertion in tests
   - Problem: `assert!(true)` in `tests/tool_schemas.rs` fails clippy.
   - Action: Delete or gate with allow; prefer deletion.
   - Files: `tests/tool_schemas.rs`
   - Acceptance: Clippy no longer flags `assertions_on_constants`.

## Phase 2 — Tests & Clippy Remediation
6) Update RMCP test contexts
   - Problem: `RequestContext::with_id` removed in rmcp 0.6.3.
   - Action: Replace with `RequestContext::<rmcp::service::RoleServer>::default()` or the current constructor supported by rmcp (typed context).
   - Files: `tests/mcp_integration.rs`
   - Acceptance: Tests compile under current rmcp; clippy passes.

7) Remove SurrealDB `.sql()` introspection from tests
   - Problem: `Query.sql()` API not present; brittle test.
   - Action: Replace with behavior assertions (dimension filter via actual query + check) behind `db_integration`, or drop the SQL peeking and validate via `check_embedding_dims()`.
   - Files: `tests/dimension_hygiene.rs`
   - Acceptance: Compiles with feature on; clippy all‑targets OK. Behavior validated when RUN_DB_TESTS=1.

8) Clippy configuration and gating
   - Problem: `--all-features` compiles DB‑integration tests; OK if they build.
   - Action: Ensure integration tests compile with feature enabled after above fixes. Keep CI using `cargo clippy --all-targets -- -D warnings` (no `--all-features`) unless we explicitly want to validate feature builds.
   - Files: CI config (if any), docs in `AGENTS.md`.
   - Acceptance: Local clippy passes; CI green.

## Phase 3 — Hardening & Housekeeping
9) Tolerant float sorts in HTTP metrics
   - Problem: `partial_cmp(...).unwrap()` can panic on NaN.
   - Action: Use `unwrap_or(Ordering::Equal)` or `total_cmp` via newtype if needed.
   - Files: `src/http.rs`
   - Acceptance: No panic on pathological inputs; clippy clean.

10) Submodes config deprecation
   - Problem: Submodes removed from surfaces; config types still present.
   - Action: Add deprecation comments; ensure no write‑paths depend on `submode`. Keep deserialization for BC.
   - Files: `src/config.rs`, `src/tools/thinking.rs` (ensure we only set `submode: NONE`), docs.
   - Acceptance: No runtime dependence; docs reflect deprecation.

11) Minor unwrap/expect cleanups
   - Examples: default bind parse `expect`, metric sorts.
   - Action: Replace with error handling where user input involved; leave safe invariants.
   - Files: `src/config.rs`, `src/http.rs`.
   - Acceptance: Clippy passes; no panics from external inputs.

12) Inner Voice help examples
   - Action: Ensure `detailed_help` examples include current response fields (answer, synth_thought_id, feedback*, sources_compact, provider/model, embedding_dim, extracted counts).
   - Files: `src/tools/detailed_help.rs`
   - Acceptance: Example matches runtime contract.

13) IV extractor env precedence doc check
   - Action: Confirm code aligns with AGENTS.md precedence (`IV_CLI_*` → `IV_SYNTH_*`), update comments if needed.
   - Files: `src/tools/inner_voice.rs`, `AGENTS.md`.
   - Acceptance: Docs and code agree.

## Phase 4 — Optional Schema/Index Ergonomics
14) Eager continuity indexes
   - Problem: Continuity indexes created via maintenance tool only.
   - Action: Optionally add to `initialize_schema()` so fresh DBs are ready.
   - Files: `src/server/mod.rs`
   - Acceptance: INFO FOR TABLE shows session/chain indexes after init.

## Validation Checklist
- cargo check OK
- cargo clippy --all-targets -- -D warnings OK
- Unit test for rate limiter OK; DB‑gated tests compile; RUN_DB_TESTS exercises behavior
- tools/list descriptions match behavior; startup log aligned
- Single WARN on `allow_token_in_url=true`

## Rollback Plan
- Guard changes behind minimal, reversible edits:
  - Rate limiter: single file swap; keep old code in git history.
  - Log/description changes: string edits only.
  - Tests: purely dev‑side; no runtime impact.

## File Touch List (expected)
- src/embeddings.rs
- src/main.rs
- src/server/mod.rs
- src/tools/detailed_help.rs
- src/http.rs
- tests/tool_schemas.rs
- tests/mcp_integration.rs
- tests/dimension_hygiene.rs
- AGENTS.md (notes on clippy and IV docs)

## Notes
- Keep env‑first behavior; do not introduce per‑call embedder fallback.
- Maintain dimension hygiene and KG‑only injection.
- Avoid SurrealQL UNION; current separation is good.

