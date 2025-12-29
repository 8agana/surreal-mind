---
date: 2025-12-28
issue type: Research of previously established code
justification: Looking to see what can be borrowed from `terminal-mcp`
status: Completed
implementation date: 2025-12-29
original prompt:
  - docs/prompts/20251227-gemini-cli-implementation.md
related_docs:
  - docs/prompts/20251221-memories_populate-implementation.md
---

# Gemini CLI: Implementation research


## Findings

### 1) Result-nesting hazard (gemini.rs)
- Location: `src/clients/gemini.rs:106-128`.
- `stdout_task`/`stderr_task` are `JoinHandle<Result<Vec<u8>, io::Error>>`.
- Awaiting them yields `Result<Result<Vec<u8>, io::Error>, JoinError>`.
- If only one `map_err`/`?` is used, you end up with a nested Result (compile error or mismatched error propagation).
- Current code already flattens with two `map_err(...)?` calls; this appears to be the intended fix.

### 2) terminal-mcp reusable patterns
- Config flags: `TERMINAL_MCP_TRANSPORT`, `TERMINAL_MCP_HTTP_BIND`, `TERMINAL_MCP_HTTP_PATH`,
  `TERMINAL_MCP_ALLOW_TOKEN_IN_URL`, `TERMINAL_MCP_DEFAULT_CWD`.
- Token handling: reads `TERMINAL_MCP_TOKEN` or persists `~/.terminal_mcp_token` (auto-generate UUID if missing).
- Auth middleware: bearer token header; optional `access_token`/`token` query param if allowed; `/health` bypass.
- HTTP vs stdio: `Transport` enum; `streamable_http_server` with `LocalSessionManager` and SSE keepalive; stdio uses rmcp stdio.
- No DB sync pattern present in terminal-mcp (no SurrealDB or persistence).

## Notes / Next steps
- If gemini.rs was recently edited, re-run a build to ensure no Result nesting regression.
- If DB sync is required for Gemini, terminal-mcp does not provide a pattern; look to SurrealMind clients or other MCPs.
