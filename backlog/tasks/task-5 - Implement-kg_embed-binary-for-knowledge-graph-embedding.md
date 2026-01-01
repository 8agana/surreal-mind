---
id: task-5
title: Implement kg_embed binary for knowledge graph embedding
status: To Do
assignee: []
created_date: '2026-01-01 02:48'
updated_date: '2026-01-01 02:52'
labels:
  - kg-orchestration
  - surreal-mind
  - maintenance-binary
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create a binary that embeds kg_entities, kg_edges, and kg_observations that don't have embeddings yet. This will be integrated into REMini (background processing daemon) for continuous knowledge graph enhancement.

The binary should follow the pattern established by reembed.rs and use the same embedding infrastructure (text-embedding-3-small, 1536 dimensions).

Related Documentation:
- doc-9: Codex review (Technical Spec)
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Binary queries for records WHERE embedding IS NULL
- [ ] #2 Embeds entities (name + description), observations (content), edges (from + relation + to + description)
- [ ] #3 Uses existing EmbeddingProvider from config
- [ ] #4 Batches records (100 entities/edges, 50 observations)
- [ ] #5 Updates records with embedding vectors
- [ ] #6 Logs progress clearly
- [ ] #7 Idempotent - safe to re-run
- [ ] #8 Clean build with no warnings
<!-- AC:END -->
