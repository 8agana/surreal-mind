# Future Considerations for call_* Tools

This document captures ideas and enhancements to consider after the core call_cc, call_codex, call_gem, and call_vibe tools are working with basic functionality.

## Phase 1 Complete Criteria

Before tackling these, ensure basic functionality works:
- Model selection per tool
- Working directory specification
- Prompt execution with JSON output
- Job tracking via call_status/call_jobs/call_cancel
- Basic error handling (protocol vs execution errors)

---

## Process Management

### Zombie Reaper
Kill orphan processes on cancel/crash. call_cancel needs to terminate the actual process tree (PID group), not just mark the job cancelled in SurrealDB.

### Adaptive Timeouts
Dynamic timeouts based on prompt token count/complexity instead of static 60s. Heavy reasoning tasks need more time; simple queries should fail fast.

---

## Safety & Verification

### Capability Handshake
On startup or first call, check if CLI is installed (`which claude`, `command -v codex`), fingerprint version, and gracefully degrade or disable unavailable tools.

### Dry Run Pre-Flight
Before execution, run lightweight connectivity/permission checks. Distinguish "agent failed to solve problem" from "agent couldn't start" in error reporting.

### Git Safety Snapshots
Standardize Codex's "Ghost Snapshots" pattern across all agents - create temporary git commit before write operations, enable automatic rollback.

### Diff-Driven Verification
Run `git diff --stat` after execution to verify what actually changed vs what agent claimed. Ground truth verification.

### Cost Circuit Breaker
Track token usage per job/session. Kill process and reject future calls if cost exceeds threshold. Prevents runaway autonomous loops.

---

## Security

### JIT Credential Scoping
Don't pass all env vars to subprocess. Accept a `scope` parameter, inject only necessary API keys. Principle of least privilege.

---

## Multi-Agent Coordination

### File System Locking
Prevent race conditions when multiple agents target same file. Use flock or SurrealDB key for lightweight file-level locking.

### Handoff Token Protocol
Define text pattern (e.g., `<<DELEGATE: AGENT_NAME>>`) that triggers automatic agent-to-agent delegation. Enables swarm behavior.

### Context Hydration
Accept `previous_job_id` parameter to inject artifacts/summary from previous job into new session. Synthetic long-term memory for ephemeral CLIs.

### Shadow Mode Consensus
Spin up multiple agents in parallel with same read-only prompt, arbitrate between outputs before executing changes. High-cost, high-reliability mode.

---

## UX Improvements

### Semantic Output Multiplexing
Split stdout into raw log file + semantic event stream. Detect and broadcast high-level events ("Plan Generated", "Compiling", "Error") to call_status in real-time.

### PTY Masquerade
Wrap CLIs in pseudo-terminal to capture rich status updates/progress bars, strip ANSI, convert to structured progress events.

---

## Brainstorming as Inner Voice

**Sam's idea:** The gemini-cli brainstorm tool is powerful. Consider building this into SurrealMind as a new `inner_voice` tool or similar - a way for CC to "think out loud" with another model before committing to an approach.

Potential design:
- Lightweight call to fast model (Gemini Flash, Haiku)
- Structured divergent thinking with idea generation
- Could replace or augment current inner_voice implementation
- Question: synchronous (blocking) or async (background pondering)?

---

## Notes

- Document created: 2026-01-16, Session 5
- Source: Gemini brainstorm on implementation gaps
- Priority: Build incrementally - core functionality first, then layer these enhancements
