---
date: 2025-12-31
issue type: No tools showing in Claude Code MCP client
justification: After async delegate_gemini change, CC client showed 0 tools
status: Resolved (client compatibility patch applied)
implementation date: 2025-12-31
original prompt:
  - docs/prompts/20251230-delegate_gemini-async.md
---

# Troubleshooting: delegate_gemini async "no tools" report

## Summary
- Root cause: Claude Code MCP client likely fails to parse newer JSON Schema (`oneOf` / unions) in tool `outputSchema`.
- Compatibility patch: removed `outputSchema` for `delegate_gemini` and async job tools from the MCP tool list.
- After rebuild + restart, tools/list returns 12 tools and CC should show tools again.

## Evidence (server works)
Validated locally against the running HTTP MCP endpoint using the bearer token.

1) Initialize (get session id):
```bash
TOKEN=$(cat ~/.surr_token)
curl -s -D - -o /dev/null \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -X POST http://127.0.0.1:8787/mcp \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"curl","title":"curl","version":"0"}}}'
```
Response includes `mcp-session-id` header.

2) Notify initialized:
```bash
curl -s \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -H "mcp-session-id: <SESSION_ID>" \
  -X POST http://127.0.0.1:8787/mcp \
  -d '{"jsonrpc":"2.0","method":"notifications/initialized"}'
```

3) Tools list:
```bash
curl -s \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -H "mcp-session-id: <SESSION_ID>" \
  -X POST http://127.0.0.1:8787/mcp \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
```
Result: **12 tools** returned.

Tools returned:
- legacymind_think
- maintenance_ops
- memories_create
- detailed_help
- delegate_gemini
- curiosity_add
- curiosity_get
- curiosity_search
- legacymind_search
- agent_job_status
- list_agent_jobs
- cancel_agent_job

## Finding
The server was healthy and returning tools, but Claude Code showed "Tools: None" after the async refactor. The most likely cause was CC's MCP client schema parser choking on newer JSON Schema constructs (notably `oneOf` in `delegate_gemini_output_schema` and union types in the new job tool output schemas). Claude Desktop continued to see tools, reinforcing a CC-only compatibility issue.

## Fix (compatibility patch)
Removed `outputSchema` from the MCP tool list entries for:
- `delegate_gemini`
- `agent_job_status`
- `list_agent_jobs`
- `cancel_agent_job`

Location: `src/server/router.rs`.

This keeps input schemas intact while avoiding CC's stricter/older schema parser. Tools remain fully functional; only output schema metadata is omitted for those tools.

## Commands run
```
cargo build --release
launchctl kickstart -k gui/$(id -u)/dev.legacymind.surreal-mind
```

## Compiler errors encountered
None. Build completed cleanly. (clippy/fmt not run in this incident.)

## Notes
- Claude Desktop already handled the original schemas; the issue appears specific to CC.
- Consider reintroducing output schemas once CC MCP client is updated to support `oneOf` and union types.

