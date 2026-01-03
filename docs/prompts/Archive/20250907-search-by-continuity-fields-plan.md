# Search Enhancements — Continuity Field Filters (Phase B+)

Owner: Codex • Date: 2025‑09‑07 • Updated: 2025‑09‑13

Goal
- Add targeted filters over continuity/link fields to thoughts search without altering scoring. Keep additive, optional params; no backward‑compat aliases.

Scope (filters to add)
- session_id?: string
- chain_id?: string
- previous_thought_id?: string
- revises_thought?: string
- branch_from?: string
- origin?: string
- confidence_gte?: number (0..1)
- confidence_lte?: number (0..1)
- date_from?: YYYY‑MM‑DD
- date_to?: YYYY‑MM‑DD
- order?: "created_at_asc" | "created_at_desc" (default depends on filters; see Ordering)

Non‑Goals
- No changes to injection logic or similarity scoring — filtering/sorting only.
- No migrations beyond fields ensured by maintenance (Phase B continuity fields).

API Surface
- Preferred: extend `legacymind_search` (src/tools/unified_search.rs) with the optional params above.
- Optional parity: add same params to `search_thoughts` (src/tools/search_thoughts.rs).
- Documentation: update detailed_help to include filters + examples.

Query Logic (SurrealQL)
- Base (dim‑safe):
  `WHERE embedding_dim = $dim AND embedding IS NOT NULL`
- Filters (binds shown as $... when provided):
  - `AND session_id = $sid`
  - `AND chain_id = $cid`
  - Previous link (tolerate record or string):
    `AND ((type::is::record(previous_thought_id) AND meta::id(previous_thought_id) = $prev) OR previous_thought_id = $prev)`
  - Revises link:
    `AND ((type::is::record(revises_thought) AND meta::id(revises_thought) = $rev) OR revises_thought = $rev)`
  - Branch link:
    `AND ((type::is::record(branch_from) AND meta::id(branch_from) = $br) OR branch_from = $br)`
  - `AND origin = $origin`
  - Date bounds (inclusive days): `AND created_at >= $from_date AND created_at <= $to_date`
  - Confidence (clamped to [0,1]):
    - lower: `AND confidence >= $cgte`
    - upper: `AND confidence <= $clte`
    - both: `AND confidence >= $cgte AND confidence <= $clte`
- Ordering
  - If session_id or chain_id present AND order not provided → `ORDER BY created_at ASC, similarity DESC` (natural thread read).
  - Else if order provided → honor it.
  - Else (no continuity filters) → keep current default `ORDER BY similarity DESC`.
- Limit/offset unchanged.

Indexes & Maintenance
- Ensure continuity fields and indexes exist via `maintenance_ops { subcommand: "ensure_continuity_fields" }`:
  - Fields: session_id, chain_id, previous_thought_id, revises_thought, branch_from, confidence.
  - Indexes: `idx_thoughts_session (session_id, created_at)`, `idx_thoughts_chain (chain_id, created_at)`.

Implementation Steps
1) Schemas
   - Update UnifiedSearchParams in `src/tools/unified_search.rs` with new optional params.
   - Update detailed_help (`legacymind_search`) to document filters + examples.
   - (Optional) mirror params in `src/tools/search_thoughts.rs`.
2) Handler (unified_search.rs)
   - Parse + clamp confidence bounds to [0,1].
   - Build WHERE clauses conditionally; bind dates as `YYYY-MM-DDT00:00:00Z` / `YYYY-MM-DDT23:59:59Z`.
   - Apply ordering rule above; keep dim‑first predicate.
   - Return shape unchanged.
3) (Optional) Convenience tools
   - thoughts.thread: `{ session_id, limit?, offset?, order? }` → minimal fields `{id, created_at, content, session_id, chain_id, previous_thought_id, revises_thought, branch_from, confidence}` ordered by created_at.
   - thoughts.links: `{ thought_id }` → `{ previous_thought_id?, revises_thought?, branch_from? }` resolving record ids via `meta::id()`.
4) Tests
   - Unit: param parsing, confidence clamp, date parsing, default order selection.
   - Integration: session filter → only that session, ascending by created_at; chain filter analogous; confidence bounds respected; no‑filter behavior unchanged.
5) Docs
   - detailed_help examples for session thread, chain slice, and link‑based lookups.

