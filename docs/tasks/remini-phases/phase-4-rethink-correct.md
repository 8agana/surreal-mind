# Phase 4: rethink Tool - Correct Mode

**Status:** In Progress (correct mode scaffold implemented; cascade basic)
**Parent:** [remini-correction-system.md](../remini-correction-system.md)
**Depends On:** Phase 1 (Schema), Phase 2 (rethink tool exists)
**Assignee:** TBD

---

## Goal

Implement correction execution with full provenance tracking.

---

## Deliverables

- [ ] `--correct` mode implementation
- [ ] CorrectionEvent record creation
- [ ] Previous state preservation
- [ ] Provenance chain linking
- [ ] Mark field clearing
- [ ] Optional cascade flagging

---

## Interface

```bash
rethink <target_id> --correct --reasoning "..." --sources '[...]'
rethink <target_id> --correct --reasoning "..." --sources '[...]' --cascade
```

**Parameters:**
- `target_id`: Record ID to correct
- `--reasoning`: Why it was wrong (required)
- `--sources`: JSON array of verification sources (required)
- `--cascade`: Flag derivatives for review (optional)

**Response:**
```json
{
  "success": true,
  "correction": {
    "id": "correction_event:xyz789",
    "target_id": "entity:abc123",
    "previous_state": { "description": "old value" },
    "new_state": { "description": "new value" },
    "reasoning": "...",
    "sources": ["..."],
    "initiated_by": "cc"
  },
  "derivatives_flagged": 3  // if --cascade used
}
```

---

## Workflow

```
1. Query target record → store as previous_state
2. Apply correction → new_state
3. Create CorrectionEvent with full provenance
4. Clear mark fields on target
5. If --cascade: query derivatives (source_thought_id), mark each
6. Return summary
```

---

## Provenance Model

```
Original Entity ─────────────────────────────────────────────────┐
  │                                                               │
  └── CorrectionEvent_1 (corrects: null)                         │
        │ previous_state: { old }                                 │
        │ new_state: { fixed }                                    │
        │ reasoning, sources, initiated_by                        │
        │                                                         │
        └── CorrectionEvent_2 (corrects_previous: CE_1)          │
              │ previous_state: { fixed }                         │
              │ new_state: { refined }                            │
              │ ...                                               │
              │                                                   │
              └── (chain continues)                               │
                                                                  │
Original Entity now has latest state ←────────────────────────────┘
Correction chain preserves full history
```

---

## Implementation Notes

- Added `correct` mode to rethink tool:
  - Requires `reasoning` (string) and `sources` (array of strings).
  - Captures `previous_state`, logs `CorrectionEvent` with provenance, clears mark fields on target.
  - `new_state` currently mirrors existing record (no content mutation yet).
  - Optional `cascade` (thought targets only): flags related `kg_entities` and `kg_observations` via `source_thought_ids`.
- Schema updated: rethink input supports `mode` enum [`mark`,`correct`], optional `cascade`.
- Still needed: payload-driven field updates to produce a real `new_state`, richer cascade rules, provenance chaining (`corrects_previous`).
