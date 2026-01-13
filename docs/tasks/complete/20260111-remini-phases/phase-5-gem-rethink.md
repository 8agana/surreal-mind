# Phase 5: gem_rethink Process

**Status:** In Progress (minimal queue processor implemented)
**Parent:** [remini-correction-system.md](../remini-correction-system.md)
**Depends On:** Phases 1-4 (full rethink tool)
**Assignee:** TBD

---

## Goal

Autonomous correction processing by Gemini - background daemon that clears the Gemini mark queue.

---

## Deliverables

- [ ] `gem_rethink` binary (like kg_populate pattern)
- [ ] Mark queue consumption logic
- [ ] Context gathering (derivatives, semantic neighbors)
- [ ] Correction execution by mark type
- [ ] Rethink report logging
- [ ] Error handling and recovery

---

## Process Flow

```
1. Query: SELECT * FROM thought, entity, observation WHERE marked_for = "gemini"
2. For each marked item:
   a. Read mark_type and mark_note
   b. Gather context:
      - Derivatives via source_thought_id
      - Semantic neighbors via embedding search
      - Related entities via relationships
   c. Based on mark_type:
      - correction: determine fix, apply with provenance
      - research: web search, enrich with findings
      - enrich: create relationships, extract entities
      - expand: explore semantically, create connected thoughts
   d. Clear mark after processing
   e. Log action to report
3. Output rethink report
```

---

## Mark Type Handling

### correction
```
Context: Original content + mark_note explaining what's wrong
Action: Generate corrected content, call rethink --correct internally
Output: CorrectionEvent with full provenance
```

### research
```
Context: Entity/thought needing more context
Action: Web search (via Gemini capabilities), internal KG search
Output: Enriched content, sources added, confidence updated
```

### enrich
```
Context: Sparse entity needing connections
Action: Analyze for relationship candidates, entity extraction
Output: New relationships, derived entities
```

### expand
```
Context: Interesting thread worth exploring
Action: Semantic wander from this point, create connected thoughts
Output: New thoughts linked to original
```

---

## Rethink Report Format

```json
{
  "run_timestamp": "2026-01-10T03:00:00Z",
  "items_processed": 15,
  "by_type": {
    "correction": 5,
    "research": 3,
    "enrich": 4,
    "expand": 3
  },
  "corrections_made": [...],
  "errors": [],
  "duration_seconds": 120
}
```

---

## Scheduling

- Designed to run overnight (REMini component)
- Can also run manually: `gem_rethink --dry-run`
- Non-destructive: all changes create provenance (current impl logs CorrectionEvent with previous_state/new_state passthrough; clears marks after)

---

## Implementation Notes
- Implemented now:
  - Standalone binary `gem_rethink` (src/bin/gem_rethink.rs), HTTP/stdio agnostic.
  - Inputs: env `GEM_RETHINK_LIMIT` (default 20), `DRY_RUN` to skip writes; hardcoded `marked_for = 'gemini'`.
  - Queue query: thoughts/kg_entities/kg_observations with marked_for gemini ordered by marked_at ASC.
  - Mark handling v1: `correction` -> create CorrectionEvent (previous_state/new_state passthrough), clear mark; others -> clear mark only.
  - Reporting: prints JSON summary (counts, errors).
  - Safety: dry-run supported; mark clearing uses RETURN NONE to avoid serialization issues.
- Still to do (future iterations):
  - Real Gemini reasoning/edits per mark_type; mutate new_state.
  - `--for` agent param, `--since`, `--limit` CLI flags instead of env-only.
  - Rich context gathering (derivatives, semantic neighbors, relationships).
  - Retry/backoff and rate-limited Gemini calls.
  - Attach `spawned_by`/`corrects_previous` provenance chains; better cascade behavior.

---

## Testing
- Dry-run smoke: `gem_rethink --dry-run --limit 3` (no mutations; ensure report writes).
- Happy path: seed a marked thought/entity/observation; run with `--limit 1`; expect mark cleared and correction_event created (correction) or new artifacts (other types).
- Empty queue: returns report with `items_processed = 0`, no errors.
- Error handling: simulate failure (e.g., deny DB write) → item logged in errors, process continues to next.
- Report contract: validate JSON fields run_timestamp/items_processed/by_type/duration_seconds; stable schema for downstream parsing.
