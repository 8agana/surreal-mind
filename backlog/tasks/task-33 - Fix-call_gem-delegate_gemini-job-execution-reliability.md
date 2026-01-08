---
id: task-33
title: Fix call_gem (delegate_gemini) job execution reliability
status: In Progress
assignee: []
created_date: '2026-01-08 03:11'
labels:
  - surreal-mind
  - bug
  - tools
  - delegate_gemini
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Make delegate_gemini jobs start, run, and complete reliably when invoked via call_gem. Investigate stuck queued/running jobs, worker lifecycle, and any queue/lock contention. Ensure results are returned and jobs transition to terminal states.
<!-- SECTION:DESCRIPTION:END -->

## Progress

- call_gem enqueue/worker path exercised repeatedly; jobs complete in DB but call_status tool deserialization fails (serialization error from SurrealDB Value/enum handling).
- delegate_gemini worker updates: cancellation checks, idempotent cancel, and exchange_id write path touched (see src/tools/delegate_gemini.rs, src/tools/cancel_agent_job.rs, src/server/schema.rs).
- call_status implementation is the current blocker for verifying end-to-end completion payloads.

## Next Steps

- Fix call_status deserialization reliably (agent_job_status.rs); likely needs explicit SQL casting or safe conversion of SurrealDB types to JSON.
- Re-test via one-shot CC (Haiku) after HTTP restart to confirm call_gem -> call_status works with live binary.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 call_gem jobs transition from queued -> running -> completed/failed within expected timeouts
- [ ] #2 Stuck jobs can be detected and do not block new jobs from starting
- [ ] #3 delegate_gemini worker logs include start/end and error details for each job
- [ ] #4 A fresh call_gem invocation returns a completed result payload without manual intervention
<!-- AC:END -->
