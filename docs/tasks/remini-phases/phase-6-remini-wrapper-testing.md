# Phase 6: REMini Wrapper - Testing

**Status:** PENDING
**Parent:** [phase-6-remini-wrapper.md](phase-6-remini-wrapper.md)
**Depends On:** Phase 6 Implementation Complete, Phase 5 Working

---

## Goal

Verify the `remini` unified maintenance daemon correctly orchestrates all background KG operations.

---

## Pre-requisites

- Phase 5 gem_rethink working
- kg_populate working
- kg_embed working
- remini binary built
- launchd plist created (for scheduling tests)

---

## Test Cases

### Happy Path

| ID | Test | Command | Expected Result |
|----|------|---------|-----------------|
| HP-1 | Run all tasks | `remini --all` | All tasks execute in order |
| HP-2 | Run specific tasks | `remini --tasks populate,embed` | Only specified tasks run |
| HP-3 | Dry run mode | `remini --dry-run` | Preview without changes |
| HP-4 | View last report | `remini --report` | Displays last sleep report |
| HP-5 | Single task | `remini --tasks rethink` | Only rethink runs |

### Task Orchestration

| ID | Task | Expected Behavior |
|----|------|-------------------|
| TASK-1 | populate | kg_populate runs, extracts from new thoughts |
| TASK-2 | embed | kg_embed runs, generates missing embeddings |
| TASK-3 | rethink | gem_rethink runs, processes mark queue |
| TASK-4 | wander | Optional exploration for new connections |
| TASK-5 | health | Health check runs, finds issues |

### Sleep Report Format

| ID | Test | Expected Fields |
|----|------|-----------------|
| RPT-1 | Report structure | `run_timestamp`, `tasks_run`, `summary`, `task_details`, `duration_seconds`, `next_scheduled` |
| RPT-2 | Summary fields | `thoughts_processed`, `entities_created`, `embeddings_generated`, `corrections_made`, `health_issues_found` |
| RPT-3 | Task details | Per-task breakdown with individual metrics |

### Scheduling (launchd)

| ID | Test | Expected Result |
|----|------|-----------------|
| SCHED-1 | Plist loads | `launchctl load` succeeds |
| SCHED-2 | Manual trigger | `launchctl start dev.legacymind.remini` runs |
| SCHED-3 | Log output | Logs written to configured paths |
| SCHED-4 | Scheduled run | Runs at 3:00 AM (verify next day) |

### Error Handling

| ID | Test | Expected Result |
|----|------|-----------------|
| ERR-1 | Task failure | Failed task logged, other tasks continue |
| ERR-2 | Invalid task name | `remini --tasks invalid` errors gracefully |
| ERR-3 | Database down | Graceful error with clear message |
| ERR-4 | Partial completion | Report shows which tasks succeeded/failed |

### Edge Cases

| ID | Test | Expected Result |
|----|------|-----------------|
| EDGE-1 | No work to do | All tasks complete with 0 items processed |
| EDGE-2 | Very long run | Doesn't timeout, reports duration |
| EDGE-3 | Concurrent run attempt | Second instance blocked or handled |

---

## Test Results

### Run 1: [DATE] ([TESTER])

| Test ID | Result | Notes |
|---------|--------|-------|
| HP-1 | | |
| HP-2 | | |
| HP-3 | | |
| HP-4 | | |
| HP-5 | | |
| TASK-1 | | |
| TASK-2 | | |
| TASK-3 | | |
| TASK-4 | | |
| TASK-5 | | |
| RPT-1 | | |
| RPT-2 | | |
| RPT-3 | | |
| SCHED-1 | | |
| SCHED-2 | | |
| SCHED-3 | | |
| SCHED-4 | | |
| ERR-1 | | |
| ERR-2 | | |
| ERR-3 | | |
| ERR-4 | | |
| EDGE-1 | | |
| EDGE-2 | | |
| EDGE-3 | | |

---

## Issues Found

| Issue | Severity | Description | Resolution |
|-------|----------|-------------|------------|

---

## Verdict

**Status:** PENDING
**Ready for Phase 7:** [ ] Yes  [ ] No
