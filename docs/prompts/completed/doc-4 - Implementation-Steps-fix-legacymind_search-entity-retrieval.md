[Reading 68 lines from start (total: 69 lines, 1 remaining)]

---

id: doc-4
title: Implementation Steps - fix legacymind_search entity retrieval
type: other
created_date: '2026-01-04 04:13'
updated_date: '2026-01-04 04:40'
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

1) **Similarity ordering for semantic entity search**

- Use `ORDER BY similarity DESC` when `q_emb` is present, instead of `created_at DESC`.
- This prevents older but highly relevant entities from being excluded.

1) **Explicit kind field in results**

- Add `{ "kind": "entity" | "relationship" | "observation" }` to each item.
- Allows consumers to distinguish results without guessing based on fields.

1) **Robust fallback if semantic results are empty**

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

## Implemented Changes (2026-01-04)

1. **Enhanced Chain ID Filtering**:
    - Implemented a reusable `chain_filter_sql` helper.
    - Updated query to check for `source_thought_ids CONTAINSANY (thoughts in chain)` in addition to `source_thought_id` and `staged_by_thought`.

2. **Semantic Ordering**:
    - Changed `ORDER BY` to `similarity DESC` (instead of `created_at DESC`) whenever a semantic embedding query is performed for Entities and Observations.

3. **Result Tagging**:
    - Injected `"kind": "entity"`, `"kind": "relationship"`, and `"kind": "observation"` into all search results.
    - Added a default `similarity: 0.0` field for non-semantic results to ensure consistent schema.

4. **Robust Fallback**:
    - Implemented fallback logic for both Entities and Observations.
    - If a semantic search returns 0 results (due to `sim_thresh` filtering), the system now automatically executes a fallback query (Name-based if `name` is present, or Recent-based otherwise) to ensure the user receives relevant context.
