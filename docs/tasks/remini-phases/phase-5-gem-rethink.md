# Phase 5: gem_rethink Process

**Status:** Not Started
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
- Non-destructive: all changes create provenance

---

## Implementation Notes

*To be filled during implementation*

---

## Testing

*To be defined*
