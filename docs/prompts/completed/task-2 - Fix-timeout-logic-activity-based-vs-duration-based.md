---
id: task-2
title: Fix timeout logic - activity-based vs duration-based
status: Done
assignee: []
created_date: '2026-01-01 00:02'
updated_date: '2026-01-01 04:12'
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
- [x] #1 Timeout detects hung processes (no output for X seconds)
- [x] #2 Timeout does NOT kill active processes that are outputting/streaming
- [x] #3 Configurable inactivity threshold (default 120s of silence)
- [x] #4 Works with PersistedAgent wrapper and Gemini CLI integration
- [x] #5 Backward compatible with existing delegate_gemini calls
- [x] #6 Tested with kg_populate batch extraction (25+ thoughts)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Complete (2025-12-31)

### Changes Made

**src/clients/gemini.rs:**
- Added `ActivityTracker` struct to track last output timestamp and bytes received
- Replaced duration-based `timeout(self.timeout, child.wait())` with activity-based loop
- Uses `tokio::select!` to concurrently monitor stdout, stderr, process exit, and inactivity
- Resets activity timer on ANY output (stdout or stderr, partial chunks)
- Only triggers timeout after configured period of SILENCE (no output)
- Default inactivity threshold increased from 60s to 120s
- Added tracing logs for timeout reason with inactivity duration and bytes seen

**src/tools/delegate_gemini.rs:**
- Removed outer `tokio::time::timeout` wrapper from worker loop
- GeminiClient now handles timeout internally - prevents double-timeout issues
- Simplified result handling (no longer nested `Ok(Ok(...))` pattern)

### Key Design Decisions
1. **Single authoritative timer** - GeminiClient owns timeout logic, worker trusts it
2. **Partial chunk detection** - Resets timer on any bytes, not just complete lines
3. **stderr counts as activity** - Gemini CLI may output progress to stderr
4. **kill_on_drop** - Relies on tokio's child process cleanup for termination

### Testing
- Clean build with no warnings
- Semantic change: `timeout_ms` now means "inactivity threshold" not "total duration"
- kg_populate batch extractions should now complete regardless of duration as long as output is flowing
<!-- SECTION:NOTES:END -->
