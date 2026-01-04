---
id: doc-2
title: Implementation Steps - delegate_cc tool
type: other
created_date: '2026-01-03 21:43'
updated_date: '2026-01-03 21:43'
---
# Implementation Steps - delegate_cc tool

Linked task: `backlog/tasks/task-8 - Build-delegate_cc-tool-in-surreal-mind.md`

## Current Gemini Baseline (for continuity)
- `src/tools/delegate_gemini.rs` normalizes `cwd`, stores it on the job record, and passes it down to `GeminiClient` via `with_cwd(...)` in the worker.
- `src/clients/gemini.rs` applies `cmd.current_dir(dir)` before spawning, so cwd is already wired end-to-end. This satisfies the prerequisite.
- `delegate_gemini` is async-only today (always queues a job). The input schema + output schema advertise sync support and `fire_and_forget`, but the handler does not implement sync paths.
- `PersistedAgent::call()` injects DB context and persists exchanges, but it does NOT pass the `session_id` to the underlying agent call. If we want Claude Code `-c` / `-r` behavior to actually resume CLI sessions, we must pass a resume flag through the persisted layer or bypass it for resume flows.
- `delegate_gemini` worker claims any queued job without filtering `tool_name`. Adding a second worker will require filtering by tool_name to avoid cross-claiming.

## Goal
Add a `delegate_cc` tool that mirrors the delegate_gemini architecture while supporting Claude Code CLI specifics: `-p`, `--output-format json`, `-c` (continue recent), `-r <session_id>`, `--max-turns`, and `cwd` via `Command::current_dir()`.

## Implementation Steps

### 1) Add `src/clients/cc.rs`
Create a new `CcClient` implementing `CognitiveAgent`. Keep it simple (non-streaming JSON output).

Recommended structure (match Gemini conventions):
- `struct CcClient { timeout: Duration, cwd: Option<PathBuf>, continue_recent: bool, max_turns: Option<u32> }`
- `impl CcClient::new()` reads `CC_TIMEOUT_MS` (default 60_000) and optionally `CC_MAX_TURNS`.
- `with_timeout_ms(timeout_ms: u64)` setter (mirrors `GeminiClient::with_timeout_ms`).
- `with_cwd(cwd)` setter.
- `with_continue_recent(bool)` setter.
- `with_max_turns(u32)` setter.

`call(prompt, session_id)` behavior:
- Build `Command::new("claude")`.
- Set env similar to Gemini: `CI=true`, `TERM=dumb`, `NO_COLOR=1`.
- Add `--output-format json` and `-p <prompt>`.
- If `continue_recent == true` and no `session_id`, add `-c`.
- If `session_id` is Some, add `-r <session_id>` (and do NOT add `-c`).
- If `max_turns` set, add `--max-turns <N>`.
- Apply `cmd.current_dir(dir)` when `cwd` is set.
- Use `tokio::time::timeout` around `wait_with_output()` to enforce inactivity timeout. (Claude Code CLI output is not streaming JSON; a total timeout is sufficient.)

Parsing:
- Define a small `CcResponse` struct:
  - `session_id: String`
  - `response: String`
  - (optional) `usage`, `model`, etc if you want to retain metadata later.
- Try `serde_json::from_str` on stdout; if it fails, fall back to the Gemini `extract_json_candidates` helper pattern to find the last JSON object in stdout.
- Strip ANSI codes before parsing (reuse `strip_ansi_codes`).
- Return `AgentResponse { session_id, response, exchange_id: None, stream_events: None }`.

Error handling:
- If `claude` not found: map to `AgentError::NotFound`.
- On non-zero exit: include stderr tail in `AgentError::CliError`.
- On timeout: return `AgentError::Timeout { timeout_ms }`.

### 2) Add `src/tools/delegate_cc.rs`
Copy the delegate_gemini pattern but adjust for Claude Code CLI semantics.

Parameters to support:
- `prompt: String` (required)
- `task_name: Option<String>` (default "delegate_cc")
- `cwd: Option<String>`
- `timeout_ms: Option<u64>` (default from `CC_TIMEOUT_MS` or 60_000)
- `max_turns: Option<u32>`
- `continue_recent: Option<bool>` (maps to `-c`)
- `resume_session_id: Option<String>` (maps to `-r`)
- `fire_and_forget: Option<bool>` (default false)

