# Phase 9: Integrate Corrections Query into Existing Tooling

**Status:** Not Started  
**Parent:** remini-correction-system.md  
**Depends On:** Phase 4 (correct mode working), Phase 3 (marks), Phase 4 testing  
**Assignee:** Codex

---

## Goal
Fold correction_event querying and maintenance binaries into existing user-facing tools (maintain/search/remini), removing the need for standalone MCP tools.

## Plan (to implement now)
- Extend `maintain` tool with subcommands/task enum:
  - `corrections` → list correction_events (optional target_id, limit).
  - `rethink` → invoke gem_rethink (accept RETHINK_TYPES).
  - `health` → run scripts/sm_health.sh (staleness auto-mark).
  - `populate` → run kg_populate.
  - `embed` → run kg_embed.
  - `wander` → run kg_wander (optional).
  - `report` → show last remini report (logs/remini_report.json).
  - `tasks` list (multi-run) honoring dry_run flag.
- Keep binaries (remini, gem_rethink, kg_populate, kg_embed) for scheduled/CLI use; MCP path will go through `maintain`.
- Update schemas/docs to reflect new maintain subcommands; mark standalone `corrections` MCP tool for deprecation after integration.

## Out of scope (future):
- NL triggers for forensic mode.
- Rich health diagnostics beyond staleness mark.
- Launchd wiring for remini (already tracked in Phase 6).

---

## Review (CC, 2026-01-11)

**Strengths:**
1. **Clean consolidation** - Single `maintain` tool with subcommands instead of scattered MCP tools. Reduces surface area for LLM clients.
2. **Thematically coherent** - All operations (corrections, rethink, health, populate, embed, wander, report) are maintenance tasks. They belong together.
3. **Dual-path preserved** - Binaries stay for CLI/scheduled use, MCP goes through `maintain`. Doesn't break existing launchd jobs or scripts.
4. **Well-scoped** - NL triggers, rich diagnostics, and launchd wiring explicitly deferred. Keeps it focused.

**Concern:**
- `maintain` is getting heavy. It already handles reembed, health_check_embeddings, health_check_indexes, list_removal_candidates, export_removals, finalize_removal, etc. Adding 7-8 more subcommands could make the enum unwieldy. Not a blocker, but worth monitoring.

**Questions to consider:**
1. Will the `corrections` subcommand mirror the existing standalone `corrections` tool's parameters exactly (target_id filter, limit)?
2. For `rethink` - does it just invoke `gem_rethink` as a subprocess, or will it be a native Rust implementation?
3. The `tasks` list with `dry_run` - is this the remini multi-task runner, or something new?

**Verdict:** Green light. Dependencies look correct. Good consolidation/refactoring task with clear scope.
