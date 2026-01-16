# claude-code-mcp

## Summary
MCP server that wraps Claude Code CLI for non-interactive delegation. Executes prompts via `claude -p --dangerously-skip-permissions`.

**Repository:** https://github.com/steipete/claude-code-mcp
**Package:** @steipete/claude-code-mcp (npm)
**Author:** steipete

## Structure

Simple TypeScript MCP server with minimal footprint:
- Single tool: `claude_code`
- Two parameters only: `prompt` (required), `workFolder` (optional)
- Hardcodes `--dangerously-skip-permissions` flag
- Synchronous execution (blocks until complete)
- Returns Claude's output directly

## Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `prompt` | string | yes | Natural language instruction for Claude |
| `workFolder` | string | no | Working directory (absolute path). Required for file operations |

## What's Missing

- No model selection (haiku/sonnet/opus)
- No `--resume` or `--continue` support
- No timeout configuration
- No async/fire-and-forget mode
- No tool restrictions (uses defaults)

## Good Things to Take Away

1. **Minimal interface works** - Two parameters is enough for basic delegation
2. **Hardcoded permission bypass** - Right call for automation; no reason to expose this as a parameter
3. **workFolder pattern** - Clean way to specify execution context
4. **Synchronous by default** - Simpler than async until notification is solved
5. **Direct output return** - No wrapping, just return what Claude says
6. **Contextual error explanations** - When hooks or other processes fail, provides helpful context about why (e.g., explaining macOS lacks `timeout` command). Useful for debugging.

## Verdict

Good starting point. Thin wrapper, does one thing well. We need to add:
- Model selection (default haiku, optional sonnet/opus)
- `--resume <session-id>` for persistent sessions
- Timeout handling
- Possibly tool restrictions