Acceptance Criteria
- Adding `session_id` limits results to that session; same for `chain_id`.
- With session/chain and no explicit order, results are `created_at ASC` (similarity as tie‑break).
- Confidence and date bounds constrain results correctly and are clamped.
- No change in behavior when none of the new filters are supplied.
- (Optional) Convenience tools list but do not mutate data.

Rollout
- Land schema + handler; run `maintenance_ops.ensure_continuity_fields` once; add examples to detailed_help. No client changes required.

Notes
- Guardrails: filter by `embedding_dim` first; keep similarity computation unchanged.
- Performance: continuity indexes support session/chain scans; confidence/date predicates are selective for typical ranges.

## Clarifying Questions

1. For the link fields (`previous_thought_id`, `revises_thought`, `branch_from`), the query logic handles both record and string types. Are these fields stored as SurrealDB record links (e.g., `thoughts:id`) or as strings in the database? If they are always records, the string handling might not be necessary.

2. In the Ordering section, when no continuity filters are present, it retains the current default `ORDER BY similarity DESC`. Is similarity always computed for thoughts search, even without a query? If no query is provided, how is similarity determined?

3. For the `confidence` filters (`confidence_gte`, `confidence_lte`), what should happen if the `confidence` field is null in the database? Should the query include `AND confidence IS NOT NULL` or allow nulls to pass through?

4. Are the optional convenience tools (`thoughts.thread` and `thoughts.links`) intended to be new MCP tools that need implementation, or are they just conceptual ideas for future consideration?

5. In the date bounds, dates are bound as `YYYY-MM-DDT00:00:00Z` and `YYYY-MM-DDT23:59:59Z`. Is `created_at` stored as a datetime field in ISO 8601 format? What timezone is assumed?

6. For the `origin` filter, is `origin` a standard field in thoughts? From the AGENTS.md, it seems thoughts have an `origin` field, but confirmation would help.

7. In the Implementation Steps, the optional parity addition to `search_thoughts.rs` – is `search_thoughts` a separate tool from `legacymind_search`, and does it need the same filters?

8. For the ordering rule: If `session_id` or `chain_id` is present and no explicit `order`, it orders by `created_at ASC, similarity DESC`. But if no query is provided (e.g., filtering by session only), is similarity still computed or should it default to just `created_at ASC`?

## Zed Q&A — Answers (Decisions)

1) Link Field Types (record vs string)
- Mixed in practice. Phase B created fields as `record(thoughts) | string` via maintenance. Some writers (e.g., older tools) may have stored plain string ids. Keep tolerant predicates handling both record and string. No migration required.

2) Similarity Without Query
- Similarity is only computed when a query embedding exists (i.e., `thoughts_content` provided or derived). If no query is provided, do not compute similarity; ordering uses created_at only (per `order`, defaulting as below).

3) Confidence Filter and NULLs
- When either `confidence_gte` or `confidence_lte` is supplied, add `AND confidence IS NOT NULL` to ensure numeric comparison semantics. Rows with NULL confidence are excluded only when a confidence bound is requested; otherwise they can appear.

4) Convenience Tools Scope
- They are new MCP tools (navigation only), but optional. Defer implementation unless requested. This plan ships continuity filters via `legacymind_search` first; tools can be added later as a small follow‑up.

5) Date Bounds & Timezone
- `created_at` is stored as SurrealDB datetime (`time::now()`), serialized in ISO‑8601 UTC. Bind `date_from` as `YYYY‑MM‑DDT00:00:00Z` and `date_to` as `YYYY‑MM‑DDT23:59:59Z` (inclusive day range, UTC).

6) `origin` Field Presence
- Confirmed: `origin` is a standard field on thoughts (e.g., 'human', 'inner_voice', etc.) and can be filtered directly.

7) `search_thoughts` Parity
- `search_thoughts` is a separate tool. Not required for first pass. Recommendation: update `legacymind_search` now (primary interface); add parity to `search_thoughts` later only if clients need it.

8) Ordering With/Without Query
- If `session_id`/`chain_id` present and a query embedding exists → `ORDER BY created_at ASC, similarity DESC` (thread‑first with relevance tie‑break).
- If no query embedding (no `thoughts_content`) → `ORDER BY created_at ASC` only (no similarity).
