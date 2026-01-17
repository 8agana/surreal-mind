# call_cc Implementation Plan

## 1) Overview
`call_cc` is an MCP tool that delegates a prompt to the Claude Code CLI (CC) from within surreal-mind. It provides a first-class, scripted way to run CC with predictable flags (non-interactive, permission-skipping, model selection, optional session resume) without hand-assembling shell commands. This brings CC into the federation on the same footing as `call_gem`, enabling repeatable automation and persistent CC sessions (via `--resume <uuid>`).

## 2) Parameter Design
Follow the codex-cli pattern (session + cwd + timeout) with CC-specific model selection.

**Parameters (schema-level):**
- `prompt` (string, **required**) — Task to delegate to CC.
- `model` (string, optional, default: `"haiku"`, enum: `haiku | sonnet | opus`) — Maps to ANTHROPIC_MODEL values.
- `session_id` (string, optional) — UUID for `--resume <uuid>`.
- `cwd` (string, optional) — Working directory for the CC CLI subprocess.
- `timeout_ms` (number, optional) — Hard timeout for the CC process.

**Validation rules:**
- `prompt` must be non-empty after trim.
- `model` must be one of `haiku|sonnet|opus` (case-insensitive, normalized to lowercase).
- `session_id` must be a valid UUID v4 format (use `uuid::Uuid::parse_str`); return invalid params before spawning.
- `cwd` (if provided) must exist and be a directory (validate with `std::fs::metadata`).
- `timeout_ms` (if provided) must be > 0; use a sane minimum (e.g., 1_000 ms) to avoid zero/negative timeouts.

**Model mapping (env var):**
- `haiku`  → `claude-haiku-4-5`
- `sonnet` → `claude-sonnet-4-5`
- `opus`   → `claude-opus-4-5`

## 3) CLI Invocation Pattern
Construct the CC command exactly and deterministically:

```
ANTHROPIC_MODEL=<mapped> \
claude -p --dangerously-skip-permissions [--resume <session_id>] <prompt>
```

**Details:**
- Always set `ANTHROPIC_MODEL` env var.
- Always pass `-p` for print mode.
- Always pass `--dangerously-skip-permissions` (no interactive prompts).
- Only include `--resume <session_id>` when `session_id` is provided.
- Use `Command::new("claude")` with args, not a shell.
- Set `current_dir` when `cwd` is provided.

## 4) Response Format
Return a structured JSON payload (via `CallToolResult::structured`) so callers can reliably parse results:

```json
{
  "status": "completed" | "failed" | "timeout",
  "output": "<stdout as string>",
  "stderr": "<stderr as string, optional>",
  "exit_code": 0,
  "model": "haiku|sonnet|opus",
  "session_id": "<uuid if provided>",
  "duration_ms": 1234
}
```

**Notes:**
- `stderr` should be included when non-empty; helpful for diagnosing CC errors.
- On non-zero exit, set `status = "failed"` and include `stderr` + exit code.
- On timeout, kill the process and return `status = "timeout"` with partial `stdout`/`stderr` if available.
- If CC writes directly to stdout only (common), `output` becomes the authoritative response.

## 5) File-Based Async Option (Future)
If we later want async execution without polling tokens, use the file notification pattern from the README:

```
/tmp/jobs/
├── <job_id>.status   # JSON status: running/completed/failed + timestamps
├── <job_id>.output   # append-only stdout (tail -f)
└── <job_id>.error    # append-only stderr (optional)
```

**Future tool shape:**
- Add `fire_and_forget: bool` or `async: bool` parameter.
- If async, return immediately with `{ job_id, status_file, output_file }`.
- A watcher (fswatch / tail -f) can trigger reads and avoid polling.

## 6) Implementation Steps
1. **Schema definition**
   - Add `call_cc_schema()` to `src/schemas.rs` with the parameters above.
   - Add `call_cc` to the `howto` metadata list (optional but recommended).
2. **Handler function**
   - Create `src/tools/call_cc.rs` with `handle_call_cc` on `SurrealMindServer`.
   - Parse/validate params, normalize model, validate UUID + cwd.
3. **CLI spawning logic**
   - Build `tokio::process::Command` for `claude`.
   - Set `ANTHROPIC_MODEL`, `-p`, `--dangerously-skip-permissions`, and optional `--resume`.
   - Capture stdout/stderr; set `kill_on_drop(true)`.
4. **Response parsing**
   - Collect stdout/stderr to strings.
   - Compute duration, set `status` based on exit code/timeout.
   - Return structured JSON payload (see section 4).
5. **Error handling**
   - Spawn errors: return a clear `Mcp` error (e.g., `claude` not found on PATH).
   - Invalid params: return `InvalidParams` with actionable message.
   - Timeout: kill process, return `status = "timeout"`.

## 7) Files to Create/Modify
- `src/tools/call_cc.rs` (new) — tool handler implementation.
- `src/schemas.rs` — add `call_cc_schema()`.
- `src/server/router.rs` — register tool in `list_tools` and route `call_cc` in `call_tool`.

## 8) Testing Plan
**Unit validation tests** (fast, no CC dependency):
- Parameter validation: empty prompt, invalid model, invalid UUID, invalid cwd, zero timeout.

**Manual smoke tests** (requires CC installed):
1. `call_cc` with `model=haiku`, short prompt → verify `status=completed`, non-empty `output`.
2. `call_cc` with `model=sonnet` and `cwd` pointing to repo → verify tool can read/write in that dir.
3. `call_cc` with `session_id=<valid uuid>` → verify it passes `--resume` (check CC output consistency).
4. Timeout test: set `timeout_ms` very low (e.g., 50 ms) → verify `status=timeout` and process is killed.

**Optional integration check**
- Add a tiny `#[tokio::test]` behind an env guard (`SM_CC_CLI=1`) that runs a 1-line prompt and asserts `exit_code == 0`.
