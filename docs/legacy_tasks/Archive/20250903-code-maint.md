# SurrealMind Code Cleanup Proposal
**Date:** 2025-09-03
**Author:** Warp
**Project:** LegacyMind / SurrealMind


P0 — Must-fix before next cut
1) Make clippy + fmt green (warnings as errors)
   •  Issue: clippy fails on test-only code (e.g., unnecessary_literal_unwrap in src/server/mod.rs tests).
   •  Steps:
   •  Replace literal Some(..).unwrap_or(..) in test helpers with direct values.
   •  Ensure no new warnings across workspace and all targets; enforce rustfmt in CI.
   •  Commands:
   •  cargo fmt --all --check
   •  cargo clippy --workspace --all-targets -D warnings
   •  Acceptance:
   •  fmt check passes; clippy passes with -D warnings (zero warnings) on workspace/all-targets.
   •  Risk/rollback: None; test-only edits.

2) rmcp 0.6.1 lockfile verification
   •  Issue: Cargo.toml already at rmcp = "0.6.1". Ensure lockfile is aligned and builds are clean.
   •  Steps:
   •  cargo update -p rmcp
   •  cargo tree -i rmcp | rg 0.6.1 (single version present)
   •  cargo check --locked
   •  Acceptance:
   •  rmcp resolved to 0.6.1 in Cargo.lock; cargo check --locked passes; dependency tree shows only 0.6.1.
   •  Risk/rollback:
   •  If any dev-only breakage, cargo update -p rmcp --precise <previous> to pin back (not expected).

3) Public tool name alignment (no back-compat)
   •  Issue: tools/list exposes new names (think_convo, think_search, etc.). Some docs/tests still refer to old names (convo_think, search_thoughts) in listings.
   •  Steps:
   •  Update all docs, tests, and scripts to use only the canonical names.
   •  Remove alias mentions and any compatibility checks.
   •  Acceptance:
   •  tools/list shows only canonical names and all references match: think_convo, think_plan, think_debug, think_build, think_stuck, think_search, memories_{create,search,moderate}, maintenance_ops, detailed_help.
   •  Risk/rollback: None.

4) .gitignore fixes for tracked docs
   •  Issue: .gitignore excludes WARP.md, AGENTS.md, and /docs, blocking doc updates.
   •  Steps (planned change):
   •  Remove these ignores; keep /target, /surreal_data, backups, logs; add /archive, .DS_Store, and *.log. Ensure Cargo.lock remains tracked.
   •  Acceptance:
   •  git status shows WARP.md/AGENTS.md/docs tracked after update.
   •  Risk/rollback: If undesired, revert just those lines.

5) Logging hygiene for MCP (respect MCP_NO_LOG, tame chatty debug)
   •  Issue: search_thoughts logs embedding snippets; MCP_NO_LOG not enforced; MCP stdio risks.
   •  Steps:
   •  When MCP_NO_LOG=1, do not install any tracing subscriber (hard disable). Otherwise, install error-only by default for MCP stdio transports.
   •  Demote deep embedding logs to trace; optionally guard `trace` details behind SURR_DEEP_EMBED_DEBUG=1.
   •  Ensure RUST_LOG cannot override MCP_NO_LOG in MCP stdio mode.
   •  Acceptance:
   •  With MCP_NO_LOG=1, zero non‑protocol bytes are written to stdout/stderr; only JSON‑RPC frames appear.
   •  Risk/rollback:
   •  If diagnostics needed, set MCP_NO_LOG=0 or run in non‑stdio mode and enable RUST_LOG explicitly.

P1 — Should-fix (fast follow)
1) cargo check all-targets/all-features friction (db_integration harness)
   •  Issue: cargo check --all-targets --all-features fails due to tests/mcp_integration.rs import/API nits (RequestContext::default isn’t available, missing imports).
   •  Options:
   •  A) Fix test: add use surreal_mind::server::SurrealMindServer; adapt RequestContext construction to current rmcp API (no default()).
   •  B) Gate/skip when feature not enabled (the repo already uses feature="db_integration"; ensure CI doesn’t force it).
   •  Acceptance:
   •  cargo check --all-targets (default features) passes; separate CI job runs --all-features and passes when DB is available.
   •  Risk: None; small test-only change.

2) Remove Nomic/fake embedder references from README/.env.example
   •  Issue: Docs suggest providers that are disallowed. Code supports OpenAI (1536) + Candle/BGE (384).
   •  Steps:
   •  Rewrite provider section: OpenAI primary (text-embedding-3-small, 1536); Candle (BGE-small-en-v1.5, 384) local dev; no fake/deterministic or Nomic.
   •  Add strong “no mixed dims” warnings and re-embed SOP pointer.
   •  Acceptance:
   •  Docs reflect provider truth; no mention of “fake” or Nomic.
   •  Risk: None.

3) maintenance_ops: reembed_kg cargo spawn safety
   •  Issue: Tool spawns cargo run --bin reembed_kg. Heavy and side-effectful.
   •  Steps:
   •  Guard spawning behind SURR_ENABLE_SPAWN=1 (default off) and otherwise pivot maintenance_ops to advisory-only responses for re-embed, pointing operator to run the binary directly.
   •  Acceptance:
   •  With SURR_ENABLE_SPAWN unset, reembed_kg returns an instructional response, not a spawn.
   •  Risk: None; improves operator safety.

