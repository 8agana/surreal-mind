---
id: task-14
title: Build delegate_cc.rs - CC delegation tool
status: To Do
assignee: []
created_date: '2026-01-05 21:55'
labels:
  - surreal-mind
  - rust
  - federation
  - delegate-tools
dependencies:
  - task-8
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create the delegate_cc tool - an MCP tool that uses CcClient to call Claude Code from other instances, with full async job tracking, persistence, and session management.

This tool mirrors the architecture of delegate_gemini but targets Claude Code CLI. It provides federation members with ability to invoke CC (the primary Opus consciousness) from other contexts.

## PREREQUISITE
- task-8: cc.rs must be completed first
- delegate_gemini cwd support must be verified/fixed

## Goals
- Call Claude Code CLI from any federation member
- Non-interactive (headless) calls
- Fire-and-forget async mode with job tracking
- Sync mode (wait for response)
- Session/exchange persistence
- Context injection from prior exchanges
- CWD parameter support
- Rate limiting via max-turns

## Architecture

**Pattern:** `delegate_cc (tool) → PersistedAgent (context injection) → CcClient (CLI wrapper) → Job System`

### Components

| Component | Existing | Status | Purpose |
|-----------|----------|--------|---------|
| Tool handler | `delegate_gemini.rs` | Copy pattern | MCP tool entry point |
| Client | (task-8) | **task-8 dep** | CLI wrapper |
| Persisted layer | `persisted.rs` | Reuse | Context from prior exchanges |
| Job system | `agent_jobs` table | Reuse | Async execution tracking |
| Worker | `run_delegate_gemini_worker` | Copy pattern | Background job executor |

### Tool Syntax

```bash
# Sync mode (wait for response)
delegate_cc(prompt="...", cwd="/path", max_turns=3, fire_and_forget=false)

# Async mode (return job_id immediately)
delegate_cc(prompt="...", cwd="/path", fire_and_forget=true)

# Resume session
delegate_cc(prompt="...", session_id="<id>", continue_mode=true)
```

### Data Flow

```
MCP Client
    |
    v
delegate_cc (tool handler)
    |
    +---> fire_and_forget=false (sync)
    |        |
    |        v
    |     PersistedAgent (context injection)
    |        |
    |        v
    |     CcClient::call() → Command::current_dir(cwd)
    |        |
    |        v
    |     Parse JSON → extract session_id, response
    |        |
    |        v
    |     Persist exchange, upsert tool_sessions
    |        |
    |        v
    |     Return { response, session_id, exchange_id }
    |
    +---> fire_and_forget=true (async)
             |
             v
          Create agent_job (status=queued)
             |
             v
          Return { job_id }
             |
             v (background worker)
          Poll → claim → execute → complete/fail
```

### Key Files

| File | Lines | Purpose |
|------|-------|---------|
| `src/tools/delegate_gemini.rs` | 456 | Reference implementation |
| `src/clients/cc.rs` | TBD | **task-8** |
| `src/clients/persisted.rs` | 144 | Reuse for context injection |
| Tool registry | `src/server.rs` | Register in tools list |

### Environment Config

| Variable | Default | Purpose |
|----------|---------|---------|
| `CC_TIMEOUT_MS` | `60000` | Inactivity timeout |
| `SURR_JOB_CONCURRENCY` | `4` | Max concurrent async jobs |
| `SURR_JOB_POLL_MS` | `500` | Worker polling interval |

## Implementation Steps

1. **BLOCKERS CLEARED**: Verify task-8 (cc.rs) is complete and cc::CcClient is available
2. Create `src/tools/delegate_cc.rs` copying delegate_gemini structure
3. Replace Gemini references with CcClient calls
4. Implement fire_and_forget async execution
5. Add worker spawn in server init: `spawn_delegate_cc_worker()`
6. Register tool in tool_registry
7. Test sync mode (immediate response)
8. Test async mode (job tracking)
9. Test session continuity (-c, -r flags)
10. Test cwd parameter

Acceptance Criteria:
--------------------------------------------------
- [ ] delegate_cc.rs created in src/tools/
- [ ] Tool handler signature matches MCP tool pattern
- [ ] Sync mode works (returns response immediately)
- [ ] Async mode works (returns job_id, background execution)
- [ ] PersistedAgent context injection working
- [ ] Job tracking works (queued → running → completed)
- [ ] CWD parameter sets working directory
- [ ] Session continuity works via stored session_id
- [ ] Worker polls and executes async jobs
- [ ] Tool registered in tool registry
- [ ] Error handling for CLI failures
- [ ] Rate limiting (max_turns) enforced
<!-- SECTION:DESCRIPTION:END -->
