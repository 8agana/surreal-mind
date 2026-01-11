# Phase 3: wander --mode marks

**Status:** COMPLETE (tested 2026-01-10)
**Parent:** [remini-correction-system.md](../remini-correction-system.md)
**Depends On:** Phase 1 (Schema)
**Can Parallel With:** Phase 2
**Assignee:** Codex

---

## Goal

Add mark surfacing capability to the wander tool.

---

## Deliverables

- [ ] New `marks` mode for wander
- [ ] `--for` filter parameter
- [ ] Query across thought, entity, observation tables
- [ ] Response formatting consistent with other wander modes

---

## Interface

```bash
wander --mode marks --for cc
wander --mode marks --for gemini
wander --mode marks  # all marks
```

**Response Format:**
```json
{
  "mode_used": "marks",
  "current_node": {
    "id": "entity:abc123",
    "name": "...",
    "mark_type": "correction",
    "marked_for": "cc",
    "mark_note": "...",
    "marked_by": "gemini",
    "marked_at": "..."
  },
  "queue_depth": 5,
  "guidance": "MARK REVIEW: This item was flagged for your attention...",
  "affordances": ["correct", "dismiss", "reassign", "next"]
}
```

---

## Query Logic

```sql
-- Query all tables for marks assigned to target
SELECT * FROM thought WHERE marked_for = $target ORDER BY marked_at ASC;
SELECT * FROM entity WHERE marked_for = $target ORDER BY marked_at ASC;
SELECT * FROM observation WHERE marked_for = $target ORDER BY marked_at ASC;
```

---

## Implementation Notes

- Added `marks` mode to wander (Surfaced via MCP). Input: `mode=marks`, optional `for` filter (enum cc/sam/gemini/dt/gem), reuses `visited_ids`.
- Query: pulls oldest marks across thoughts, kg_entities, kg_observations with `marked_for != NONE`, filtered by `for` when provided; excludes visited; orders by `marked_at ASC`.
- Queue depth: uses scalar `RETURN count((SELECT ...))` to avoid SurrealDB enum deserialization issues.
- Response includes `queue_depth`, `guidance` string, and affordances `["correct","dismiss","reassign","next"]`; current_node includes table/id/mark fields plus name/content when present.
