# Thoughts → Knowledge Graph Pipeline (Concept)

## Context
We currently store free-form agent output in `thoughts` and rely on manual conversions to the knowledge graph when information needs to persist. This has led to duplicate storage, inconsistent recall, and missed opportunities to structure important context.

## Proposed Direction
Introduce a repeatable pathway that promotes significant thoughts into structured KG entities/observations without losing the quick capture qualities of the existing `thoughts` tool.

### Goals
- Preserve low-friction logging for transient inner voice
- Automatically promote durable information into KG nodes/edges
- Maintain lineage (which thought spawned which KG entry)
- Avoid regressions in existing MCP tools (`inner_voice`, search, moderation)

### Potential Components
1. **Tagging & Criteria**
   - Thoughts tagged `decision`, `handoff`, or above a confidence threshold trigger review/promotion
   - Optional manual flag (`promote_to_kg = true`) from tools like `inner_voice`
2. **Transformation Helper**
   - MCP task converts thought → `kg_entity`/`kg_observation`
   - Reuses auto-embedding helper so metadata stays consistent
   - Adds relationships (`revises`, `branch_from`, `chain_id`)
3. **Review Surface**
   - Queue promoted items for confirmation (similar to `memories_moderate`)
   - Allow edits before final insert into KG
4. **Backfill Script**
   - One-off job to migrate existing high-value chains (handoffs, implementation plans)

### Open Questions
- Should transient thoughts expire once promoted? (Archive vs. delete)
- How to capture multi-thought narratives—single KG node or linked sequence?
- How do photography namespaces interact with the promotion rules?

### Next Steps (when bandwidth available)
- Define promotion rules & tagging format
- Implement MCP helper for transformation + review
- Update docs & tooling to encourage structured capture
