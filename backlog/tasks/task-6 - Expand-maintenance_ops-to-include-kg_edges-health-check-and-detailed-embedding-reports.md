---
id: task-6
title: >-
  Expand maintenance_ops to include kg_edges health check and detailed embedding
  reports
status: To Do
assignee: []
created_date: '2026-01-01 04:55'
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

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 health_check_embeddings includes kg_edges table
- [ ] #2 Separate NULL count from mismatched dimension count
- [ ] #3 Returns sample record IDs for records with issues
- [ ] #4 Clarifies whether 'missing' means field doesn't exist vs IS NULL
<!-- AC:END -->
