# Implementation Plan — Automatic KG Embeddings

**Author:** Codex  
**Date:** 2025-09-18  
**Executor:** (TBD – recommended: GPT-5 High Reasoning / Warp)

## Objectives
- Automatically persist embedding vectors and metadata for every KG entity/observation created through `memories_create` or promoted via moderation.
- Ensure photography namespace mirrors the primary namespace (same embedding helper works against cloned DB handles).
- Preserve existing schemas and external tool contracts while improving retrieval quality.

## Requirements & Constraints
- Continue using the active embedder selected at startup (`SurrealMindServer::get_embedding_metadata`).
- Avoid blocking writes when embedding fails; emit structured warnings and return success.
- Relationships (`kg_edges`) remain non-embedded.
- Maintain compatibility with existing db fixtures/tests; new tests may be gated behind `--features db_integration` or `RUN_DB_TESTS=1`.

## Work Breakdown

1. **Introduce reusable embedding helper**
   - Add `ensure_kg_embedding(&self, table: &str, id: &str, name: &str, data: &serde_json::Value)` in a shared location (e.g., new `kg_embedding.rs` module or inside `knowledge_graph.rs`).
   - Build text payload using existing `build_kg_text` logic (name + entity_type/description).
   - Call `self.embedder.embed(&text)`; on success, `UPDATE type::thing($table, $id)` with `embedding`, `embedding_model`, `embedding_provider`, `embedding_dim`, `embedded_at`.
   - On failure: log `warn!` with table/id and continue without error.

2. **Wire helper into KG creation paths**
   - After creating entities/observations in `handle_knowledgegraph_create`, invoke helper (skip when record not `created` or when `kind == relationship`).
   - Ensure `id` passed to helper is raw Surreal ID (strip prefix if needed) and tests cover both main and photography DB clones.

3. **Wire helper into moderation approvals**
   - In `handle_knowledgegraph_moderate`, after approving an entity/observation candidate and creating/updating rows, call helper when the record lacks an embedding array.
   - Keep existing reuse path (if candidate already carries `embedding`, write it directly and skip helper).

4. **Photography parity**
   - Confirm helper uses `self.db` (works with `clone_with_db`); add targeted call in the photography server clone test if necessary.
   - Document that photography KG now embeds automatically.

5. **Telemetry & logging**
   - Standardize log messages: `inner_voice`-style `tracing::warn!("kg_embedding_failed", table, id, error)`.
   - Add debug logs when embeddings are persisted (optional, ensure they respect `MCP_NO_LOG`).

6. **Validation & testing**
   - Add db-integration test (e.g., `tests/kg_embedding_autoset.rs`) that:
     - Creates an entity via `memories_create`, asserts embedding metadata non-null.
     - Approves a staged candidate and verifies embedding.
   - Update `check_kg_embeddings.sh` messaging to reference automatic embedding.
   - Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --features db_integration` (document RUN_DB_TESTS expectation).

## Risks & Mitigations
- **Embedder latency:** Calls on each create could slow write paths. *Mitigation:* keep asynchronous helper, tolerate errors, consider future batching if needed.
- **Dimension drift:** Helper must use `get_embedding_metadata()` to align provider/model/dim with system configuration.
- **DB contention:** Updates immediately after CREATE may conflict under heavy load. *Mitigation:* same transaction block, retry once on conflict if needed (log otherwise).

## Deliverables
- Updated Rust source implementing automatic embeddings.
- New/updated tests ensuring embeddings are set.
- Refreshed `check_kg_embeddings.sh` output and relevant documentation (CHANGELOG/README snippet if desired).
