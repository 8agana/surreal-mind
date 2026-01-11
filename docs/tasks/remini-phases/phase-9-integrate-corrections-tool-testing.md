# Phase 9: Integrate Corrections Tool - Testing

**Status:** PENDING
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

### Run 1: [DATE] ([TESTER])

| Test ID | Result | Notes |
|---------|--------|-------|
| HP-1 | | |
| HP-2 | | |
| HP-3 | | |
| HP-4 | | |
| TASK-1 | | |
| TASK-2 | | |
| TASK-3 | | |
| TASK-4 | | |
| TASK-5 | | |
| TASK-6 | | |
| TASK-7 | | |
| MULTI-1 | | |
| MULTI-2 | | |
| MULTI-3 | | |
| RPT-1 | | |
| RPT-2 | | |
| RPT-3 | | |
| ERR-1 | | |
| ERR-2 | | |
| ERR-3 | | |
| ERR-4 | | |
| ERR-5 | | |
| INT-1 | | |
| INT-2 | | |
| INT-3 | | |
| DEP-1 | | |
| DEP-2 | | |
| DEP-3 | | |

---

## Issues Found

| Issue | Severity | Description | Resolution |
|-------|----------|-------------|------------|

---

## Verdict

**Status:** PENDING
**Ready for Phase 10:** [ ] Yes  [ ] No
