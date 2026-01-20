# Knowledge Graph Audit Tracking

**Owner:** CC
**Status:** Pending
**Parent:** [proposal.md](proposal.md)

---

## Scope

- Review entity quality
- Prune stale/redundant observations
- Verify relationship integrity
- Clean up experimental data from early development

---

## Current KG Statistics

*To be populated*

| Table | Count | Notes |
|-------|-------|-------|
| thoughts | | |
| kg_entities | | |
| kg_observations | | |
| kg_edges | | |

---

## Entity Audit

### Entity Types Distribution

| Type | Count | Quality Assessment | Action |
|------|-------|-------------------|--------|
| | | | |

### Problematic Entities

| ID | Name | Issue | Resolution | Status |
|----|------|-------|------------|--------|
| | | | | |

---

## Observation Audit

### Observation Quality

| Category | Count | Notes |
|----------|-------|-------|
| High quality | | |
| Redundant | | |
| Stale | | |
| Noise | | |

### Observations to Remove

| ID | Content (truncated) | Reason | Status |
|----|---------------------|--------|--------|
| | | | |

---

## Relationship Audit

### Relationship Types

| rel_type | Count | Valid? | Notes |
|----------|-------|--------|-------|
| | | | |

### Orphaned Relationships

Relationships pointing to non-existent entities:

| ID | Source | Target | Action | Status |
|----|--------|--------|--------|--------|
| | | | | |

---

## Thought Chain Integrity

| Chain ID | Thought Count | Coherent? | Notes |
|----------|---------------|-----------|-------|
| | | | |

---

## Cleanup Queries

```sql
-- Template queries for cleanup operations

-- Find orphaned relationships
SELECT * FROM kg_edges WHERE source_id NOT IN (SELECT id FROM kg_entities);

-- Find duplicate entities by name
SELECT name, count() FROM kg_entities GROUP BY name HAVING count() > 1;

-- Find observations without embeddings
SELECT count() FROM kg_observations WHERE embedding IS NULL;

-- Find old test data (if identifiable)
-- TBD based on patterns found
```

---

## Cleanup Log

| Date | Action | Records affected | Notes |
|------|--------|------------------|-------|
| | | | |
