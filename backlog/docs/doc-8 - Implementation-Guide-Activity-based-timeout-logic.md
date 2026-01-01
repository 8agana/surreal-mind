---
id: doc-8
title: Implementation Guide - Activity-based timeout logic
type: other
created_date: '2026-01-01 00:06'
---
# Implementation Guide: Activity-based Timeout Logic

**Linked Task:** task-2 (Fix timeout logic - activity-based vs duration-based)
**Author:** CC (synthesized from task-2 + Codex review doc-7)
**Date:** 2025-12-31

---

## Problem Summary

### Current Behavior
The delegate_gemini and PersistedAgent timeout logic measures **wall-clock duration** from process start. After a fixed time (e.g., 120s), the process is killed regardless of whether work is actively progressing.

### Why This Fails
1. **Kills active work** - Legitimate slow operations (kg_populate batch extraction) are terminated mid-progress
2. **Orphaned processes** - Gemini CLI continues running in background after timeout because the supervisor exits without proper cleanup
3. **Wrong mental model** - Designed for quick interactive calls, not batch processing that may run 5+ minutes while continuously streaming output

### Real-World Evidence
- kg_populate batch extraction with 25+ thoughts: Gemini confirmed still running after timeout triggered
- Process was actively working, just slow - not hung

---

## Solution Overview

Replace duration-based timeout with **inactivity-based timeout**:

1. Track `last_activity` timestamp
2. Reset timestamp on ANY stdout/stderr output (partial chunks, not just newlines)
3. Timeout triggers only after N seconds of **silence**
4. Proper process termination (SIGTERM → SIGKILL) with process group cleanup

**Key principle:** A process outputting data is working. Only kill processes that go silent.

---

## Implementation Steps

### Step 1: Define Data Structures

```rust
struct ActivityTracker {
    last_activity: Instant,
    bytes_since_reset: usize,
    inactivity_threshold: Duration,
    max_runtime: Option<Duration>,  // Optional hard cap
    start_time: Instant,
}

impl ActivityTracker {
    fn reset(&mut self, bytes: usize) {
        self.last_activity = Instant::now();
        self.bytes_since_reset += bytes;
    }
    
    fn is_inactive(&self) -> bool {
        self.last_activity.elapsed() > self.inactivity_threshold
    }
    
    fn exceeded_max_runtime(&self) -> bool {
        self.max_runtime.map_or(false, |max| self.start_time.elapsed() > max)
    }
}
```

### Step 2: Modify Async Stream Reading (delegate_gemini)

Location: `src/tools/delegate_gemini.rs` (or equivalent)

```rust
// Replace blocking read with async non-blocking loop
loop {
    tokio::select! {
        // Check for output (partial chunks, not line-buffered)
        result = stdout.read(&mut buffer) => {
            match result {
                Ok(0) => break,  // EOF
                Ok(n) => {
                    tracker.reset(n);
                    output.extend_from_slice(&buffer[..n]);
                }
                Err(e) => return Err(e.into()),
            }
        }
        
        // Check for stderr
        result = stderr.read(&mut err_buffer) => {
            if let Ok(n) = result {
                if n > 0 {
                    tracker.reset(n);  // Stderr counts as activity
                }
            }
        }
        
        // Inactivity check (runs periodically)
        _ = tokio::time::sleep(Duration::from_secs(1)) => {
            if tracker.is_inactive() {
                log::warn!(
                    "Inactivity timeout: {}s since last output, {} bytes seen",
                    tracker.inactivity_threshold.as_secs(),
                    tracker.bytes_since_reset
                );
                terminate_process_group(child.id());
                return Err(TimeoutError::Inactivity);
            }
            if tracker.exceeded_max_runtime() {
                log::warn!("Max runtime exceeded");
                terminate_process_group(child.id());
                return Err(TimeoutError::MaxRuntime);
            }
        }
    }
}
```

### Step 3: Process Termination Logic

