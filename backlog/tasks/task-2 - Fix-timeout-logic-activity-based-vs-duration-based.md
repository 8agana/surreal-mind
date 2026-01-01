---
id: task-2
title: Fix timeout logic - activity-based vs duration-based
status: To Do
assignee: []
created_date: '2026-01-01 00:02'
labels:
  - timeout
  - delegate_gemini
  - PersistedAgent
  - bug
  - enhancement
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Current issue: Duration-based timeout kills active work that's legitimately slow, not hung.

Affects: delegate_gemini, PersistedAgent wrapper, any long-running Gemini calls

Root cause: Timeout measures "time elapsed" not "time without progress"
- Current behavior: kills process after fixed time (e.g., 120s)
- Problem: kills ACTIVE work that's just slow
- Gemini continues running in background after "timeout"
- Designed for quick interactive calls, not batch processing

Impact: kg_populate and other batch operations fail prematurely at 120s even when Gemini is actively processing.

Real-world evidence: kg_populate batch extraction confirmed Gemini still running in background after timeout.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Timeout detects hung processes (no output for X seconds)
- [ ] #2 Timeout does NOT kill active processes that are outputting/streaming
- [ ] #3 Configurable inactivity threshold (default 120s of silence)
- [ ] #4 Works with PersistedAgent wrapper and Gemini CLI integration
- [ ] #5 Backward compatible with existing delegate_gemini calls
- [ ] #6 Tested with kg_populate batch extraction (25+ thoughts)
<!-- AC:END -->
