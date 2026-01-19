---
id: task-33
title: Fix call_gem (delegate_gemini) job execution reliability
status: Completed
assignee: [CC, Rusty]
created_date: '2026-01-08 03:11'
completed_date: '2026-01-08 14:45'
labels:
  - surreal-mind
  - bug
  - tools
  - delegate_gemini
  - agent_job_status
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Make delegate_gemini jobs start, run, and complete reliably when invoked via call_gem. Investigate stuck queued/running jobs, worker lifecycle, and any queue/lock contention. Ensure results are returned and jobs transition to terminal states.
<!-- SECTION:DESCRIPTION:END -->

## Resolution

**Problem:** call_status tool failed with deserialization errors when querying job status, blocking end-to-end validation of call_gem pipeline.

**Root Cause:** The `exchange_id` field in agent_jobs table is `option<record<agent_exchanges>>` (Record type). Direct deserialization into `Option<String>` failed, especially for running jobs where exchange_id = NONE.

**Fix (commit 1c87fe5):**
Modified `src/tools/agent_job_status.rs` SQL query to use SurrealDB conditional casting:
```sql
IF exchange_id != NONE THEN type::string(exchange_id) ELSE null END as exchange_id
```

This safely converts Record → string when present, null when NONE, allowing proper deserialization into `Option<String>`.

**Additional cleanup:**
- Removed phantom fields (tool_timeout_ms, expose_stream) that don't exist in schema
- Updated JobRow struct to match actual schema
- Added test coverage for running jobs with NONE values

**Validation:** Tested both scenarios successfully:
1. Immediate call_status on running job (NONE values) ✓
2. call_status on completed job (Record converted to string) ✓

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 call_gem jobs transition from queued -> running -> completed/failed within expected timeouts
- [x] #2 Stuck jobs can be detected and do not block new jobs from starting (no blocking observed)
- [x] #3 delegate_gemini worker logs include start/end and error details for each job
- [x] #4 A fresh call_gem invocation returns a completed result payload without manual intervention
<!-- AC:END -->
