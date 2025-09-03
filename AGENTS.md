# Surreal Mind MCP — Agent Guide

Last updated: 2025-09-03

This file is the working guide for operating and extending the Surreal Mind MCP server. It reflects the current design after the thought/injection refactor and should be used by CLI agents (Codex, CC) and contributors.

— WARP users: `WARP.md` is a symlink to this file.

## Purpose
Surreal Mind augments agent thinking with persistent memory backed by SurrealDB and a Knowledge Graph (KG). It exposes MCP tools for capturing thoughts, retrieving relevant memories, and running maintenance operations.

## Current State
- Embeddings: OpenAI `text-embedding-3-small` at 1536 dims is primary. Candle BGE-small-en-v1.5 (384 dims) is for local dev/fallback only. No Nomic. No fake/deterministic embedders.
- Consistency: Provider is selected at startup; there is no per-call fallback. Every write stamps `embedding_provider`, `embedding_model`, `embedding_dim`, `embedded_at`.
- Dimension hygiene: All search/injection paths filter by `embedding_dim` before cosine.
- Injection: KG-only (entities + observations). Limits by scale: 1→5, 2→10, 3→20. UNION queries were removed; two SELECTs are merged client-side.
- Submodes: Removed from storage and tool surfaces; any legacy `submode` input is ignored.

## Exposed MCP Tools
- `think_convo`: Persist a thought with embeddings and run KG-only injection (scale configurable). Returns thought id + optional enriched summary.
- `think_plan`: Planning-oriented thought capture (same backbone; different candidate pool size).
- `think_debug`, `think_build`, `think_stuck`: Variants tuned for retrieval pool size only. No behavior drift beyond thresholds.
- `think_search`: Dimension-safe semantic search over thoughts.
- `memories_create` (alias: `knowledgegraph_create`): Create KG entities/observations.
- `memories_search` (alias: `knowledgegraph_search`): Search KG.
- `memories_moderate` (alias: `knowledgegraph_moderate`): Review/stage KG entries.
- `maintenance_ops`: Health checks, re-embedding, archival/export, and cleanup.

See `src/schemas.rs`, `src/server/mod.rs`, and `src/tools/*` for exact parameters. `detailed_help` returns live schema/aliases.

## Embeddings Strategy
- Primary: `SURR_EMBED_PROVIDER=openai`, `SURR_EMBED_MODEL=text-embedding-3-small`, `SURR_EMBED_DIM=1536` (implicit for this model).
- Dev/Fallback: `SURR_EMBED_PROVIDER=candle` uses local BGE-small-en-v1.5 (384 dims). Only for development; do not mix dims in the same DB.
- Selection: Startup picks one provider based on env and keys; no per-call fallback. If `OPENAI_API_KEY` is unset, Candle is used.
- Guardrails:
  - Always filter by `embedding_dim` before cosine.
  - Never write embeddings without stamping provider/model/dim/embedded_at.
  - Single provider per runtime; re-embed when switching providers/models.

## Memory Injection (KG-only)
- Scale limits: 1→5, 2→10, 3→20 results.
- Thresholds (env tunables):
  - `SURR_INJECT_T1`, `SURR_INJECT_T2`, `SURR_INJECT_T3` control cosine thresholds for scales 1–3.
  - `SURR_INJECT_FLOOR` acts as a minimal floor if nothing passes the scale threshold.
- Recommended production values after validation: `T1=0.6`, `T2=0.4`, `T3=0.25`, `FLOOR=0.15`.
- Candidate pools by tool (defaults):
  - `think_convo=500`, `think_plan=800`, `think_debug=1000`, `think_build=400`, `think_stuck=600`.
- Implementation notes:
  - Two SELECTs against `kg_entities` and `kg_observations`; results are merged in code (no UNION).
  - Missing KG embeddings are computed on the fly and persisted best-effort if dimensions match.

## Health Checks and Re-embed SOPs
- Health check: `maintenance_ops { subcommand: "health_check_embeddings" }` → reports `expected_dim` and per-table mismatches across `thoughts`, `kg_entities`, `kg_observations`.
- Re-embed thoughts (to current dims):
  1) `export OPENAI_API_KEY=...` and `export SURR_EMBED_PROVIDER=openai`
  2) `cargo run --bin reembed`
  3) Verify: `SELECT array::len(embedding), count() FROM thoughts GROUP BY array::len(embedding);`
- Re-embed KG: `cargo run --bin reembed_kg` (observes active provider; persists dims and metadata).

## Configuration
- Env-first; `surreal_mind.toml` mirrors defaults. Key env vars:
  - Embeddings: `OPENAI_API_KEY`, `SURR_EMBED_PROVIDER` (`openai`|`candle`), `SURR_EMBED_MODEL`, `SURR_EMBED_DIM`.
  - DB: `SURR_DB_URL`, `SURR_DB_NS`, `SURR_DB_DB`, `SURR_DB_USER`, `SURR_DB_PASS`.
  - Retrieval: `SURR_KG_CANDIDATES`, `SURR_INJECT_T1/T2/T3`, `SURR_INJECT_FLOOR`.
  - Runtime/logging: `RUST_LOG`, `MCP_NO_LOG`, `SURR_TOOL_TIMEOUT_MS`.
  - Maintenance: `SURR_RETENTION_DAYS` for archival.



## Build & Run
- Prereqs: Rust toolchain, SurrealDB reachable via WebSocket, `.env` from `.env.example`.
- Build: `cargo build` (release: `cargo build --release`).
- Run MCP (stdio): `cargo run`.
- Logs: `RUST_LOG=surreal_mind=debug,rmcp=info cargo run`.
- Binary (release): `target/release/surreal-mind`.

## Testing
- Unit tests live near modules; integration tests under `tests/`.
- Avoid external network; mock embeddings/DB where possible.
- Run: `cargo test` (with logs: `RUST_LOG=debug cargo test -- --nocapture`).

## Operating Principles
- Truth-first diagnostics: verify embedding dims and candidate counts before tuning.
- Minimal blast radius: stage changes behind env flags; defaults remain safe.
- No mixed dims: pick one provider/model per runtime and re-embed on switch.
- KG-only injection: thoughts are never injected as raw context; only KG entities/observations are.
- Avoid SurrealQL UNION for combined queries; prefer separate SELECTs.

## Quick Start (Post-Restart)
1) `maintenance_ops { "subcommand": "health_check_embeddings" }` → expect `mismatched_or_missing = 0` across tables.
2) Spot-check injection: call `think_convo` with `injection_scale=1/2/3` → expect 5/10/20 injected memories.
3) Confirm logs show `provider=openai`, `model=text-embedding-3-small`, `dims=1536`.

## Roadmap (next focus)

- Restore injection thresholds to recommended values after validation.
- Light cleanup: confirm DB indexes, drop dead imports, update docs as code stabilizes.

## Safety & Guardrails
- Do not reintroduce fake/deterministic or Nomic embedders.
- Do not silently change defaults or leak secrets in logs.
- Do not compare embeddings of different dimensions.

## Reference Paths
- Binary: `target/release/surreal-mind`
- Build roots:
  - Repo: `/Users/samuelatagana/Projects/LegacyMind/surreal-mind`
  - Local models (BGE): `models/bge-small-en-v1.5`
- Key sources: `src/main.rs`, `src/server/mod.rs`, `src/embeddings.rs`, `src/tools/*`, `src/schemas.rs`, `src/config.rs`
