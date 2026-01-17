# call_codex Tool

MCP tool for delegating tasks to Codex (ChatGPT's Coding CLI) from within surreal-mind.

Status: Planned

## Goal
Expose a first-class `call_codex` MCP tool that queues and executes Codex CLI jobs with controlled flags, structured output, and session resume support. This will be the template for other delegation tools.

## MVP Scope
- Synchronous surface (MCP call returns job_id) with background worker execution.
- Clean JSON output capture (`--json`, `--color never`) and stderr separation.
- CWD required; prompt required.
- Optional: model override (default `gpt-5.2-codex`), resume/continue, timeouts, expose_stream flag.
- Persist job state in SurrealDB and surface via existing `call_status`/`call_jobs`/`call_cancel`.

## Open Questions (answered)
- Default model: `gpt-5.2-codex` with `model_reasoning_effort=medium` (implicit).
- Resume semantics: support `resume_session_id` (`codex resume <id>`) and `continue_latest` (`codex resume --last`).
- Headless output: enforce `--json --color never`; stdout should be NDJSON events, stderr for logs.
- Auth expiry: codex auto-retries 401/503 but may device-code prompt; enforce outer timeout to avoid hangs.

## Implementation Plan

### 1) API & Schema
- Add `call_codex_schema` in `src/schemas.rs` with fields:
  - `prompt` (string, required)
  - `task_name` (string, default "call_codex")
  - `model` (string, default "gpt-5.2-codex")
  - `cwd` (string, required, non-empty)
  - `resume_session_id` (string, optional)
  - `continue_latest` (boolean, default false)
  - `timeout_ms` (number, default 60_000)
  - `tool_timeout_ms` (number, default 300_000)  // maps to env for CLI if supported
  - `expose_stream` (boolean, default false)
  - `fire_and_forget` (boolean, default false)

### 2) Router Registration
- Register tool in `src/server/router.rs` and `main.rs` tool list as `call_codex`.

### 3) Client Wrapper (`src/clients/codex.rs`)
- Build `tokio::process::Command`:
  - binary: `codex`
  - subcommand: `exec`
  - flags: `--json`, `--color`, `never`, `--model <model>`, `--cd <cwd>`, `--full-auto`
  - resume: if `resume_session_id` -> prepend `resume <id>` before `exec`; else if `continue_latest` -> `resume --last` before `exec`.
  - prompt: pass as positional at end.
- Apply environment overrides per call (optional `TOOL_TIMEOUT_SEC` from `tool_timeout_ms`).
- Capture stdout (NDJSON) and stderr separately; return `AgentResponse { session_id, response, stream_events? }`. If parsing session_id is non-trivial, return placeholder and store raw output.
- Map non-zero exit to AgentError::CliError with exit code + stderr.

### 4) Tool Handler (`src/tools/call_codex.rs`)
- Define params struct mirroring schema.
- Validate `cwd` and `prompt` non-empty.
- Default `task_name` -> "call_codex".
- Generate job_id; insert `agent_jobs` row with `tool_name="call_codex"`, `agent_instance="codex"`, prompt, model, cwd, resume flags, timeouts, expose_stream.
- Return `{status:"queued", job_id}`.

### 5) Worker (`run_call_codex_worker`)
- Poll queued jobs with `tool_name="call_codex"`.
- Build CodexClient with per-job settings; outer timeout using `timeout_ms` via `tokio::time::timeout`.
- On success: mark completed, store `session_id`, `exchange_id` (if derivable), `duration_ms`; if `expose_stream`, persist parsed events in metadata.
- On failure: mark failed with exit code/stderr summary; if cancelled, mark cancelled.

### 6) Error & Edge Handling
- Classify exit code 1 + stderr containing auth/rate-limit hints; still mark failed but include hint string.
- Enforce stdout/stderr separation; reject if stdout is empty and stderr is not (treat as failure).
- Handle cancellation via job registry (abort task).

### 7) Defaults & Constants
- `DEFAULT_MODEL`: "gpt-5.2-codex"
- `DEFAULT_TIMEOUT_MS`: 60_000
- `DEFAULT_TOOL_TIMEOUT_MS`: 300_000

### 8) Tests
- `tests/tool_schemas.rs`: schema includes `call_codex`.
- Unit: constant assertions (timeouts).
- Integration (future): spawn worker with mock command? (optional, can defer).

### 9) Future Enhancements
- Sandbox/autonomy flag mapping (`--sandbox read-only` vs `--full-auto`).
- Cost guardrail: pre-set `tool_output_token_limit` env or enforce outer byte limit.
- Stream event parsing into structured JSON for status polling.

### Implementation Notes (2026-01-17)
- Implemented `call_codex` schema, router registration, and tool handler with async job queue.
- Added `CodexClient` wrapper and worker integration with job metadata + stream event capture.
- Updated agent job schema for tool timeouts, resume flags, and stream metadata.
