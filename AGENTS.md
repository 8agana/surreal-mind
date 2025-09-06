# Surreal Mind MCP — Agent Guide

Last updated: 2025-09-06

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
- `think_convo`: Persist a thought with embeddings and run KG-only injection (scale configurable). Runs a local, deterministic “frameworks” pass (convo/1) post‑create, pre‑injection. Returns thought id and framework_enhanced flag.
- `think_plan`, `think_debug`, `think_build`, `think_stuck`: Variants tuned for retrieval pool size only (no behavior drift beyond thresholds).
- `legacymind_search`: Unified LM repo search — memories by default; include_thoughts=true also searches thoughts.
- `photography_think`, `photography_memories`, `photography_search`: Tools scoped to Photography repo (ns=photography, db=work). photography_think injection candidate pool=500.
- `memories_create` (alias: `knowledgegraph_create`): Create KG entities/observations.
- `memories_moderate` (alias: `knowledgegraph_moderate`): Review/stage KG entries.
- `maintenance_ops`: Health checks, re-embedding, archival/export, and cleanup.
- `inner_voice`: Retrieval + synthesis with optional auto‑extraction to staged KG candidates (default ON). Thought persisted with compact Sources line.

Notes
- Tools are always listed (no env flag required for visibility). Any gating is enforced inside handlers.
- See `src/schemas.rs`, `src/server/mod.rs`, and `src/tools/*` for exact parameters. `detailed_help` returns live schema/aliases.

### Frameworks (think_convo)
- Versioned envelope: `framework_version: "convo/1"`, `methodology: (socratic|first_principles|mirroring|lateral|constraints)`.
- Data: `{ summary, takeaways[], prompts[], next_step, tags[] }` — all length‑bounded; strict JSON validation.
- Determinism: seeded template selection via blake3(content_norm) → u64.
- Timeout: `SURR_THINK_ENHANCE_TIMEOUT_MS` (default 600ms) enforced; fail‑open.
- Tag policy: merged into thought.tags via `SURR_THINK_TAG_WHITELIST` (default `plan,debug,dx,photography,idea`).
- Env lexicons (comma‑sep): `SURR_THINK_LEXICON_DECIDE`, `SURR_THINK_LEXICON_VENT`, `SURR_THINK_LEXICON_CAUSAL`, `SURR_THINK_LEXICON_POS`, `SURR_THINK_LEXICON_NEG`.
- Logging: 200‑char preview gated behind `SURR_THINK_DEEP_LOG=1`.

### Inner Voice (auto‑extraction)
- Param: `auto_extract_to_kg` (boolean). Default ON via `SURR_IV_AUTO_EXTRACT_KG=1`.
- Creates pending candidates in `kg_entity_candidates`/`kg_edge_candidates` with `data.staged_by_thought=<thought_id>`, `origin='inner_voice'`.
- Moderate via `memories_moderate`.

### Photography Isolation
- Auto‑connects to `ns=photography`, `db=work` (URL/user/pass inherited unless overridden by `SURR_PHOTO_*`).
- No cross‑pollination with the default repo by default.

### Prompt Registry (AI-driven, discoverability-only)
- Purpose: Expose stable, named prompt metadata (id, one-liner, purpose, inputs, constraints) for inspection and docs — no runtime auto-switching.
- Surfaces via the existing `detailed_help` tool:
  - Roster: `{ "tool": "detailed_help", "arguments": { "prompts": true } }`
  - Full entry: `{ "tool": "detailed_help", "arguments": { "prompt_id": "think-search-v2", "format": "full" } }`
- Metadata: `id`, `version`, `checksum`, `one_liner`, `purpose`, `inputs{}`, `constraints{}` (MCP_NO_LOG, no mixed dims, KG-only injection, etc.), `lineage{ parent_id?, created_at, created_by, change_rationale? }`.
- Optional P3.5 metrics and critiques:
  - `prompt_invocations`: usage tracking (success/refusal/error rates, latency, tokens).
  - Critiques: stored as thoughts with `critique_data.prompt_id` linkage; use to generate evolution suggestions.
