# call_gem Implementation Analysis

## Summary
`call_gem` is the MCP-facing tool name that routes to the `delegate_gemini` handler and enqueues Gemini CLI work as an async background job. It stores job metadata in SurrealDB, runs a worker loop to execute jobs with bounded concurrency, and records session continuity via `tool_sessions` for future resumptions. In short: it is the async delegation and persistence layer for Gemini inside surreal-mind.

## Architecture
- **Tool surface**: `call_gem` is registered in `src/server/router.rs` and its schema is defined in `src/schemas.rs`. The router maps `call_gem` to `SurrealMindServer::handle_delegate_gemini` in `src/tools/delegate_gemini.rs`.
- **Request handling**: `handle_delegate_gemini` validates input, normalizes fields, selects model/timeouts, creates an `agent_jobs` row (status `queued`), and returns a job ID immediately.
- **Async job pattern (enqueue + worker + status polling)**:
  - Enqueue: `create_job_record` writes `agent_jobs` with `status=queued` and job metadata (prompt, task_name, model_override, cwd, timeouts, expose_stream).
  - Worker: `run_delegate_gemini_worker` polls for queued jobs, claims the next job, marks it `running`, acquires a semaphore permit, and spawns an async task that calls Gemini via `PersistedAgent`.
  - Status polling: `call_status` (`src/tools/agent_job_status.rs`) reads the job row; `call_jobs` (`src/tools/list_agent_jobs.rs`) lists jobs; `call_cancel` updates status and attempts registry abort.
- **Session/job tracking via `agent_jobs` and `tool_sessions` tables**:
  - `agent_jobs` stores lifecycle state, timings, error, prompt/task metadata, and IDs (`session_id`, `exchange_id`).
  - `PersistedAgent` writes `agent_exchanges` and upserts `tool_sessions` via `upsert_tool_session` in `src/utils/db.rs`, so the next call can resume from `last_agent_session_id`.

## What's Good
- Clear separation of concerns: schema → handler → job record → worker loop.
- Bounded concurrency via a semaphore (`SURR_JOB_CONCURRENCY`), preventing runaway parallelism.
- Persistent continuity via `tool_sessions` and `agent_exchanges`, enabling resume semantics.
- Robust Gemini CLI integration (stream-json parsing, activity-based timeout, stderr capture, tool-timeout tracking).
- Async job APIs (`call_status`, `call_jobs`, `call_cancel`) provide standard operational visibility.
- Defensive job lifecycle handling: completion, failure, cancellation updates with timestamps and duration tracking.

## Issues Found

### Critical
- **Registry dummy handle issue (can't actually cancel)**
  - The registry stores a dummy `JoinHandle` (a pending task), not the real Gemini execution handle. `call_cancel` tries to abort via the registry, but it only aborts the dummy task. Actual cancellation only happens later via polling in the worker loop.

### High
- **Jobs marked running before semaphore acquired**
  - `claim_next_job` updates status to `running` before acquiring the semaphore. If concurrency is saturated, jobs will appear “running” even though they haven’t started. This breaks observability and can mislead cancel expectations.

### Medium
- **Unused `_tool_timeout`**
  - `handle_delegate_gemini` computes `_tool_timeout` but doesn’t use it. The computed default is never persisted, so the job record lacks an explicit `tool_timeout_ms` unless the caller provided one.
- **Regex-based error recovery**
  - The worker attempts to recover from invalid prompt records by parsing error strings and extracting record IDs via regex. This is brittle and dependent on SurrealDB error formatting.
- **`expose_stream` no-op**
  - The flag toggles event capture inside `GeminiClient`, but those stream events are never returned to the caller or stored in job metadata. For the async pattern, the client has no way to receive the stream.

### Low
- **Parameter naming inconsistencies**
  - Tool schema uses `model`, while jobs store `model_override`. The job’s `tool_name` is recorded as `delegate_gemini`, not `call_gem`, so filtering by `call_gem` won’t match. These mismatches increase cognitive load and make querying less intuitive.

## Recommendations
- **Fix cancellation**: register a real cancel handle.
  - Option A: store a `tokio::task::AbortHandle` in the registry (or a `CancellationToken`) and connect it to the actual Gemini task.
  - Option B: wrap execution in a task whose `JoinHandle<()>` you can store directly (e.g., spawn a task that forwards the result through a channel).
  - Update `call_cancel` to signal the token and mark job cancelled only after confirmation.
- **Correct job state transitions**:
  - Acquire semaphore *before* marking job `running`. Consider a “claimed” state or move the `UPDATE ... SET status = 'running'` into the worker after acquiring a permit.
- **Resolve `_tool_timeout` usage**:
  - Either remove the unused variable or persist the computed default into `agent_jobs` so the stored record reflects actual runtime behavior.
- **Replace regex error recovery**:
  - Validate prompt at creation (already done) and avoid letting invalid rows exist.
  - If cleanup is needed, run a targeted query for invalid records instead of parsing error strings.
- **Make `expose_stream` meaningful**:
  - Either remove it from `call_gem` (if async-only), or persist stream events in `agent_jobs.metadata` and surface them via `call_status`.
  - For real-time streaming, consider a synchronous tool path or MCP notifications.
- **Align naming and filters**:
  - Accept both `model` and `model_override`, or rename schema to `model_override` and document it.
  - Store `tool_name = 'call_gem'` in `agent_jobs`, and keep `agent_instance = 'gemini'` as the runtime identity.

## Comparison to MCP Patterns
- Typical MCP tools are synchronous request/response with direct results. `call_gem` uses a job queue + polling model to handle long-running work, which is reasonable for CLI subprocesses but diverges from standard MCP expectations.
- MCP streaming is usually handled via progress/notifications or direct response streaming, not an out-of-band DB queue. The current design is functional but adds operational overhead and requires explicit polling tools (`call_status`, `call_jobs`, `call_cancel`).
- The persistence layer (`agent_exchanges` + `tool_sessions`) is stronger than typical MCP patterns, which often ignore continuity. This is a major advantage, but only if job state and cancellation are accurate.

## Takeaways for call_cc
- **Use real cancellation handles**: design `call_cc` so registry aborts the *actual* task, not a dummy placeholder.
- **Status must reflect reality**: mark `running` only after resources are acquired and execution begins.
- **Keep schema and storage aligned**: parameter names should match stored fields and filtering semantics.
- **Avoid brittle recovery**: don’t parse error strings for control flow; fix the upstream cause or query directly.
- **Decide on streaming strategy early**: either support it fully (persist + expose) or remove it from the API to avoid false promises.
- **Leverage continuity**: keep `tool_sessions` and `agent_exchanges` integration, but ensure `call_cc` writes consistent metadata and session IDs.
