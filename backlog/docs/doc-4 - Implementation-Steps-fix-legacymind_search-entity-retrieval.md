---
id: doc-4
title: Implementation Steps - fix legacymind_search entity retrieval
type: other
created_date: '2026-01-04 04:13'
updated_date: '2026-01-04 04:13'
---
# Implementation Steps - fix legacymind_search entity retrieval

Linked task: `backlog/tasks/task-13 - Fix-legacymind_search-entity-retrieval-and-improve-unified-search.md`

## Goal
Fix `legacymind_search` (unified search) so entity results are reliable, especially under `chain_id` filtering and semantic search. Improve result metadata and robustness.

## Key Fixes
1) **Chain ID filter for entities**
- Current filter only checks `data.source_thought_id` and `data.staged_by_thought`.
- Add support for `source_thought_ids` (array, used by kg_populate) and any other known fields.
- Example predicate:
  - `data.source_thought_id IN (...) OR data.staged_by_thought IN (...) OR array::contains(source_thought_ids, <thought_id>)`

2) **Similarity ordering for semantic entity search**
- Use `ORDER BY similarity DESC` when `q_emb` is present, instead of `created_at DESC`.
- This prevents older but highly relevant entities from being excluded.

3) **Explicit kind field in results**
- Add `{ "kind": "entity" | "relationship" | "observation" }` to each item.
- Allows consumers to distinguish results without guessing based on fields.

4) **Robust fallback if semantic results are empty**
- If semantic query returns 0 rows, fall back to name-based or recent entities.
- Ensures entities are returned even when semantic query misses.

## Implementation Steps

### 1) Update entity SQL for chain_id
File: `src/tools/unified_search.rs`
- Modify the entity SQL to include `source_thought_ids` containment:
  - Use `array::contains(source_thought_ids, <thought_id>)` or `source_thought_ids CONTAINS <id>` depending on SurrealDB syntax in use elsewhere.
- Ensure it works for both semantic and non-semantic branches.

### 2) Semantic entity ordering
- When `q_emb` is present, use:
  - `ORDER BY similarity DESC LIMIT ...`
- Remove the `created_at DESC` ordering for semantic searches.

### 3) Add kind tags in results
- For entity/relationship/observation results, inject `"kind": "entity"` etc in the JSON before returning.
- Do this in all three target sections.

### 4) Add fallback when semantic query yields zero
- After semantic search, if `scored_entities.is_empty()`:
  - Run a name‑like search (if `name_like` exists) else recent entities.
- Keep limit to `top_k_mem`.

### 5) Tests
- Add tests around entity search:
  - Semantic ordering (high similarity beats recent)
  - Chain_id filter respects `source_thought_ids`
  - Fallback path returns entities when semantic returns none

## Optional Improvements
- Consolidate duplicate entity/observation query logic into helper functions.
- Allow `query.text` to be used as `name_like` fallback if `query.name` missing.

## Notes
- User stated embeddings are being maintained; do not assume missing embeddings.
- Keep changes minimal and localized to `unified_search.rs` unless a helper is clearly reused.