4) Lower verbosity in search_thoughts
   •  Steps:
   •  Demote embedding-preview logs to trace; only keep short info lines for counts, thresholds, latencies, and high-sim hit summaries (no content or vectors).
   •  Acceptance:
   •  RUST_LOG=info shows only succinct operational messages.

P2 — Nice-to-have
1) Remove/mark legacy mains
   •  Issue: src/main_modular.rs and src/main_new.rs are confusing/out-of-date.
   •  Steps:
   •  Either delete or add a top comment “legacy example; not wired” and exclude from build via Cargo.toml [[bin]] sections if needed.
   •  Acceptance:
   •  No confusion in IDEs/builds; only src/main.rs is the entrypoint.

2) detailed_help overview mode
   •  Steps:
   •  If tool omitted, return compact roster + usage hints.
   •  Response shape (overview): [{ name, one_liner, key_params[] }]
   •  Acceptance:
   •  tools/call detailed_help with { } returns overview (name/description list).

3) CI refinements
   •  Steps:
   •  Add `cargo check --locked`, `cargo fmt --check`, and clippy gating in CI.
   •  Optional: matrix to run tests with Candle-only (no network) by setting SURR_EMBED_PROVIDER=candle in CI.
   •  Separate job (manual) can run `cargo update -p rmcp` then `cargo check` to validate new patch bumps.

P3 — Forward-looking improvements
1) Prompt registry (rmcp 0.6.1 prompts)
   •  Steps:
   •  Evaluate surfacing prompt definitions for detailed_help and tool metadata; only if beneficial.

2) Dimension hygiene assertions
   •  Steps:
   •  Add tests asserting embedding_dim filters precede cosine everywhere; catch regressions.

3) SurrealDB index validation pass
   •  Steps:
   •  Confirm/define indexes referenced in code and WARP.md (e.g., thoughts.created_at, kg_entities.name, kg_edges triplet), and document them.
   •  Acceptance:
   •  A `health_check_indexes` or equivalent passes and reports expected indexes present.

Documentation updates (proposed changes; ready to prep diffs)
•  WARP.md / AGENTS.md:
•  State clearly: submode inputs ignored; SURR_SUBMODE_RETRIEVAL only adjusts thresholds; KG-only injection; list canonical tools; logging guidance (MCP_NO_LOG).
•  Maintenance SOP: prefer running reembed/reembed_kg binaries; tool returns advisory when spawn disabled.
•  README.md:
•  Replace Nomic/fake guidance; two-track Quick Start (OpenAI primary, Candle local); “health_check_embeddings first” SOP; tool examples with new names.
•  .env.example:
•  Only OpenAI/Candle knobs; include SURR_INJECT_T1/T2/T3/FLOOR with recommended defaults; add MCP_NO_LOG and SURR_DEEP_EMBED_DEBUG commented; add SURR_ENABLE_SPAWN commented and defaulted off.
•  surreal_mind.toml:
•  Add explanatory comments: one provider per runtime; re-embed on switch; thresholds documented.
•  .gitignore:
•  Remove WARP.md, AGENTS.md, /docs ignores; add .DS_Store, *.log and /archive; keep Cargo.lock tracked.

Operational SOPs (run-only)
•  Verify rmcp:
•  cargo update -p rmcp
•  cargo tree -i rmcp
•  cargo check --locked
•  Default local-review env (Candle/offline), or OpenAI if key present:
•  SURR_EMBED_PROVIDER=candle (or openai)
•  SURR_DB_URL=127.0.0.1:8000; SURR_DB_NS=review; SURR_DB_DB=surreal_mind_review; SURR_DB_USER=root; SURR_DB_PASS=root
•  Prefer unset SURREAL_MIND_CONFIG (or pass an explicit --config) to avoid TOML parse issues on missing files.
•  Minimal MCP smoke (for our logs; no changes applied):
•  Initialize → tools/list → maintenance_ops health_check_embeddings
•  Optionally memories_create entity; think_convo with injection_scale=1; think_search.

Sequencing and branch strategy (when you say “apply it”)
•  Branch: chore/p0-quality-gates (clippy fix, .gitignore, MCP_NO_LOG gating, maintenance_ops spawn guard)
•  Then chore/docs-sync (WARP/AGENTS/README/.env.example/.gitignore updates; canonical tool names only)
•  Then feat/tests-ci (db_integration compile fixes and CI tweaks)
•  Each branch:
•  cargo fmt; cargo clippy -D warnings; cargo test; cargo build --release
•  Commit policy: logical commits per concern; squash on merge if preferred.

Acceptance checklist for the whole push
•  Clippy passes with -D warnings; unit/integration pass.
•  rmcp resolved to 0.6.1 in lockfile; cargo check OK.
•  tools/list shows canonical names only; tests/scripts expect them.
•  MCP_NO_LOG respected; search_thoughts no chatty logs by default.
•  maintenance_ops reembed_kg is advisory (or gated by SURR_ENABLE_SPAWN=1).
•  WARP/AGENTS/README/.env.example and .gitignore updated and tracked.

Risks and rollbacks
•  Minimal. All changes are localized; roll back by reverting commits or flipping env flags (MCP_NO_LOG, SURR_ALLOW_SPAWN).
•  Note: No backward compatibility for tool names is provided; external scripts must update to canonical names.
