# Claude Code: Milestone 1 Scaffold Tasks

This document describes parallelizable tasks for Milestone 1 (schema + tool interface + scaffolding). All tasks must be deterministic, avoid network I/O, and pass `make ci`.

## Ground Rules
- No external dependencies beyond existing workspace.
- Do not alter runtime behavior yet (no scoring changes). This milestone is scaffolding + persistence only.
- Backward compatible reads: tolerate missing new fields via serde defaults.
- Keep changes small and focused; follow existing style.

## Tasks

1) Schema and Persistence Wiring
- Add new fields to `thoughts` table:
  - `submode` (string, default `"problem_solving"`)
  - `framework_enhanced` (bool, default `false`)
  - `framework_analysis` (object; optional)
- Optionally extend `recalls` table with:
  - `submode_match` (bool; optional)
  - `flavor` (string; optional)
- Where: extend `initialize_schema()` and SurrealQL `DEFINE TABLE/DEFINE FIELD` statements.
- Add safe, idempotent backfill updates after schema init.
- Acceptance: `make ci` passes; server starts with existing data; SurrealDB accepts the schema.

2) Tool Interface: convo_think `submode`
- Extend the MCP tool schema to accept `submode` param with enum values: `sarcastic | philosophical | empathetic | problem_solving`.
- Default to `problem_solving` when omitted or invalid; log a warning for invalid values.
- Where: tool definition/registration and handler input parsing.
- Acceptance: `cargo test` confirms tool list includes `submode` with default; existing tests remain green.

3) Cognitive Module Scaffolding (stubs only)
- Create directory `src/cognitive/` with the following files (stubs, no runtime linkage yet):
  - `mod.rs` — module doc + re-exports placeholders, but do NOT `pub use` anything that forces compilation dependency.
  - `types.rs` — define placeholder structs for `FrameworkOutput` and comments (no logic).
  - `framework.rs` — declare a `Framework` trait signature in comments (no trait yet to avoid unused warnings until wired).
  - `ooda.rs`, `socratic.rs`, `first_principles.rs`, `root_cause.rs`, `lateral.rs` — comment stubs only.
  - `profile.rs` — comment stub outlining `Submode` enum and profile structs; no code yet.
- Important: Do NOT add `mod cognitive;` to crate root yet to avoid compile-time linkage.
- Acceptance: Files exist; `make ci` unchanged.

4) Docs and Developer Notes
- Update `README.md` or add short section to reference `submode` concept and upcoming feature flag (no behavior claims).
- Add link to this file for contributor coordination.
- Acceptance: Docs build (markdown) and remain concise.

5) Tests
- Add small tests for tool schema exposure only if the test harness already asserts tool parameters. Otherwise, defer tests to Milestone 2 when behavior is wired.
- Acceptance: Test suite remains green.

## Deliverables
- Updated schema initialization with new fields.
- convo_think tool accepts `submode` parameter with defaulting + logging.
- `src/cognitive/` stubs (no runtime linkage).
- Minimal docs updated.

## Out of Scope (Milestone 1)
- No changes to retrieval scoring, injection behavior, or orbital weights.
- No enrichment logic; no new frameworks’ heuristics.
- No feature flag toggling behavior yet.

