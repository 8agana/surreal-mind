---
id: task-11
title: Build "wander" - Interactive knowledge graph exploration tool
status: Done
assignee: []
created_date: '2026-01-03 22:35'
updated_date: '2026-01-05 00:12'
labels:
  - contemplation
  - graph-exploration
  - knowledge-discovery
  - UX
  - legacymind
milestone: LegacyMind Infrastructure
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Interactive knowledge graph exploration tool that enables deep thinking and serendipitous discovery. Addresses the "local maxima problem" where semantic search gets trapped in dominant clusters (consciousness observations dominating all queries regardless of relevance).

## Core Capabilities

1. **Semantic traversal** - explore similar/related items from current position
2. **Random jump ("Surprise Me")** - escape filter bubbles via random selection
3. **Keep you in flow** - no context switching to data manipulation
4. **Metadata routing** - tags, origin, topic as primary navigation (not just semantic)
5. **View history** - prevent circling back to same items
6. **Recency bias option** - surface recent entries buried under semantic density

## Design Principles

- "Surprise Me" is NECESSARY, not optional (only way to escape dominant attractors)
- Support Sam's creative process (orbital mechanics → context injection type leaps)
- Random selection defeats local maxima where semantic search fails
- Tool should prompt synthesis when showing unrelated items

## Implementation Notes

Waiting for entity search bug fix first, but design and architecture ready to proceed. Key insight from Session 4 contemplation work: merged curiosity into thoughts table, enabling random collisions across ALL domains rather than isolated curiosity entries. This makes the graph exploration tool vastly more powerful - it's not just browsing notes, it's genuine serendipitous knowledge discovery across the entire consciousness substrate.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Can navigate graph without getting trapped in consciousness cluster
- [ ] #2 Random selection provides genuine serendipity across domains
- [ ] #3 Metadata filtering works as primary navigation method
- [ ] #4 Keeps user in contemplative flow state
- [ ] #5 View history prevents circling back to same items
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
**TESTING STATUS (2026-01-04)**

Three modes tested. Current findings:

✅ **RANDOM MODE**: Working correctly - successfully returns random entity from graph
✅ **META MODE**: Working correctly - returns recent thought with enriched_content showing nearby entities  
❌ **SEMANTIC MODE**: Bug identified - cosine similarity function receiving NONE instead of embedding array
   - Error: "Incorrect arguments for function vector::similarity::cosine(). Argument 1 was the wrong type. Expected a array but found NONE"
   - Root cause: ID→embedding lookup path broken - finds the record but can't retrieve the embedding field
   - Likely issue: embedding field not persisting or wrong field accessor in SurrealQL query

**Next Steps**:
1. Debug embedding field persistence in kg_populate (verify embeddings are actually stored)
2. Check SurrealQL query path for embedding retrieval
3. Test field accessor syntax against actual stored records
4. Re-test semantic mode once embedding lookup fixed

Waiting for entity search bug fix first, but design and architecture ready to proceed. Key insight from Session 4 contemplation work: merged curiosity into thoughts table, enabling random collisions across ALL domains rather than isolated curiosity entries. This makes the graph exploration tool vastly more powerful - it's not just browsing notes, it's genuine serendipitous knowledge discovery across the entire consciousness substrate.
<!-- SECTION:NOTES:END -->
