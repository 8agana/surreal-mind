# Implementation Plan — SurrealMind MCP CCR (2025-09-17)
Author: Codex (planner/grader)
Scope: Address defects from `20250917-codex-ccr.md`
Target implementer: Grok Code Fast 1 or Claude Code (follow instructions exactly)

---

## 1. Repair SurrealDB bootstrap regression (Blocker)
- **Files**: `src/server/mod.rs`
- **Goal**: Stop reconnecting after a successful retry-protected connection.
- **Steps**:
  1. Remove or refactor the second `Surreal::new` block at lines ~484-507.
  2. Reuse the authenticated handle from the retry loop; store it in an `Arc` immediately.
  3. Ensure `db_photo` logic still works by cloning the existing handle (or by cloning `Arc`).
  4. Retain logging/backoff; update comments to reflect single-connect behavior.
- **Validation**:
  - `cargo check --package surreal-mind --all-targets`
  - Run an integration smoke test: `cargo test --package surreal-mind --test server_init -- --ignored` (add if missing).

## 2. Guard continuity links against missing records (High)
- **Files**: `src/tools/thinking.rs`
- **Goal**: Reject/normalize `previous_thought_id`, `revises_thought`, `branch_from` when the target record does not exist.
- **Steps**:
  1. In `resolve_continuity_links`, change the `type::thing` check to verify the result set is non-empty; only then return `(Some(id), "record")`.
  2. If empty, fall back to `(None, "invalid")` and note the reason in `links_resolved`.
  3. Add telemetry warnings via `tracing::warn!` when inputs are discarded.
  4. Update unit test or add a new test covering invalid IDs.
- **Validation**:
  - `cargo test --package surreal-mind --lib tools::thinking::tests::test_invalid_continuity` (write if absent).

## 3. Fix unified search ordering (High)
- **Files**: `src/tools/unified_search.rs`
- **Goal**: Sort entity candidates by cosine similarity before truncation.
- **Steps**:
  1. After populating `scored_entities`, sort descending by `similarity` (stable if equal).
  2. Only then truncate to `top_k_mem`.
  3. Confirm relationship branch mirrors this behavior (if applicable).
  4. Add regression test capturing a high-similarity but older entity scenario.
- **Validation**:
  - `cargo test --package surreal-mind --lib tools::unified_search::tests::test_similarity_ordering`.

## 4. Harden Grok planner/synth HTTP error handling (Medium)
- **Files**: `src/tools/inner_voice.rs`
- **Goal**: Surface upstream failures instead of “No valid response”.
- **Steps**:
  1. In both `call_planner_grok` and `call_grok`, check `resp.status()` before parsing.
  2. On non-success, read body once and return `SurrealMindError::External` with status/details.
  3. Preserve existing timeout semantics.
  4. Add tracing for rate-limit (429) responses.
- **Validation**:
  - Add async test using `httpmock` or feature-gated stub to simulate 500/429.

## 5. Align maintenance export format with output (Medium)
- **Files**: `src/tools/maintenance.rs`
- **Goal**: Remove misleading “parquet” promise or implement real Parquet output.
- **Preferred quick fix**: accept `json` and name file `.json`.
- **Steps**:
  1. Update format validation to accept `json` (and reject others).
  2. Change output filename to `.json`.
  3. Adjust log messages and response payload fields accordingly.
  4. Update documentation or CLI help if necessary.
- **Validation**:
  - `cargo test --package surreal-mind --lib tools::maintenance::tests::test_export_removals_json`.

## 6. Optional follow-ups (if bandwidth allows)
- Emit telemetry when CLI KG extractor fails (`inner_voice.rs`).
- Review `src/http.rs` lock patterns after critical fixes land.

---

## Test & Verification Checklist (run after all fixes)
1. `cargo fmt --all`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace --all-features`
4. Manual smoke: `cargo run --bin surreal-mind -- --stdio` (connect once, ensure startup succeeds).
5. Document changes in `CHANGELOG.md` (2025-09-17 section).

## Handoff Notes
- Implementer must leave inline comments explaining non-obvious logic.
- Ping Codex (grader) before merging for targeted review.
- If a task proves too large, split into PRs but land blocker/high items first (1–3).
