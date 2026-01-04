---
id: task-13
title: Fix legacymind_search entity retrieval and improve unified search
status: To Do
assignee: []
created_date: '2026-01-04 04:13'
labels:
  - surreal-mind
  - search
  - kg
  - bugfix
  - refactor
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Investigate and fix legacymind_search (unified search) not returning entities. Address chain_id filtering, similarity ordering, and add robustness/metadata improvements in entity results.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 legacymind_search returns entities reliably with and without query text
- [ ] #2 chain_id filtering includes kg_entities.source_thought_ids and other valid fields
- [ ] #3 semantic entity search orders by similarity and does not exclude relevant older entities
- [ ] #4 results include an explicit kind field (entity/relationship/observation)
- [ ] #5 tests updated or added for entity retrieval paths
<!-- AC:END -->
