---
id: task-6
title: >-
  Expand maintenance_ops to include kg_edges health check and detailed embedding
  reports
status: Done
assignee: []
created_date: '2026-01-01 04:55'
completed_date: '2026-01-03'
labels:
  - maintenance
  - debugging
  - embeddings
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The health_check_embeddings maintenance operation currently checks thoughts, kg_entities, and kg_observations but does NOT check kg_edges. It also doesn't provide detailed breakdown of what "mismatched_or_missing" means (is it NULL embeddings vs wrong dimensions?).

**Current gap**: kg_embed found 0 records to embed, but health_check shows 1,355 entities and 2,033 observations with mismatched_or_missing embeddings. Need visibility into what's actually wrong.

**Expand to include**:

1. kg_edges health check (currently missing)
2. Separate counts for NULL vs mismatched dimension embeddings
3. Sample record IDs for debugging (first 5 with issues)
4. Check if embedding field exists vs is NULL vs has wrong dimensions

**Related**: This would help debug why kg_embed (WHERE embedding IS NULL) finds nothing when health_check reports thousands of missing/mismatched embeddings.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Details
<!-- SECTION:IMPLEMENTATION:BEGIN -->
- **Updated `handle_health_check_embeddings`** in `src/tools/maintenance.rs`:
  - Added `kg_edges` to the health checked tables.
  - Implemented 4 distinct queries per table:
    1. **Total**: `count()`
    2. **OK**: `array::len(embedding) = $expected`
    3. **Missing**: `embedding IS NONE OR embedding = NULL`
    4. **Mismatched**: `type::is::array(embedding) AND array::len(embedding) != $expected`
  - Added sample fetching:
    - Queries `LIMIT 5` IDs for both `missing` and `mismatched` categories.
  - Updated output JSON structure to:

    ```json
    "table_name": {
      "total": 100,
      "ok": 90,
      "missing": {"count": 5, "samples": ["id1", "id2"]},
      "mismatched_dim": {"count": 5, "samples": ["id3", "id4"]},
      "unknown_state": 0
    }
    ```
<!-- SECTION:IMPLEMENTATION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 health_check_embeddings includes kg_edges table
- [x] #2 Separate NULL count from mismatched dimension count
- [x] #3 Returns sample record IDs for records with issues
- [x] #4 Clarifies whether 'missing' means field doesn't exist vs IS NULL
<!-- AC:END -->
