---
id: doc-10
title: Technical Review — task-5 kg_embed binary
type: other
created_date: '2026-01-01 02:55'
---
# Technical Review — kg_embed binary (task-5)

Linked Task: task-5
Related Doc: doc-9

## Architecture analysis
- Proposed location: `src/bin/kg_embed.rs` that calls a new library function (e.g., `run_kg_embed_missing`) similar to `run_reembed_kg` and uses `embeddings::create_embedder` with `Config::load`.
- Data flow: config -> embedder -> DB -> fetch missing embeddings (entities/edges/observations) -> build text -> embed -> update record with embedding metadata.
- Query pattern: use `WHERE embedding IS NULL` for each table; loop by batch with `LIMIT`/`START` to keep memory bounded, using `meta::id(id)` for updates.
- Edge text composition may require dereferencing entity names from `source`/`target` record links. Prefer `SELECT ..., source.name AS source_name, target.name AS target_name` to avoid N+1 queries; fallback to IDs if names missing.
- Keep this binary focused on missing-only embeddings to avoid overlap with `reembed_kg` which handles mismatched dims/model.

## Implementation considerations
- Entities: embed `name` plus description when present. Description appears to live in `data.description` (and `data.entity_type` exists); consider `"{name} - {description}"` when available.
- Observations: embed `data.content` (see `kg_populate.rs`), fallback to `name` if content is missing.
- Edges: embed `"{source_name} {rel_type} {target_name} - {data.description}"` with graceful fallbacks if any field is missing.
- Batch sizes per AC: 100 for entities/edges, 50 for observations. Implement loops with `LIMIT`/`START` and log per-batch counts.
- Idempotency: restrict to missing only and write `embedding_provider`, `embedding_model`, `embedding_dim`, `embedded_at` on success.
- Logging: print start banner, provider/model/dims, per-batch progress, and a final summary. Follow `reembed_kg.rs` style for consistency.
- Error handling: decide whether to fail-fast or skip per-record embed errors; if skipping, log enough context for replay.

## Recommendations
1. Implement as a thin binary + library function, mirroring `reembed_kg` for shared DB and embedder setup.
2. Add `DRY_RUN` and optional `LIMIT` env flags for safe testing and incremental runs.
3. Use a single query to hydrate edge names (`source.name`, `target.name`) to avoid extra DB round trips.
4. Verify SurrealDB null check syntax for embeddings in the current version (`IS NULL` vs `IS NONE`) and align tests accordingly.
5. Log and skip rows with empty text to avoid writing meaningless embeddings.
