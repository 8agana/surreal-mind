---
id: task-34
title: Fix call_cancel to actually stop running delegate jobs
status: In Progress
assignee: []
created_date: '2026-01-08 03:11'
labels:
  - surreal-mind
  - bug
  - tools
  - cancellation
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Ensure call_cancel terminates running agent jobs (especially delegate_gemini) and updates job status to cancelled promptly. Verify worker handles cancellation correctly and releases any locks/resources.
<!-- SECTION:DESCRIPTION:END -->

## Progress

- call_cancel updated to be idempotent and to set duration_ms=0 on cancel (src/tools/cancel_agent_job.rs).
- delegate_gemini worker checks cancellation and aborts active tasks (src/tools/delegate_gemini.rs).
- Needs validation once call_status is stable (current deserialization bug blocks confirmation).

## Next Steps

- Run a long-running call_gem job and call_cancel; verify job flips to cancelled and worker stops.
- Ensure cancelled jobs do not produce output post-cancel and do not block queue.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 call_cancel changes job status to cancelled within seconds
- [ ] #2 Cancelled jobs stop executing and do not produce output afterward
- [ ] #3 A cancelled job does not block subsequent queued jobs
- [ ] #4 Cancellation is idempotent and safe to call multiple times
<!-- AC:END -->