```rust
fn terminate_process_group(pid: u32) {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    
    let pgid = Pid::from_raw(-(pid as i32));  // Negative = process group
    
    // Step 1: SIGTERM (graceful)
    let _ = kill(pgid, Signal::SIGTERM);
    
    // Step 2: Wait grace period
    std::thread::sleep(Duration::from_secs(2));
    
    // Step 3: SIGKILL (force) if still running
    let _ = kill(pgid, Signal::SIGKILL);
}
```

### Step 4: Update PersistedAgent Wrapper

Location: `src/tools/persisted_agent.rs` (or equivalent)

**Critical:** Avoid double timers. Either:
- PersistedAgent is authoritative for timeout (preferred)
- Or delegate_gemini handles it and PersistedAgent just monitors

Recommended: PersistedAgent owns the ActivityTracker, passes activity signals up from the underlying delegate_gemini call.

### Step 5: Update Gemini CLI Spawn

Ensure Gemini CLI is spawned in its own process group for clean termination:

```rust
use std::process::Command;
use std::os::unix::process::CommandExt;

let mut cmd = Command::new("gemini");
cmd.args(&["-p", &prompt, "-y", "-m", &model]);
cmd.process_group(0);  // Create new process group
```

---

## Backward Compatibility

### API Changes
| Parameter | Old Meaning | New Meaning |
|-----------|-------------|-------------|
| `timeout_ms` | Max total runtime | Max inactivity (silence) duration |

### Migration Path
1. **Document semantic shift** - Update tool descriptions, docstrings
2. **Existing calls work** - 120000ms (120s) of silence is generous for most use cases
3. **Optional:** Add `max_runtime_ms` parameter for hard caps if needed

### Breaking Changes
- Calls that previously timed out (killing active work) will now complete
- This is the desired behavior change

---

## Edge Cases & Mitigations

| Edge Case | Risk | Mitigation |
|-----------|------|------------|
| **Buffered output** | Gemini CLI may buffer, causing false inactivity | Increase default threshold (120s is generous); consider unbuffered stderr |
| **Double timers** | Both PersistedAgent and delegate_gemini enforce timeout | Make PersistedAgent authoritative; delegate_gemini has no independent timeout |
| **Burst output** | Output arrives in bursts with gaps | Timer resets on any byte, not just complete messages |
| **Race with exit** | Timeout races normal process exit | Check exit status before reporting timeout error |
| **Windows future** | SIGTERM/SIGKILL not available | Use `TerminateProcess` on Windows (not current priority) |

---

## Testing Checklist

### Unit Tests
- [ ] Fake child prints byte every 60s (below 120s threshold) - should NOT timeout
- [ ] Fake child sleeps 130s with no output - should timeout at 120s
- [ ] Timer resets on stderr output (not just stdout)
- [ ] Continuous streaming for 300s - no timeout if output keeps flowing
- [ ] Silent-then-output: child sleeps 130s then outputs - timeout triggers before output

### Integration Tests
- [ ] **kg_populate batch** - 25+ thoughts extraction completes without timeout
- [ ] No orphaned Gemini CLI processes after timeout (check `ps aux | grep gemini`)
- [ ] Backward compatibility: existing delegate_gemini calls respect timeout_ms as inactivity

### Manual Verification
- [ ] Run `delegate_gemini` with a long prompt, observe no premature timeout
- [ ] Force hang (infinite loop prompt), observe timeout triggers and cleanup occurs

---

## Success Criteria

From task-2 acceptance criteria:

1. **Timeout detects hung processes** - No output for X seconds triggers timeout
2. **Timeout does NOT kill active processes** - Output/streaming resets timer
3. **Configurable inactivity threshold** - Default 120s, adjustable via timeout_ms
4. **Works with PersistedAgent wrapper** - Single authoritative timer, no double enforcement
5. **Backward compatible** - Existing calls work (semantic shift documented)
6. **Tested with kg_populate** - 25+ thought batch completes without premature timeout

**Definition of Done:** kg_populate batch extraction runs to completion regardless of duration, and orphaned processes are properly terminated on actual timeout.
