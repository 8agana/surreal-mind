---
id: doc-7
title: Codex review - timeout logic implementation
type: other
created_date: '2026-01-01 00:04'
updated_date: '2026-01-01 02:28'
---
# Codex review - timeout logic implementation

**Linked task:** task-2 (Fix timeout logic - activity-based vs duration-based)
**Status:** IMPLEMENTED (2025-12-31)

## Technical assessment

- The current timeout measures wall-clock duration and can terminate a healthy long-running Gemini CLI call that is still producing output, which creates false timeouts and leaves the child process running in the background.
- PersistedAgent and delegate_gemini treat "timeout" as total runtime rather than "time without progress," so slow batch work (kg_populate) is penalized despite active streaming.
- Observed symptom: process is still working after timeout, which implies the supervisor is exiting without properly draining/terminating the child.

## Implementation approach recommendations

- Replace duration-based timeout with inactivity-based timeout: track last_activity timestamp and reset it on any stdout/stderr output from the child process.
- Keep API backward compatible by interpreting existing timeout_ms as inactivity timeout (document the semantic shift). Optionally add max_runtime_ms as a separate hard cap.
- In PersistedAgent and delegate_gemini, unify the timeout logic so the same inactivity timer is applied in the outer wrapper and not double-applied.
- Ensure the read loop consumes output in a non-blocking / async manner and updates last_activity on partial chunks (do not wait for newline).
- On inactivity timeout: attempt graceful termination (SIGTERM), then SIGKILL after a short grace period; also terminate the process group to prevent orphaned Gemini CLI subprocesses.
- Log timeout reason clearly (inactivity vs max runtime) with last_activity age and bytes seen since last reset.

## Potential challenges / edge cases

- Gemini CLI may buffer output; if output is line-buffered or suppressed, inactivity timeout could still trigger during valid work. Mitigation: increase default inactivity threshold or add optional heartbeat flag.
- If both PersistedAgent and delegate_gemini enforce inactivity timeouts, the shorter one may still kill the process unexpectedly; avoid double timers or make one authoritative.
- Output can arrive in bursts; ensure timer resets on any stderr noise or stdout progress messages.
- Make sure timeout does not race with normal process exit; handle cleanup without reporting false timeout errors.
- If running on Windows in future, signal semantics differ; use platform-appropriate termination logic.

## Testing strategy

- Unit tests with a fake child process that prints a byte every N seconds (below inactivity threshold) and then sleeps longer to trigger timeout; verify reset/timeout behavior.
- Test that continuous streaming prevents timeout even beyond previous duration limit (e.g., >120s).
- Test silent-but-active simulation: child process sleeps and then outputs after threshold to ensure timeout triggers and cleanup occurs.
- Integration test: kg_populate batch with 25+ thoughts; verify no premature timeouts and no orphaned Gemini CLI processes.
- Regression test for backward compatibility: existing delegate_gemini calls still respect timeout_ms (now inactivity).

---

## Implementation Summary (Completed)

### What Was Done

1. **ActivityTracker struct** - Tracks `last_activity` timestamp, `bytes_since_reset`, and `inactivity_threshold`
2. **Async select loop** - `tokio::select!` monitors stdout, stderr, process exit, and inactivity concurrently
3. **Partial chunk detection** - Any bytes reset the timer, not just complete lines
4. **Single authoritative timer** - GeminiClient owns timeout; worker wrapper removed outer timeout
5. **Default threshold increased** - 60s -> 120s to accommodate slower operations

### Files Changed

- `src/clients/gemini.rs` - Complete rewrite of timeout logic
- `src/tools/delegate_gemini.rs` - Removed redundant outer timeout wrapper

### Edge Cases Handled

- stderr counts as activity (Gemini may output progress there)
- Process exit drains remaining output before returning
- kill_on_drop handles cleanup if timeout triggers
