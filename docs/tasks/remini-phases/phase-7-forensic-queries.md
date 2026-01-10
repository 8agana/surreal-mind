# Phase 7: Forensic Queries

**Status:** Not Started
**Parent:** [remini-correction-system.md](../remini-correction-system.md)
**Depends On:** Phase 4 (CorrectionEvent records exist)
**Assignee:** TBD

---

## Goal

Enable deep provenance inspection - answer "why do we believe X?" and "what changed?"

---

## Deliverables

- [ ] `--forensic` flag on search tool
- [ ] Correction chain traversal
- [ ] Source tracking in results
- [ ] Natural language trigger detection
- [ ] Blast radius query capability

---

## Interface

```bash
search --query "REMini" --forensic

# Auto-triggered by natural language:
search --query "why do we believe REMini is simple"
search --query "what changed about the REMini architecture"
search --query "history of distributed consciousness"
```

---

## Forensic Response Format

```json
{
  "entity": {
    "id": "entity:abc123",
    "name": "REMini",
    "current_state": { ... }
  },
  "correction_chain": [
    {
      "id": "correction_event:xyz789",
      "timestamp": "2026-01-10T...",
      "previous_state": { "description": "Complex two-tier Worker/Sage..." },
      "new_state": { "description": "Simple Gemini wrapper..." },
      "reasoning": "Original was overcomplicated tangent",
      "sources": ["conversation with Sam"],
      "initiated_by": "cc"
    }
  ],
  "sources": {
    "current": ["conversation with Sam 2026-01-10", "first principles"],
    "historical": ["earlier session speculation"]
  },
  "derivation": {
    "derived_from": null,
    "derivatives": ["entity:def456", "observation:ghi789"]
  },
  "verification_status": "sam_verified"
}
```

---

## Natural Language Triggers

Detect phrases that should auto-escalate to forensic mode:

| Phrase Pattern | Action |
|----------------|--------|
| "why do we believe" | forensic |
| "how do we know" | forensic |
| "what changed about" | forensic |
| "history of" | forensic |
| "where did X come from" | forensic |
| "who said" | forensic |
| "source for" | forensic |

---

## Blast Radius Query

Answer: "What else might be affected by this being wrong?"

```sql
-- Find derivatives
SELECT * FROM entity WHERE source_thought_id = $target_thought;
SELECT * FROM observation WHERE source_thought_id = $target_thought;

-- Find semantic neighbors that might have been influenced
SELECT * FROM entity WHERE embedding <|5|> $target_embedding;

-- Find relationships involving this entity
SELECT * FROM relationship WHERE source_id = $target OR target_id = $target;
```

---

## Two-Layer Retrieval

| Layer | Trigger | Content |
|-------|---------|---------|
| Shallow (default) | Normal search | Current state, lean |
| Deep (forensic) | --forensic flag or NL trigger | Full provenance chain |

Token efficiency: forensic layer exists but isn't the default.

---

## Implementation Notes

*To be filled during implementation*

---

## Testing

*To be defined*
