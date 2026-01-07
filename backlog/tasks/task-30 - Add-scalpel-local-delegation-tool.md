# Task 30: Add `scalpel` Tool for Local Model Delegation

**Status:** Proposed
**Owner:** Gemini (Antigravity)
**Context:** The user wants to offload routine, high-volume operations (file reads, minor code edits, summarization) to a local model (Ministral 8B) running via `mistral.rs`. This saves API costs and primary LLM context window.

## Architecture

We will implement a "Sidecar Pattern":
1. **Primary LLM (Gemini/Claude)**: Handles reasoning and decisions.
2. **Surreal-Mind (MCP)**: exposing a new tool `scalpel`.
3. **Ministral 8B (mistral.rs)**: Running locally as an OpenAI-compatible server.

```
[Gemini/Claude] -> [surreal-mind: scalpel()] -> [mistral.rs: Ministral 8B]
```

## Proposed Changes

### 1. New Dependency (External)
- `mistral.rs` running in server mode (`mistralrs-server --model mistral-8b-instruct`).
- Env vars for configuration:
  - `SURR_SCALPEL_ENABLED` (default false)
  - `SURR_SCALPEL_ENDPOINT` (default http://localhost:8080)

### 2. New Tool: `scalpel`
A general-purpose delegation tool.

**Schema:**
```json
{
  "name": "scalpel",
  "description": "Delegate routine operations to a fast local model (Ministral 8B). Use for file analysis, summarization, or minor code transformations.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "task": { "type": "string", "description": "The specific operation to perform" },
      "context": { "type": "object", "description": "Optional context (code snippets, text)" },
      "format": { "type": "string", "enum": ["text", "json", "code"] }
    },
    "required": ["task"]
  }
}
```

### 3. Implementation Details
- Create `src/clients/local.rs`: Simple HTTP client for OpenAI-compatible completions.
- Create `src/tools/scalpel.rs`: Tool handler that constructing the system prompt for Ministral.
- Update `src/server/router.rs`, `src/schemas.rs`, `lib.rs`, `main.rs`.

## Acceptance Criteria

1.  **Server Connectivity**: `surreal-mind` can successfully ping the `mistral.rs` endpoint.
2.  **Tool Execution**: Calling `scalpel(task="summarize this", context={...})` returns a valid response from the local model.
3.  **Fallback**: Graceful error handling if the local server is down (return error string, don't crash).
4.  **Configurable**: Endpoint and model name can be set via env vars.

## Implementation Steps

1.  [ ] Setup `mistral.rs` locally and verify Ministral 3B inference.
2.  [ ] Create `src/clients/local.rs` with `LocalClient`.
3.  [ ] Create `src/tools/scalpel.rs` handler.
4.  [ ] Register tool in `schemas.rs` and `router.rs`.
5.  [ ] Add integration test (mocked or requiring local server).

## Federation Notes
- **Cognitive Load**: This reduces load on the primary agent.
- **Privacy**: Local inference stays on-device.
- **Cost**: Zero marginal cost per call.
