# Phase 8: Confidence Decay & Learning

**Status:** In Progress (implementation plan aligned with current schema)
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
| SDK/API docs | High | ~90 days | "SurrealDB syntax" |
| Tool versions | High | ~60 days | "rmcp 0.9 features" |
| Architecture decisions | Medium | ~365 days | "use Rust for MCPs" |
| Workflow patterns | Medium | ~180 days | "delegation to Gemini" |
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
5. **Embed/Populate touch**: Extraction/embedding updates the record

---

## Freshness Fields

```sql
-- Apply to kg_entities and kg_observations (and optionally thoughts)
DEFINE FIELD volatility ON TABLE kg_entities TYPE string DEFAULT "medium";
DEFINE FIELD last_refreshed ON TABLE kg_entities TYPE datetime;
DEFINE FIELD refresh_count ON TABLE kg_entities TYPE int DEFAULT 0;
DEFINE FIELD decay_confidence ON TABLE kg_entities TYPE float;

DEFINE FIELD volatility ON TABLE kg_observations TYPE string DEFAULT "medium";
DEFINE FIELD last_refreshed ON TABLE kg_observations TYPE datetime;
DEFINE FIELD refresh_count ON TABLE kg_observations TYPE int DEFAULT 0;
DEFINE FIELD decay_confidence ON TABLE kg_observations TYPE float;
```

---

## Auto-Marking Stale Items

REMini health check includes:

```sql
-- Find high-volatility items past half-life without refresh
LET $stale = SELECT id FROM kg_entities
WHERE volatility = "high"
AND time::since(last_refreshed) > duration("90d")
AND marked_for IS NULL;

UPDATE $stale SET
  marked_for = "gemini",
  mark_type = "research",
  mark_note = "Auto-flagged: high volatility, stale"
RETURN NONE;
```

---

## Correction-as-Training-Data

### Queries for Meta-Learning

```sql
-- Which source types hold up vs need re-correction?
SELECT sources, count() as total,
       count(IF corrects_previous IS NOT NULL THEN 1 END) as re_corrected
FROM correction_events
GROUP BY sources;

-- What's the error rate by entity_type?
SELECT target_table, count() as corrections
FROM correction_events
GROUP BY target_table;

-- Sam-verified corrections re-correction rate
SELECT count() as total,
       count(IF corrects_previous IS NOT NULL THEN 1 END) as re_corrected
FROM correction_events
WHERE verification_status = "sam_verified";
```

### Learning Insights

- Sources with low re-correction rate → weight heavily in future
- Entity types with high correction rate → higher volatility classification
- Sam-verified items → near-zero re-correction → trust anchors

Additional notes:
- Schema fields (volatility, last_refreshed, refresh_count, decay_confidence) added to kg_entities/kg_observations.
- Health script (`scripts/sm_health.sh`) marks stale high-vol entities (beyond half-life, default 90d) for gemini research, capped by STALENESS_LIMIT.

---

## Implementation Notes

Planned steps:
1) Schema: add volatility/last_refreshed/refresh_count/decay_confidence to kg_entities/kg_observations (and thoughts if desired).
2) Config: map volatility → half-life (config file), decay_factor=0.5.
3) Instrumentation: on search/wander/remini (populate/embed/rethink), update last_refreshed + refresh_count; on correction, also reset decay_confidence to confidence_initial.
4) Health task (remini): compute decay_confidence nightly; mark stale high-vol items (respect DRY_RUN); configurable limit to avoid large blasts.
5) Training queries: add saved queries or scripts to surface correction quality metrics.
6) Testing: fixtures with high/medium/zero volatility; ensure only high-vol stale items auto-mark; verify decay_confidence decreases over time.

---

## Testing

*To be defined (post schema + instrumentation)*
