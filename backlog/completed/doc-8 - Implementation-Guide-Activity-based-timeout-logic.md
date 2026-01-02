---
id: doc-8
title: Implementation Guide - Activity-based timeout logic
type: other
created_date: '2026-01-01 00:06'
updated_date: '2026-01-01 02:29'
---
# Implementation Guide: Activity-based Timeout Logic

**Linked Task:** task-2 (Fix timeout logic - activity-based vs duration-based)
**Author:** CC (synthesized from task-2 + Codex review doc-7)
**Date:** 2025-12-31
**Status:** IMPLEMENTED

---

## Problem Summary

### Current Behavior (FIXED)
The delegate_gemini and PersistedAgent timeout logic measured **wall-clock duration** from process start. After a fixed time (e.g., 120s), the process was killed regardless of whether work was actively progressing.

### Why This Failed
1. **Killed active work** - Legitimate slow operations (kg_populate batch extraction) were terminated mid-progress
2. **Orphaned processes** - Gemini CLI continued running in background after timeout because the supervisor exited without proper cleanup
3. **Wrong mental model** - Designed for quick interactive calls, not batch processing that may run 5+ minutes while continuously streaming output

### Real-World Evidence
- kg_populate batch extraction with 25+ thoughts: Gemini confirmed still running after timeout triggered
- Process was actively working, just slow - not hung

---

## Solution Implemented

Replaced duration-based timeout with **inactivity-based timeout**:

1. Track `last_activity` timestamp via `ActivityTracker` struct
2. Reset timestamp on ANY stdout/stderr output (partial chunks, not just newlines)
3. Timeout triggers only after N seconds of **silence**
4. Relies on `kill_on_drop(true)` for process cleanup

**Key principle:** A process outputting data is working. Only kill processes that go silent.

---

## Implementation Details

### ActivityTracker Struct (src/clients/gemini.rs)

```rust
struct ActivityTracker {
    last_activity: Mutex<Instant>,
    bytes_since_reset: AtomicUsize,
    inactivity_threshold: Duration,
    start_time: Instant,
}

impl ActivityTracker {
    async fn reset(&self, bytes: usize) {
        let mut last = self.last_activity.lock().await;
        *last = Instant::now();
        self.bytes_since_reset.fetch_add(bytes, Ordering::Relaxed);
    }
    
    async fn is_inactive(&self) -> bool {
        let last = self.last_activity.lock().await;
        last.elapsed() > self.inactivity_threshold
    }
}
```

### Async Select Loop (GeminiClient::call)

```rust
loop {
    tokio::select! {
        // Check for stdout data
        result = stdout.read(&mut stdout_chunk) => {
            if let Ok(n) = result && n > 0 {
                tracker.reset(n).await;
                stdout_buf.extend_from_slice(&stdout_chunk[..n]);
            }
        }

        // Check for stderr data (also counts as activity)
        result = stderr.read(&mut stderr_chunk) => {
            if let Ok(n) = result && n > 0 {
                tracker.reset(n).await;
                // Cap stderr to prevent memory issues
            }
        }

        // Check process status
        result = child.wait() => {
            // Drain remaining output, check exit status
            break;
        }

        // Periodic inactivity check (every 1 second)
        _ = tokio::time::sleep(Duration::from_millis(1000)) => {
            if tracker.is_inactive().await {
                tracing::warn!(
                    "Inactivity timeout: {}s since last output",
                    tracker.inactivity_duration().await.as_secs()
                );
                let _ = child.kill().await;
                return Err(AgentError::Timeout { timeout_ms });
            }
        }
    }
}
```

### Worker Simplification (src/tools/delegate_gemini.rs)

Removed outer timeout wrapper - GeminiClient is now authoritative:

```rust
// Before (double timeout - BAD)
let result = tokio::time::timeout(
    Duration::from_millis(timeout),
    execute_gemini_call(...),
).await;

// After (single timeout - GOOD)
let result = execute_gemini_call(...).await;
```

---

## Backward Compatibility

### API Changes
| Parameter | Old Meaning | New Meaning |
|-----------|-------------|-------------|
| `timeout_ms` | Max total runtime | Max inactivity (silence) duration |

### Migration Path
1. **Document semantic shift** - Tool descriptions updated
2. **Existing calls work** - 120000ms (120s) of silence is generous for most use cases
3. **Default increased** - 60s -> 120s to accommodate slower operations

### Breaking Changes
- Calls that previously timed out (killing active work) will now complete
- This is the **desired behavior change**

---

## Edge Cases Handled

| Edge Case | How Handled |
|-----------|-------------|
| **Buffered output** | Default threshold increased to 120s; any partial chunk resets timer |
| **Double timers** | Worker outer timeout removed; GeminiClient is authoritative |
| **Burst output** | Timer resets on any byte, not just complete messages |
| **Race with exit** | Process exit branch drains output before returning |
| **stderr activity** | Counts as activity (Gemini may output progress there) |

---

## Testing Completed

- [x] Clean build with no warnings
- [x] Semantic change documented
- [ ] kg_populate batch test (pending manual verification)

---

## Success Criteria Status

From task-2 acceptance criteria:

1. **Timeout detects hung processes** - YES: No output for threshold seconds triggers timeout
2. **Timeout does NOT kill active processes** - YES: Output/streaming resets timer
3. **Configurable inactivity threshold** - YES: Default 120s, adjustable via timeout_ms
4. **Works with PersistedAgent wrapper** - YES: Single authoritative timer in GeminiClient
5. **Backward compatible** - YES: Existing calls work (semantic shift documented)
6. **Tested with kg_populate** - PENDING: Manual verification needed

**Definition of Done:** kg_populate batch extraction runs to completion regardless of duration, and orphaned processes are properly terminated on actual timeout.
