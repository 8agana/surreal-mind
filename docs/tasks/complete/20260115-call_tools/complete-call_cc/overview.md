# call_cc Tool

**Status:** Planning
**Date:** 2026-01-15
**Investigator:** Codex

## Overview

MCP tool for delegating tasks to Claude Code instances from within surreal-mind.

## Rationale

- MCP tool calls are natural for CC; bash command construction is friction
- Models `call_gem` pattern (already working)
- Enables clean delegation without bash orchestration overhead
- Part of federation tooling: call_cc, call_codex, call_vibe

## Sam's Requirements

1. **--resume <session-id> support (CRITICAL)** - Resume specific sessions by UUID, not just "last session." This enables persistent domain-specific CC instances (SM-CC, PM-CC) that accumulate context over time.
   - `--continue` = resume last session (implicit)
   - `--resume <uuid>` = resume specific session (explicit) ← **THIS IS THE KEY**
   - Error from CLI: "Session IDs must be in UUID format (e.g., 550e8400-e29b-41d4-a716-446655440000)"

2. **--dangerously-skip-permissions** - No human in the loop for delegated calls. Permission prompts would block automation.

3. **Model selection** - Must support haiku/sonnet/opus via ANTHROPIC_MODEL env var. Start cheap, escalate as needed.

4. **-p flag** - Print mode for non-interactive use.

## CLI Pattern (from federation skill)

```bash
# Existing aliases show the pattern:
cch  = ANTHROPIC_MODEL=claude-haiku-4-5 claude -p --dangerously-skip-permissions
cchc = ANTHROPIC_MODEL=claude-haiku-4-5 claude -p --dangerously-skip-permissions --continue
ccs  = ANTHROPIC_MODEL=claude-sonnet-4-5 claude -p --dangerously-skip-permissions
ccsc = ANTHROPIC_MODEL=claude-sonnet-4-5 claude -p --dangerously-skip-permissions --continue
cco  = ANTHROPIC_MODEL=claude-opus-4-5 claude -p --dangerously-skip-permissions
ccoc = ANTHROPIC_MODEL=claude-opus-4-5 claude -p --dangerously-skip-permissions --continue
```

**Key insight:** Start with Haiku, escalate to Sonnet if needed, Opus for hard problems.

## Design Questions

- [x] How to handle session continuity? → `--resume <uuid>` for specific sessions
- [ ] Working directory specification? → Pass via `--cwd` or in prompt context?
- [ ] How to return results back to caller?
- [ ] Timeout handling?

### Fire-and-Forget: Probably Not

**Problem:** Fire-and-forget without notification = deferred polling. You trade waiting for checking, still burn tokens either way.

**Decision:** Start with synchronous only. Add async later if we solve notification (webhook? file watch? MCP push?). Don't build complexity we'll hate using.

### Future: Async with Streaming

**Idea for later:** Instead of polling for job status (burns tokens), could async tools provide streaming feedback?

**Concept:**
- Tool call returns immediately with `{ stream_id, status: "running" }`
- Client subscribes to parallel channel (SSE, WebSocket, or file tail)
- Live output streams as it happens
- Completion notification when done

**Why this matters:**
- Solves the fire-and-forget polling problem
- No wasted tokens checking status
- Real-time visibility into delegated work

**Implementation options:**
1. SSE (Server-Sent Events) endpoint
2. WebSocket connection
3. File-based streaming (tail -f pattern)

**Status:** Research/future - needs protocol-level investigation

### File-Based Notification Pattern (Preferred Approach)

**Insight:** Use the filesystem as the notification channel. No WebSockets, no SSE, no infrastructure. Just files.

**Pattern:**
1. Dispatch async task → returns immediately with `{ job_id, status_file: "/tmp/jobs/{job_id}.status" }`
2. Delegated task writes to status file as it progresses
3. Coordinator runs `fswatch /tmp/jobs/` (via ht-mcp or background shell)
4. File change triggers read → get result without polling
5. No token burn, no repeated API calls

**For streaming output:**
- Task appends lines to status file
- Coordinator runs `tail -f` on the file
- Live updates as they happen

**Implementation sketch:**
```
/tmp/jobs/
├── abc123.status      # JSON: { status: "running", progress: "50%" }
├── abc123.output      # Streaming output (append-only)
├── def456.status      # JSON: { status: "completed", result: "..." }
└── def456.output
```

**Status file format:**
```json
{
  "job_id": "abc123",
  "status": "running|completed|failed|cancelled",
  "started_at": "2026-01-15T17:30:00Z",
  "progress": "optional progress info",
  "result": "final result when completed",
  "error": "error message if failed"
}
```

**Why this is elegant:**
- Uses existing primitives (files, fswatch, tail)
- No new infrastructure to build or maintain
- Works across all CLI tools (CC, Codex, Gemini, Vibe)
- Coordinator can watch multiple jobs with single fswatch
- Easy to debug (just cat the files)
- Survives restarts (state is on disk)

### Context Header for Delegated Calls

**Idea:** Prepend a short context header to delegated prompts so the called agent knows where it's being called from.

Example:
```
[SurrealMind Delegation] You are being called from the SurrealMind MCP by CC. 
Task: {actual prompt here}
```

**Benefits:**
- Called agent has immediate context about the caller
- Can adjust behavior based on source
- Helps with debugging/tracing
- Sets expectations for response format

**Open question:** What information should the header include?
- Caller identity (SurrealMind, CC, etc.)
- Session context?
- Expected response format?

## Proposed Interface

```rust
// TBD - Codex investigating
```

## Parameters (Draft)

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| prompt | string | yes | Task to delegate |
| cwd | string | no | Working directory |
| timeout_ms | number | no | Timeout in milliseconds |
| fire_and_forget | bool | no | Run async without waiting |
| session_id | string | no | Continue existing session? |

## Implementation Notes

*Pending Codex investigation*

## Reference

- `call_gem` implementation: see surreal-mind/src/tools/call_gem.rs
- Brain file delegation skill: ~/.claude/skills/delegation/SKILL.md
- **steipete/claude-code-mcp**: https://github.com/steipete/claude-code-mcp
  - Existing MCP server wrapping Claude Code CLI
  - Uses single `claude_code` tool with prompt + tools array
  - `--dangerously-skip-permissions` for non-interactive execution
  - One-shot executor model
  - Does NOT have --continue support (our differentiator)

## Open Questions

1. How does call_gem handle context passing? Model that.
2. What's the CLI invocation pattern for CC?
3. How to parse/return structured results?
