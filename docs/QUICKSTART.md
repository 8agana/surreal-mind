# Surreal-Mind MCP Quick Start

Quick reference for calling `surreal-mind` MCP tools.

## Prerequisites

- Server running: `cargo run` (stdio) or `cargo run -- --http` (HTTP on :6600)
- Connection: stdio MCP or HTTP with JSON-RPC

---

## Core Tool: `legacymind_think`

The primary tool for storing and processing thoughts.

### Basic Call (Question Mode)
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "legacymind_think",
    "arguments": {
      "content": "What is the best approach for modularizing large Rust files?",
      "tags": ["architecture", "rust"]
    }
  }
}
```

**Response:**
```json
{
  "thought_id": "abc123...",
  "embedding_dim": 1536,
  "mode_selected": "question",
  "memories_injected": 3,
  "continuity": { "session_id": null, "chain_id": null }
}
```

### Debug Mode (with hint)
```json
{
  "arguments": {
    "content": "Stack overflow in recursive function call",
    "hint": "debug",
    "injection_scale": 3
  }
}
```

### Plan Mode (with continuity)
```json
{
  "arguments": {
    "content": "Designing the authentication layer architecture",
    "hint": "plan",
    "session_id": "session-2025-01-06",
    "chain_id": "auth-design"
  }
}
```

---

## Search: `unified_search`

Search across thoughts, entities, and observations.

```json
{
  "name": "unified_search",
  "arguments": {
    "query": "authentication patterns",
    "kind": "thoughts",
    "limit": 5,
    "min_similarity": 0.7
  }
}
```

---

## Delegation: `delegate_gemini`

Delegate complex tasks to Gemini CLI.

```json
{
  "name": "delegate_gemini",
  "arguments": {
    "task_name": "analyze-codebase",
    "prompt": "Review the error handling patterns in src/error.rs"
  }
}
```

**Returns:** Job ID for async tracking via `agent_job_status`.

---

## Knowledge Graph: `knowledgegraph_create`

Create entities and observations in the KG.

```json
{
  "name": "knowledgegraph_create",
  "arguments": {
    "entity_name": "Rust Modularization",
    "entity_type": "Pattern",
    "description": "Best practices for splitting large Rust modules"
  }
}
```

---

## Hints Reference

| Hint | Use Case | Default injection_scale |
|------|----------|------------------------|
| `debug` | Error investigation | 3 |
| `build` | Implementation work | 2 |
| `plan` | Architecture design | 3 |
| `stuck` | Blocked/confused | 3 |
| `question` | General inquiry | 1 |
| `conclude` | Session wrap-up | 1 |

---

## More Info

- Full tool list: `detailed_help` tool
- Schema validation: `tests/tool_schemas.rs`  
- Integration tests: `tests/test_mcp_comprehensive.sh`
