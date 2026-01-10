# Phase 8: Confidence Decay & Learning

**Status:** Future
**Parent:** [remini-correction-system.md](../remini-correction-system.md)
**Depends On:** Phases 1-7 (full correction system operational)
**Assignee:** TBD

---

## Goal

Knowledge freshness management and meta-learning from correction patterns.

---

## Deliverables

- [ ] Volatility classification system
- [ ] Confidence decay model
- [ ] Freshness tracking fields
- [ ] Auto-marking stale items
- [ ] Correction-as-training-data queries
- [ ] Learning metrics dashboard

---

## Volatility Classification

| Entity Type | Volatility | Half-Life | Example |
|-------------|------------|-----------|---------|
| SDK/API docs | High | ~3 months | "SurrealDB syntax" |
| Tool versions | High | ~2 months | "rmcp 0.9 features" |
| Architecture decisions | Medium | ~1 year | "use Rust for MCPs" |
| Workflow patterns | Medium | ~6 months | "delegation to Gemini" |
| Personal history | Zero | Permanent | "Sam's Iraq service" |
| Relationship facts | Zero | Permanent | "Crystal is fiancée" |

---

## Decay Model

```
confidence_current = confidence_initial * decay_factor^(days_since_refresh / half_life)

where:
- confidence_initial = confidence at last refresh
- decay_factor = 0.5 (halves at half-life)
- days_since_refresh = time since last: retrieval, verification, or correction
- half_life = based on volatility classification
```

### Refresh Events (reset decay timer)

1. **Re-retrieval**: Entity used in a search/wander result
2. **Verification**: Sources checked, confirmed still valid
3. **Correction**: Explicitly updated via rethink
4. **Cross-reference**: Other entities link to it

---

## Freshness Fields

```sql
DEFINE FIELD volatility ON TABLE entity TYPE string DEFAULT "medium";
DEFINE FIELD last_refreshed ON TABLE entity TYPE datetime;
DEFINE FIELD refresh_count ON TABLE entity TYPE int DEFAULT 0;
DEFINE FIELD decay_confidence ON TABLE entity TYPE float;  -- computed field
```

---

## Auto-Marking Stale Items

REMini health check includes:

```sql
-- Find high-volatility items past half-life without refresh
SELECT * FROM entity
WHERE volatility = "high"
AND time::since(last_refreshed) > duration("90d")
AND marked_for IS NULL;

-- Mark them for re-verification
UPDATE entity SET
  marked_for = "gemini",
  mark_type = "research",
  mark_note = "Auto-flagged: high volatility, not refreshed in 90+ days"
WHERE id IN $stale_ids;
```

---

## Correction-as-Training-Data

### Queries for Meta-Learning

```sql
-- Which source types hold up vs need re-correction?
SELECT sources, count() as total,
       count(IF corrects_previous IS NOT NULL THEN 1 END) as re_corrected
FROM correction_event
GROUP BY sources;

-- What's the error rate by entity_type?
SELECT target_table, count() as corrections
FROM correction_event
GROUP BY target_table;

-- Sam-verified corrections re-correction rate
SELECT count() as total,
       count(IF corrects_previous IS NOT NULL THEN 1 END) as re_corrected
FROM correction_event
WHERE verification_status = "sam_verified";
```

### Learning Insights

- Sources with low re-correction rate → weight heavily in future
- Entity types with high correction rate → higher volatility classification
- Sam-verified items → near-zero re-correction → trust anchors

---

## Implementation Notes

This phase is explicitly future work. Prerequisites:
1. Correction system operational (Phases 1-6)
2. Enough correction history to analyze patterns
3. Clear volatility classification rules established

*To be filled during implementation*

---

## Testing

*To be defined*