Sync vs async:
- If `fire_and_forget == true`: create an `agent_jobs` record and return `{ status: "queued", job_id, message }`.
- Else: execute directly via `execute_cc_call(...)` and return `{ response, session_id, exchange_id }`.

Important continuity decisions:
- **Session continuity**: if you want `-c` / `-r` to work, you must pass a session flag through the persisted layer.
  - Option A (recommended): update `PersistedAgent::call` to pass `session_id` into `self.agent.call(prompt_to_send, session_id)`.
  - Option B: when `continue_recent` or `resume_session_id` is set, bypass `PersistedAgent` and call `CcClient` directly, then manually persist the exchange (duplicate of `PersistedAgent` logic). This avoids changing Gemini behavior but adds code duplication.
- If you change `PersistedAgent`, verify delegate_gemini still behaves as expected.

Job metadata:
- The `agent_jobs` table is schemafull; it currently does NOT define `tool_timeout_ms`, `expose_stream`, `max_turns`, or `resume_session_id`. If you want to store these fields directly, add them to `src/server/schema.rs`. Otherwise, store CC-specific fields in `metadata` (preferred to avoid schema churn).
- Keep `agent_source = "claude"` and `agent_instance = "claude_code"` (or similar constant) for consistency. There is no model override in Claude Code CLI today.

### 3) Add a CC worker and fix job claiming
Create `run_delegate_cc_worker` mirroring the Gemini worker, but ensure **only CC jobs are claimed**.

Update `claim_next_job` logic:
- Add `WHERE tool_name = $tool_name` for each worker.
- Example for Gemini: `... WHERE status = 'queued' AND tool_name = 'delegate_gemini' ...`.
- Example for CC: `... WHERE status = 'queued' AND tool_name = 'delegate_cc' ...`.

This is required because the current Gemini worker claims any queued job, which would cause CC jobs to be executed by the wrong client once the CC worker exists.

### 4) Wire worker spawn in server init
In `src/server/db.rs`, add a second `tokio::spawn` for `run_delegate_cc_worker` next to the Gemini worker when transport is `http`.

### 5) Register the new tool
Update the following files:
- `src/tools/mod.rs`: `pub mod delegate_cc;`
- `src/schemas.rs`: add `delegate_cc_schema()` and output schema if you support sync response (recommended).
- `src/server/router.rs`: add `delegate_cc` in `list_tools` and `call_tool` routing.
- `src/tools/detailed_help.rs`: add a roster entry and full help text.
- `src/main.rs`: update the startup "Loaded N MCP tools" log to include `delegate_cc`.

### 6) Add tests (lightweight)
- Unit test parsing of CC JSON output (success + malformed output). Use the same parsing helpers as Gemini.
- Optional: smoke test for `-c`/`-r` flag selection based on inputs (without executing the CLI).

## Suggested CC API Contract
Minimal payload for sync call:
```
{
  "prompt": "string",
  "task_name": "delegate_cc",
  "cwd": "/path",
  "continue_recent": false,
  "resume_session_id": null,
  "max_turns": 6,
  "timeout_ms": 60000
}
```

Minimal payload for async call:
```
{
  "prompt": "string",
  "fire_and_forget": true,
  "cwd": "/path"
}
```

## Open Questions / Decisions
- Should `delegate_cc` default to async-only like `delegate_gemini`, or should it implement true sync behavior to match the output schema?
- Do we want to modify `PersistedAgent` to pass `session_id` (enabling CLI resume), knowing it also changes Gemini behavior?
- Where to store CC-specific fields (`max_turns`, `continue_recent`, `resume_session_id`) in `agent_jobs`: new schema fields vs `metadata` object.
- Should `resume_session_id` take precedence over `continue_recent` (recommended: yes).

## Summary
The Gemini side already sets cwd via `Command::current_dir`, so the prerequisite is satisfied. The new `delegate_cc` should mirror Gemini’s end-to-end flow (tool handler → persisted agent → client → job system) while adding CC-specific flags (`-c`, `-r`, `--max-turns`). The main continuity risks are (1) the persisted layer currently drops session IDs, and (2) the job worker currently claims queued jobs without filtering `tool_name`.
