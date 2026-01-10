# Phase 3: wander --mode marks

**Status:** Not Started
**Parent:** [remini-correction-system.md](../remini-correction-system.md)
**Depends On:** Phase 1 (Schema)
**Can Parallel With:** Phase 2
**Assignee:** TBD

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

*To be filled during implementation*
