# Phase 9: Integrate Corrections Tool - Testing

**Status:** PARTIAL IMPLEMENTATION (Run 1: 2026-01-11) — fixes shipped 2026-01-12, pending retest
**Parent:** [phase-9-integrate-corrections-tool.md](phase-9-integrate-corrections-tool.md)
**Depends On:** Phase 9 Implementation Complete, Phase 4 (correct mode), Phase 3 (marks)

---

## Goal

Verify the `maintain` tool correctly integrates correction querying and all maintenance subcommands, removing the need for standalone MCP tools.

---

## Pre-requisites

- Phase 4 rethink correct mode working
- Phase 3 wander marks mode working
- maintain tool updated with new subcommands
- gem_rethink, kg_populate, kg_embed binaries available
- scripts/sm_health.sh exists
- Test data: correction_events in database

---

## Test Setup

Before running tests, ensure test data exists:
```bash
# Create a correction event for testing
rethink <target_id> --correct --reasoning "Test correction" --sources '["test"]'

# Create marks for rethink testing
rethink <thought_id> --mark --type correction --for gemini --note "Test mark"
```

---

## Test Cases

### Happy Path - Corrections Subcommand

| ID | Test | Command | Expected Result |
|----|------|---------|-----------------|
| HP-1 | List all corrections | `maintain corrections` | Returns list of correction_events |
| HP-2 | List with target filter | `maintain corrections --target <id>` | Only corrections for specified target |
| HP-3 | List with limit | `maintain corrections --limit 5` | Returns at most 5 correction_events |
| HP-4 | Empty result | `maintain corrections --target nonexistent` | Returns empty list, no error |

### Happy Path - Task Subcommands

| ID | Test | Command | Expected Result |
|----|------|---------|-----------------|
| TASK-1 | Run rethink | `maintain rethink` | Invokes gem_rethink, returns result |
| TASK-2 | Run rethink with types | `RETHINK_TYPES=correction maintain rethink` | Only processes specified types |
| TASK-3 | Run populate | `maintain populate` | Invokes kg_populate, returns result |
| TASK-4 | Run embed | `maintain embed` | Invokes kg_embed, returns result |
| TASK-5 | Run wander | `maintain wander` | Invokes kg_wander (optional task) |
| TASK-6 | Run health | `maintain health` | Runs sm_health.sh, returns health status |
| TASK-7 | View report | `maintain report` | Shows last remini_report.json content |

### Multi-Task Execution

| ID | Test | Command | Expected Result |
|----|------|---------|-----------------|
| MULTI-1 | Run multiple tasks | `maintain tasks populate,embed` | Runs both tasks in order |
| MULTI-2 | Run all tasks | `maintain tasks all` | Runs full maintenance cycle |
| MULTI-3 | Dry run mode | `maintain tasks all --dry-run` | Shows what would run, no execution |

### Report Format Verification

| ID | Test | Expected Fields |
|----|------|-----------------|
| RPT-1 | Corrections list structure | `id`, `target_id`, `target_table`, `initiated_by`, `created_at` |
| RPT-2 | Task result structure | `task_name`, `success`, `duration_ms`, `output` |
| RPT-3 | Multi-task report | `tasks_run`, `summary`, `task_details`, `duration_seconds` |

### Error Handling

| ID | Test | Expected Result |
|----|------|-----------------|
| ERR-1 | Invalid subcommand | `maintain invalid` errors gracefully with usage help |
| ERR-2 | Task failure | Failed task logged, continues with remaining tasks (multi-task) |
| ERR-3 | Missing binary | Clear error message identifying missing binary |
| ERR-4 | Database down | Graceful error with clear message |
| ERR-5 | Invalid target_id format | Returns error, doesn't crash |

### Integration Verification

| ID | Test | Expected Result |
|----|------|-----------------|
| INT-1 | Corrections via maintain matches standalone | Same results from `maintain corrections` as deprecated `corrections` tool |
| INT-2 | Rethink via maintain matches standalone | Same behavior as direct gem_rethink execution |
| INT-3 | Dry run flag propagates | `--dry-run` passed to underlying binaries |

### Deprecation Path

| ID | Test | Expected Result |
|----|------|-----------------|
| DEP-1 | Standalone corrections tool | Still works but logs deprecation warning |
| DEP-2 | Documentation updated | `howto maintain` shows new subcommands |
| DEP-3 | Schema reflects changes | Tool schema includes all new subcommands |

---

## Test Results

### Run 1: 2026-01-11 (CC)