- Guardrails: MCP_NO_LOG respected; registry does not change tool behavior; operator action required for updates.

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
- Re-embed thoughts (to current dims) — resilient HTTP parsing & stable ids:
  1) `export OPENAI_API_KEY=...` and `export SURR_EMBED_PROVIDER=openai`
  2) `cargo run --bin reembed` (HTTP client UA: `surreal-mind/<ver> (component=reembed; ns=<ns>; db=<db>[; commit=<sha>])`)
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

## Warp Code Integration (Profiles and Flow)
Warp Code provides an in-terminal "prompt → diff → review → apply" loop. If the profile picker is not available yet on your account, use the shortcuts below to emulate profiles.

Recommended profiles for this repo
- Build (Rust MCP): Implement features with guardrails and standards.
- Debug (Root cause): Investigate first; propose minimal, reversible fixes.
- Docs/Release: Keep docs/config/tests aligned with behavior.

Guardrails enforced
- Env-first configuration; do not introduce Docker.
- No fake/deterministic embedders; respect provider/model/dim stamps.
- Never compare embeddings across mismatched dimensions.
- Injection is KG-only; respect scale limits and thresholds.
- Rust standards: cargo fmt, clippy (treat warnings as errors), cargo check, cargo test before calling work "done".

Shortcuts (profile emulation)
Use these as slash-style prompts to seed agent behavior until native profile UI is visible:

```yaml path=null start=null
shortcuts:
  - name: /build
    description: Implement features with Rust MCP guardrails
    prompt: |
      Act as a Build profile for a Rust MCP project.
      Requirements:
      - Propose diffs; do not apply until I accept.
      - Enforce: cargo fmt, clippy (warnings as errors), cargo check, cargo test.
      - Env-first configuration; no Docker; no fake/deterministic embeddings.
      - Maintain embedding_dim hygiene and provider/model/dim stamps.
      - Suggest a branch name and concise commit message; do not commit without asking.
      Context focus: src/tools/, src/server/mod.rs, src/schemas.rs, src/config.rs, tests/

  - name: /debug
    description: Investigate failures and propose minimal fixes
    prompt: |
      Act as a Debug profile (root cause first, minimal reversible changes).
      Requirements:
      - Read-first; do not apply changes without confirmation.
      - Prefer logs/repro and minimal blast radius fixes; gate behind flags.
      - Highlight risky edits and tradeoffs.
      Priority files: src/embeddings.rs, src/server/, src/tools/, tests/

  - name: /docs
    description: Update docs/config/tests after code changes
    prompt: |
      Act as a Docs/Release profile.
      Requirements:
      - Propose diffs to README/WARP.md/config docs/tests when schemas or behavior change.
      - Document env vars; avoid Docker instructions.
      - Keep Quick Start and SOPs accurate.
```

Using profiles in practice
1) Start with /build, /debug, or /docs.
2) Describe the task and constraints (e.g., "env-first; update schemas/tests; fix clippy warnings").
3) Review the proposed diffs; request tweaks until correct; then accept to apply.
4) Verify in terminal: run fmt, clippy, check, test; iterate if issues are found.

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
- No flag‑gating for tool visibility; defaults should work after rebuild without toggles.

## Quick Start (Post-Restart)
1) `maintenance_ops { "subcommand": "health_check_embeddings" }` → expect `mismatched_or_missing = 0` across tables.
2) Spot-check injection: call `think_convo` with `injection_scale=1/2/3` → expect 5/10/20 injected memories.
3) Confirm logs show `provider=openai`, `model=text-embedding-3-small`, `dims=1536`.

## Roadmap (next focus)

### Completed
- **[2025-09-04] First AI-driven Feature**: Self-aware prompt registry with transparency, lineage, and critique capabilities.
  - Transparent prompt metadata via detailed_help
  - Git-style lineage with parent/child relationships
  - Critique storage as linked thoughts
  - MCP_NO_LOG and dimension guardrails
  - Optional invocation metrics

### Next
- Restore injection thresholds to recommended values after validation.
- Light cleanup: confirm DB indexes, drop dead imports, update docs as code stabilizes.
- Potential prompt registry extensions:
  - Critiques and evolution suggestions for core tools
  - Usage stats collection for popular patterns
  - Cross-reference support between prompts and thoughts

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
