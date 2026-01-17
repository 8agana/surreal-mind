# call_cc Tool

MCP tool for delegating tasks to Claude Code CLI from within surreal-mind.

Status: Planned (implementation plan below).

## Overview
Build `call_cc` by mirroring the existing `call_codex` flow: a thin CLI client wrapper plus a synchronous tool handler that validates inputs, runs the CLI, and returns structured results. The Claude Code CLI differs in invocation shape (no subcommand, `-p` prompt), output format (stream-json), and session continuation flags (`--resume` or `-c`). Model selection is via the `ANTHROPIC_MODEL` environment variable instead of a `--model` flag in the command line (use the env var pattern).

## CLI Mapping

| Concern | Codex (existing) | Claude Code (new) | Notes |
| --- | --- | --- | --- |
| Binary | `codex` | `claude` | |
| Non-interactive | `exec <prompt>` | `-p <prompt>` | `claude` has no `exec` subcommand |
| JSON output | `--json` (NDJSON) | `--output-format stream-json` or `--json` | stream-json is the preferred event stream |
| Permissions | `--full-auto` | `--dangerously-skip-permissions` | bypass prompts |
| Model | `--model <name>` | `ANTHROPIC_MODEL=<name>` | env var only |
| Resume | `resume <id>` / `resume --last` | `--resume <id>` / `-c` | `-c` continues last session |
| CWD | `--cd <path>` | `cd <path>` (use `Command::current_dir`) | no `--cd` flag |

## Implementation Plan

### Files to Create
1. `src/clients/claude.rs`
   - Create `ClaudeClient` mirroring `CodexClient` in `src/clients/codex.rs`.
   - Fields: `model`, `cwd`, `resume_session_id`, `continue_latest`, `tool_timeout_ms`, `expose_stream`.
   - Build `Command::new("claude")` with:
     - `-p <prompt>`
     - `--dangerously-skip-permissions`
     - `--output-format stream-json` (or `--json` if required by upstream)
     - `--resume <uuid>` when `resume_session_id` provided; else `-c` when `continue_latest=true`.
     - `current_dir(cwd)` if provided.
   - Set `ANTHROPIC_MODEL` env var when `model` is specified (default in tool handler).
   - Capture stdout/stderr; parse stream-json for session id + response text (similar to `parse_codex_ndjson`, but adapted to CC event schema).
   - Map CLI errors to `AgentError` with helpful hints (auth/rate-limit if stderr includes known tokens).

2. `src/tools/call_cc.rs`
   - Create tool handler matching the shape of `call_codex` in `src/tools/call_codex.rs`.
   - Params: `prompt`, `task_name`, `model`, `cwd`, `resume_session_id`, `continue_latest`, `timeout_ms`, `tool_timeout_ms`, `expose_stream`, `fire_and_forget`.
   - Enforce non-empty `prompt` and `cwd` (required).
   - Disallow `resume_session_id` + `continue_latest` combination.
   - Use `ClaudeClient` to execute with `tokio::time::timeout`.
   - Return structured result: `{ status, response, session_id, metadata }` (mirror call_codex).

### Files to Modify
1. `src/clients/mod.rs`
   - Export `claude` module.

2. `src/tools/mod.rs`
   - Export `call_cc` module.

3. Router registration (same pattern as `call_codex`)
   - Add handler wiring in the tool router where other call_* tools are registered.

## Testing Plan
1. Unit tests:
   - `src/tools/call_cc.rs`: assert defaults (model, timeout).
   - Validate `resume_session_id` vs `continue_latest` guard.
2. Schema/tool registration tests (if present):
   - Ensure `call_cc` appears in any schema or tool listing test.
3. Manual sanity (optional):
   - Run a small prompt in a safe repo to verify `stream-json` parsing and session id extraction.

## Reference Files
- `src/clients/codex.rs` (client pattern)
- `src/tools/call_codex.rs` (handler pattern)
- `docs/tasks/20260115-call_tools/notebooklm-research.md` (CLI details)
