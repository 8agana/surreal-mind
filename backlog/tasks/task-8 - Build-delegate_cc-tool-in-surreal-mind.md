---
id: task-8
title: Build delegate_cc tool in surreal-mind
status: To Do
assignee: []
created_date: '2026-01-02 23:14'
updated_date: '2026-01-02 23:19'
labels:
  - surreal-mind
  - rust
  - federation
  - delegate-tools
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create a delegate_cc tool mirroring delegate_gemini architecture but for Claude Code CLI.

**PREREQUISITE: Fix delegate_gemini cwd support first. Verify it's actually being passed to Command::current_dir().**

---

## Goals
- Non-interactive calls (headless execution)
- --continue option for conversation resumption
- Ability to set cwd (working directory)
- Fire-and-forget async mode
- Job status tracking (queued → running → completed/failed)
- Session/exchange persistence

---

## Architecture (from delegate_gemini analysis)

**Pattern:** `Tool Handler → PersistedAgent (context injection) → CognitiveAgent (CLI wrapper) → Job System`

### Components to Create

| Component | Existing (Gemini) | New (CC) |
|-----------|-------------------|----------|
| Tool handler | `delegate_gemini.rs` | `delegate_cc.rs` (copy + modify) |
| Client | `gemini.rs` | `cc.rs` (new) |
| Persisted layer | `persisted.rs` | **Reuse as-is** |
| Job system | `agent_jobs` table | **Reuse as-is** |
| Worker | `run_delegate_gemini_worker` | `run_delegate_cc_worker` (copy pattern) |

### CLI Syntax Comparison

```bash
# Gemini
gemini -y [-m model] -e "" --output-format json [--resume session] "prompt"

# Claude Code
claude -p "prompt" --output-format json [-c | -r session_id] [--max-turns N]
```

### Claude Code CLI Reference (from NBLM research)

1. **Non-interactive:** `claude -p "prompt"` or pipe: `cat file | claude -p "explain"`
2. **Continue recent:** `claude -c -p "follow up"` (most recent in current dir)
3. **Resume specific:** `claude -r <session_id> -p "follow up"`
4. **CWD:** No --cwd flag. Must set working directory on process spawn via `Command::current_dir()`
5. **JSON output:** `--output-format json` (returns session_id, response, cost metadata)
6. **Rate limiting:** `--max-turns N` to prevent runaway loops

### Key Files

- Tool definition: `src/tools/delegate_gemini.rs` (456 lines)
- Gemini client: `src/clients/gemini.rs` (385 lines)
- Persisted agent: `src/clients/persisted.rs` (144 lines)
- Job management: within delegate_gemini.rs

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
    |     PersistedAgent (context injection from prior exchanges)
    |        |
    |        v
    |     CcClient::call() → spawn "claude -p" with Command::current_dir(cwd)
    |        |
    |        v
    |     parse JSON → extract session_id, response
    |        |
    |        v
    |     persist exchange, upsert tool_sessions
    |        |
    |        v
    |     return { response, session_id, exchange_id }
    |
    +---> fire_and_forget=true (async)
             |
             v
          create agent_jobs (status=queued) → return { job_id }
             |
             v (background worker)
          poll → claim → execute → complete/fail
```

### Environment Config

| Variable | Default | Purpose |
|----------|---------|---------|
| `CC_TIMEOUT_MS` | `60000` | Inactivity timeout |
| `SURR_JOB_CONCURRENCY` | `4` | Max concurrent async jobs |
| `SURR_JOB_POLL_MS` | `500` | Worker polling interval |

---

## Implementation Steps

1. **Verify/fix cwd in delegate_gemini** - confirm Command::current_dir() is being used
2. Create `src/clients/cc.rs` - CcClient implementing CognitiveAgent trait
3. Create `src/tools/delegate_cc.rs` - tool handler mirroring delegate_gemini
4. Add worker spawn in server init (alongside gemini worker)
5. Register tool in tool registry
6. Test sync and async modes
7. Test cwd parameter
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 delegate_gemini cwd parameter verified/fixed
- [ ] #2 CcClient created implementing CognitiveAgent trait
- [ ] #3 delegate_cc tool handler created
- [ ] #4 Worker spawned at server init
- [ ] #5 Tool registered in registry
- [ ] #6 Sync mode works: claude -p with JSON output
- [ ] #7 Async mode works: fire_and_forget with job tracking
- [ ] #8 cwd parameter sets working directory correctly
- [ ] #9 Session continuity works via -c and -r flags
<!-- AC:END -->