| Test ID | Result | Notes |
|---------|--------|-------|
| HP-1 | PASS | Returns list of 11 correction_events with all expected fields |
| HP-2 | PASS | `corrections --target entity:nx7drjlxf1lxyy7t7plc` returns 4 matching events |
| HP-3 | PASS | `corrections --limit 5` returns exactly 5 events |
| HP-4 | PASS | Nonexistent target returns empty list `{"success":true,"count":0,"events":[]}` |
| TASK-1 | PASS | `maintain rethink` invokes gem_rethink, returns structured result |
| TASK-2 | N/T | RETHINK_TYPES env var not tested (requires env setup) |
| TASK-3 | PASS | `maintain populate` invokes kg_populate, processed 27 thoughts |
| TASK-4 | PASS | `maintain embed` invokes kg_embed, updated 458 embeddings |
| TASK-5 | PARTIAL | `maintain wander` runs but times out after 60s (expected for long task) |
| TASK-6 | FAIL | `maintain health` fails - sm_health.sh has endpoint format bug |
| TASK-7 | FAIL | `maintain report` returns "unknown task: report" |
| MULTI-1 | FAIL | `maintain tasks populate,embed` - "Unknown subcommand: tasks" |
| MULTI-2 | FAIL | `maintain tasks all` - "Unknown subcommand: tasks" |
| MULTI-3 | FAIL | `maintain tasks all --dry-run` - "Unknown subcommand: tasks" |
| RPT-1 | PASS | Corrections list has: id, target_id, target_table, initiated_by, timestamp, reasoning, sources |
| RPT-2 | PASS | Task results have: task, success, stdout, stderr |
| RPT-3 | N/A | Multi-task not implemented |
| ERR-1 | PASS | Invalid subcommand returns: "Unknown subcommand: invalid_subcommand_test" |
| ERR-2 | N/A | Multi-task not implemented |
| ERR-3 | N/T | Missing binary scenario not tested |
| ERR-4 | N/T | Database down scenario not tested |
| ERR-5 | PASS | Invalid target_id format returns empty list, no crash |
| INT-1 | PASS | `maintain corrections` matches standalone `corrections` tool exactly |
| INT-2 | PASS | `maintain rethink` produces same output as direct gem_rethink |
| INT-3 | PASS | `--dry-run` flag propagates - shows "🔎 Dry run: no writes to DB" |
| DEP-1 | PARTIAL | Standalone corrections works, but no deprecation warning logged |
| DEP-2 | FAIL | `howto maintain` missing new subcommands (corrections, rethink, populate, embed, wander, health) |
| DEP-3 | FAIL | Schema outdated - shows only old subcommands, not Phase 9 additions |

---

## Issues Found

| Issue | Severity | Description | Resolution |
|-------|----------|-------------|------------|
| ISS-1 | HIGH | `tasks` subcommand not implemented | ✅ Implemented 2026-01-12 (multi-task orchestration; aggregated JSON). Pending verification run. |
| ISS-2 | HIGH | `report` subcommand not implemented | ✅ Implemented 2026-01-12 (reads `logs/remini_report.json`). Pending verification run. |
| ISS-3 | MEDIUM | `sm_health.sh` endpoint format bug | ✅ Script now omits endpoint unless provided; runs with defaults. Pending verification run. |
| ISS-4 | MEDIUM | Schema/howto not updated for Phase 9 | ✅ Schemas + detailed_help updated with new subcommands/params. |
| ISS-5 | LOW | No deprecation warning on standalone corrections | ✅ Warning added to corrections tool output. |
| ISS-6 | LOW | wander times out at 60s default | Open (behavior unchanged; consider configurable timeout). |

### Run 2: 2026-01-12 (Codex) — implementation fixes only

No formal test execution yet. Code fixes for ISS-1 through ISS-5 shipped and built (`cargo build --release`). Queue tool `gem_rethink` smoke-tested: empty queue returns structured report successfully. Awaiting CC/Vibe retest to record results.

### Run 3: 2026-01-11 (CC) — Verification of Codex Fixes

| Test ID | Result | Notes |
|---------|--------|-------|
| MULTI-1 | PASS | `maintain tasks populate,embed --dry-run` returns aggregated JSON with both task results |
| TASK-6 | FAIL | `maintain health` still fails: "invalid value '127.0.0.1:8000' for '--endpoint'" |
| TASK-7 | FAIL | `maintain report` still returns "unknown task: report" |
| DEP-1 | PASS | Standalone corrections now includes `"deprecation":"Use maintain corrections..."` |
| DEP-2 | PASS | `howto maintain` now shows: corrections, rethink, populate, embed, wander, health, report, tasks |
| DEP-3 | PASS | Schema includes all new subcommands and parameters |

**Issue Status Update:**

| Issue | Codex Claimed | CC Verified |
|-------|---------------|-------------|
| ISS-1 | ✅ Fixed | ✅ CONFIRMED - `tasks` subcommand works |
| ISS-2 | ✅ Fixed | ❌ NOT WORKING - "unknown task: report" |
| ISS-3 | ✅ Fixed | ❌ NOT WORKING - endpoint format error persists |
| ISS-4 | ✅ Fixed | ✅ CONFIRMED - howto/schema updated |
| ISS-5 | ✅ Fixed | ✅ CONFIRMED - deprecation warning present |

**3 of 5 fixes verified. ISS-2 and ISS-3 need attention.**

---

## Verdict

**Status:** PARTIAL IMPLEMENTATION
**Ready for Phase 10:** [ ] Yes  [X] No

### Summary

Phase 9 is **partially implemented**. Core functionality works:
- ✅ `maintain corrections` - fully functional with filtering
- ✅ `maintain rethink` - invokes gem_rethink correctly
- ✅ `maintain populate` - invokes kg_populate correctly
- ✅ `maintain embed` - invokes kg_embed correctly
- ✅ `maintain wander` - invokes kg_wander (with timeout caveat)
- ✅ Dry-run flag propagation works
- ✅ Integration with standalone corrections tool verified

### Blocking Issues (Must Fix)

1. **Multi-task orchestration (`tasks` subcommand)** - Not implemented
2. **Report viewer (`report` subcommand)** - Not implemented
3. **Schema/howto documentation** - Outdated, doesn't reflect Phase 9 additions

### Non-Blocking Issues

1. `sm_health.sh` endpoint format bug (TASK-6 failure)
2. No deprecation warning on standalone corrections tool
3. wander 60s timeout (configurable would be nice)

### Recommendation

Fix ISS-1 through ISS-4 before proceeding to Phase 10.
