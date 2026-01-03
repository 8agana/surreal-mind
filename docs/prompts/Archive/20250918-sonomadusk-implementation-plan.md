# Implementation Plan — Photography Voice & Moderation Parity

**Author:** Codex
**Date:** 2025-09-18
**Executor:** Sonoma Dusk Alpha

## Goals
- Give the photography namespace a first-class grounded synthesis tool (`photography_voice`) that mirrors `inner_voice` while keeping the namespaces isolated.
- Clarify and streamline photography knowledge-graph moderation so it is discoverable and ergonomic.
- Preserve existing behavior for the primary namespace and avoid regressions in tool schemas, tests, or MCP registration.

## Constraints
- Keep photography data in the photography DB (`ns=photography`, `db=work`); do not merge with the primary SurrealDB namespace.
- Shared logic between inner_voice and photography voice must live in a single, reusable module.
- Maintain compatibility with existing clients: no renaming or removal of current tools without explicit aliases.
- All changes must pass `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace --features db_integration`.

## Work Breakdown

1. **Refactor inner_voice core logic**
   - Extract retrieval + synthesis workflow into a helper (e.g., `inner_voice::run_voice(&SurrealMindServer, CallToolRequestParam)`).
   - Ensure the helper accepts a generic `SurrealMindServer` so it can operate on a cloned instance with a different DB handle.
   - Unit-test the helper in isolation (mock embedder, stub Grok call) if feasible.

2. **Implement `photography_voice` tool**
   - Add `handle_photography_voice` to `src/tools/photography.rs`; clone the server with `clone_with_db(db_photo)` and invoke the shared helper.
   - Register the new tool in `src/server/mod.rs`, `src/main.rs` startup log, `src/tools/detailed_help.rs`, and tool schemas/tests (`tests/tool_schemas.rs`).
   - Document expected parameters (match `inner_voice` signature) and any photography-specific defaults, if needed.

3. **Tighten photography moderation UX**
   - Option A (preferred): add `handle_photography_moderate` that simply dispatches to `handle_knowledgegraph_moderate` with the cloned photography DB. Register it as a dedicated MCP tool `photography_moderate` while keeping `photography_memories(mode="moderate")` functional for backward compatibility.
   - Update detailed help and README sections so both entry points are discoverable.

4. **Documentation & configuration updates**
   - Describe new tools in `README.md` (Photography section) and `AGENTS.md`/`fixes/20250917-ccr-implementation-plan.md` if cross references needed.
   - Add any new env vars or defaults (none anticipated, but capture if introduced).

5. **Testing & validation**
   - Run full formatting/lint/test suite as noted in Constraints.
   - Add targeted tests for photography voice/moderation (e.g., stubbed integration test that ensures the tool dispatches and returns expected schema; may use feature flags to avoid DB dependencies).

6. **Checklist before hand-off**
   - [ ] New tools appear in `surreal-mind` tool list with correct help text.
   - [ ] `photography_voice` returns grounded synthesis when photography DB has data; falls back to local summarizer otherwise.
   - [ ] `photography_moderate` surfaces review/decision workflow identical to main namespace.
   - [ ] All docs updated; no stray `.DS_Store` or unrelated changes.

## Risks & Mitigations
- **Risk:** Shared helper introduces regressions in core inner_voice.
  - *Mitigation:* Keep integration tests for main `inner_voice`; add at least one targeted regression test.
- **Risk:** Duplicate tool registration causes MCP schema drift.
  - *Mitigation:* Update tool schema tests and run them locally.
- **Risk:** Photography DB connection not initialized when tools invoked.
  - *Mitigation:* Reuse existing `connect_photo_db()` guard (already used by other photography tools).

## Deliverables
- Source updates implementing the two new photography-focused tools and inner_voice refactor.
- Updated documentation (README, detailed help) and passing test suite.
- Optional: short CHANGELOG entry summarizing the new capabilities.

