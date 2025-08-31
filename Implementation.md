# Inner Voice Refactor — RAG + Organize Plan

Purpose: Redefine `inner_voice` as a retrieval-and-synthesis tool (RAG) that optionally stages Knowledge Graph (KG) candidates, and persist the synthesized answer as a new thought. Add a lightweight lifecycle so original source thoughts can be marked for later removal and archived.

## Outcomes
- `inner_voice` takes a natural language query → retrieves top thoughts → returns `synthesized_answer` and `sources`.
- Saves `synthesized_answer` as a thought (is_summary=true) with provenance `summary_of=[source ids]`.
- When `stage_kg=true`, runs the extractor on retrieved sources and stages candidates to `kg_entity_candidates` / `kg_edge_candidates` with provenance.
- Optional: mark source thoughts for `removal` (feature-flagged, default off) to support periodical archive/cleanup.

---

## 1) Data Model Changes (Thoughts)
Add the following fields to `thoughts` (schema migration in `server::initialize_schema`):
- `is_summary`: option<bool> (default false)
- `summary_of`: option<array<string>>
- `pipeline`: option<string> (e.g., "inner_voice")
- `status`: option<string> (enum-like: "active" | "removal"). Default "active".

SurrealQL patch (idempotent):
```
DEFINE FIELD is_summary       ON TABLE thoughts TYPE option<bool>;
DEFINE FIELD summary_of       ON TABLE thoughts TYPE option<array<string>>;
DEFINE FIELD pipeline         ON TABLE thoughts TYPE option<string>;
DEFINE FIELD status           ON TABLE thoughts TYPE option<string>;
DEFINE INDEX thoughts_status_idx ON TABLE thoughts FIELDS status;
```

Notes:
- Do NOT remove existing fields. We only add.
- These are optional to keep backward compatibility.

Acceptance:
- Re-running the server does not error if fields already exist.

---

## 2) Tool API: `inner_voice` (RAG + Stage + Save)
Redefine parameters (update `schemas.rs::inner_voice_schema` and `tools/detailed_help.rs`):
- `content` (string, required): query text.
- `top_k` (int, default env SURR_TOP_K, clamp 1–50)
- `sim_thresh` (float 0.0–1.0, default env SURR_SIM_THRESH)
- `stage_kg` (bool, default false) — stage candidates from retrieved thoughts.
- `confidence_min` (float 0.0–1.0, default 0.6) — staging threshold.
- `max_nodes` (int, default 30) — cap staged entities.
- `max_edges` (int, default 60) — cap staged relationships.
- `save` (bool, default true) — persist synthesized answer as a thought (is_summary=true).
- `auto_mark_removal` (bool, default false) — set `status='removal'` on source thoughts after successful staging.

Handler flow (`tools/inner_voice.rs`):
1) Embed query, fetch all `thoughts` (id, content, embedding) up to SURR_DB_LIMIT.
2) Rank by cosine; slice top_k with sim_thresh.
3) Synthesize: lightweight extractive summary (first line of up to 3 top results concatenated, truncate ~600 chars). Leave hook to plug LLM summarizer later.
4) If `save=true`:
   - Create a new thought with `content=synthesized_answer`, `is_summary=true`, `summary_of=[source ids]`, `pipeline='inner_voice'`.
5) If `stage_kg=true` and results exist:
   - Run `HeuristicExtractor::extract` on the retrieved sources (list of texts).
   - For each entity with confidence >= `confidence_min` (take up to `max_nodes`):
     - `CREATE kg_entity_candidates ... { name, entity_type in data, data, confidence, source_thought_id }`
   - For each relationship with confidence >= `confidence_min` (take up to `max_edges`):
     - `CREATE kg_edge_candidates ... { source_name, target_name, rel_type, data, confidence, source_thought_id }`
6) If `auto_mark_removal=true` and staging performed:
   - `UPDATE thoughts SET status='removal' WHERE id IN [source ids]` (best-effort, ignore errors).
7) Return structured result:
   - `{ synthesized_answer, saved_thought_id?, sources: [{thought_id, similarity, excerpt}], staged: { pending_entities, pending_relationships }, marked_for_removal: N }`.

Acceptance:
- Handler compiles and returns correct shapes.
- `save=false` skips thought insertion entirely.

---

## 3) Detailed Storage for Saved Synthesis
When `save=true`, insert thought as:
```
CREATE type::thing('thoughts', $id) CONTENT {
  content: $synth,
  created_at: time::now(),
  embedding: $embedding,
  injected_memories: [],
  enriched_content: NONE,
  injection_scale: 0,
  significance: 0.5,
  access_count: 0,
  last_accessed: NONE,
  submode: NONE,
  framework_enhanced: NONE,
  framework_analysis: NONE,
  is_inner_voice: NONE,
  inner_visibility: NONE,
  is_summary: true,
  summary_of: $source_ids,
  pipeline: 'inner_voice',
  status: 'active'
} RETURN NONE;
```
Notes:
- You may choose not to embed the synthesized answer (set embedding to empty) or reuse the query embedding.
- Generate `$id` with UUID v4.

---

## 4) Config Flags
- `SURR_TOP_K` (default 5)
- `SURR_SIM_THRESH` (default 0.5)
- `SURR_DB_LIMIT` (default 500)
- `SURR_AUTO_MARK_REMOVAL` (default false)
- `SURR_RETENTION_DAYS` (default 30)
- `SURR_ARCHIVE_DIR` (default ./archive/)
- `SURR_ARCHIVE_FORMAT` (default parquet)

---

## 5) Maintenance Tool (separate PR)
New MCP tool `maintenance_ops` with sub-commands:
- `list_removal_candidates`
- `export_removals`
- `finalize_removal`

See acceptance and I/O details in this document when implementing.

---

## 6) Moderation & Promotion
- `knowledgegraph_moderate` promotes candidates; no changes needed here.
- After promotion, `maintenance_ops` (or a follow-up call) can mark sources to `status='removal'`.

---

## 7) Backward Compatibility
- Do not rely on `is_inner_voice` anywhere; keep field only for legacy compatibility.
- No changes to `convo_think`/`tech_think`.
- Update help text: inner_voice = RAG query + optional KG staging; saves summary when `save=true`.

---

## 8) Testing Plan
1) Seed 5 thoughts; run `inner_voice` with `top_k=3`. Expect `synthesized_answer` and 3 `sources`.
2) With `save=true`, verify summary thought exists with `is_summary=true`, `summary_of` containing source ids.
3) With `stage_kg=true`, verify candidates in `kg_entity_candidates`/`kg_edge_candidates` with `source_thought_id`.
4) With `auto_mark_removal=true`, verify sources have `status='removal'`.

---

## 9) Checklist for Worker
- [ ] Add thought fields + index in schema init.
- [ ] Replace `inner_voice` handler with RAG + staging + save logic.
- [ ] Update schemas/help for new params and description.
- [ ] Build + run unit tests.
- [ ] Manual integration check against a running SurrealDB.

