# Phase 9: Integrate Corrections Query into Existing Tooling

**Status:** Not Started  
**Parent:** remini-correction-system.md  
**Depends On:** Phase 4 (correct mode working), Phase 3 (marks), Phase 4 testing  
**Assignee:** TBD

---

## Goal
Fold the new `corrections` listing capability into an existing tool surface (likely `wander` marks mode or `detailed_help`/`maintain`) to avoid an extra MCP tool.

## Deliverables
- Decision on host surface (wander marks, maintain, or unified search/help)
- Implement query path for correction_events with target filter & limit
- Remove or deprecate standalone `corrections` MCP tool
- Update docs and tests accordingly

## Notes
- Current stopgap: `corrections` MCP tool lists correction_events with optional `target_id`, `limit`.
- Ensure provenance chain fields (corrects_previous/spawned_by) remain visible in the chosen surface.
