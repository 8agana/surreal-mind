# SurrealMind MCP Testing Plan

Date: 2026-01-19
Owner: SurrealMind Federation
Scope: Verify MCP server protocol compliance, tool behavior, error handling, and integration after cleanup/renames.

## Conventions
- Transport: JSON-RPC 2.0 over stdio or HTTP, depending on runtime. Examples below use JSON-RPC 2.0.
- Replace placeholders (in ALL_CAPS) with real values from the running server.
- Each test includes a concrete JSON payload that can be used directly.
- Response schemas may include additional fields; only required outcomes are listed.

### Common Request Envelope
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "METHOD",
  "params": {}
}
```

### Common Success Response Envelope
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {}
}
```

### Common Error Response Envelope
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32600,
    "message": "Invalid Request",
    "data": {}
  }
}
```

## 1) Protocol Compliance Tests

### MCP-PR-001 Initialize Handshake
Purpose: Validate server initialization and capability negotiation.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2024-11-05",
    "clientInfo": {
      "name": "mcp-test-client",
      "version": "0.1.0"
    },
    "capabilities": {}
  }
}
```
Expected:
- Result includes serverInfo (name, version) and capabilities.
- No error.

### MCP-PR-002 Tools List
Purpose: Verify tools/list returns all exposed tools.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/list",
  "params": {}
}
```
Expected:
- Result includes tools array containing: think, search, remember, wander, maintain, howto,
  call_gem, call_codex, call_cc, call_status, call_jobs, call_cancel, rethink, corrections, test_notification.

### MCP-PR-003 Tools Call Basic Shape
Purpose: Validate tools/call request/response shape.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "howto",
    "arguments": {
      "format": "short",
      "tool": "think"
    }
  }
}
```
Expected:
- Result includes a structured response for the tool (text or JSON).
- No error.

### MCP-PR-004 Notifications Capability (if supported)
Purpose: Ensure server can emit notifications when test_notification is called.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tools/call",
  "params": {
    "name": "test_notification",
    "arguments": {
      "level": "info",
      "message": "MCP notification test"
    }
  }
}
```
Expected:
- Success result.
- A notification event appears on the client (if client supports notifications).

## 2) Individual Tool Tests (Example Payloads)

### MCP-TK-001 think
Purpose: Validate unified thinking with optional memory injection.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 10,
  "method": "tools/call",
  "params": {
    "name": "think",
    "arguments": {
      "content": "Summarize the current testing goal in 2 bullets.",
      "hint": "plan",
      "needs_verification": false
    }
  }
}
```
Expected:
- Result includes a concise response and any metadata fields the server returns.

### MCP-TK-002 search
Purpose: Validate semantic search across thoughts and KG.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 11,
  "method": "tools/call",
  "params": {
    "name": "search",
    "arguments": {
      "query": {
        "text": "testing plan",
        "top_k": 5
      },
      "target": "all"
    }
  }
}
```
Expected:
- Result includes an array of matches with ids and relevance scores.

### MCP-TK-003 remember (entity)
Purpose: Create a KG entity.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 12,
  "method": "tools/call",
  "params": {
    "name": "remember",
    "arguments": {
      "kind": "entity",
      "data": {
        "type": "System",
        "name": "SurrealMind MCP",
        "tags": ["mcp", "testing"]
      },
      "confidence": 0.7
    }
  }
}
```
Expected:
- Result includes created entity id and stored fields.

### MCP-TK-004 remember (relationship)
Purpose: Create a KG relationship.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 13,
  "method": "tools/call",
  "params": {
    "name": "remember",
    "arguments": {
      "kind": "relationship",
      "data": {
        "from": "entity:REPLACE_ENTITY_ID_1",
        "to": "entity:REPLACE_ENTITY_ID_2",
        "type": "depends_on",
        "evidence": "MCP tools rely on core kernel"
      },
      "confidence": 0.6
    }
  }
}
```
Expected:
- Result includes created relationship id.

### MCP-TK-005 wander
Purpose: Traverse KG connections from a known node.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 14,
  "method": "tools/call",
  "params": {
    "name": "wander",
    "arguments": {
      "mode": "breadth",
      "current_thought_id": "entity:REPLACE_ENTITY_ID_1",
      "recency_bias": true,
      "visited_ids": []
    }
  }
}
```
Expected:
- Result returns neighboring nodes/edges or an empty list when none exist.

### MCP-TK-006 maintain
Purpose: Verify maintenance subcommands.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 15,
  "method": "tools/call",
  "params": {
    "name": "maintain",
    "arguments": {
      "subcommand": "health",
      "dry_run": true
    }
  }
}
```
Expected:
- Result includes health status or a summary without modifying data.

### MCP-TK-007 howto
Purpose: Retrieve tool documentation.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 16,
  "method": "tools/call",
  "params": {
    "name": "howto",
    "arguments": {
      "format": "short",
      "tool": "search"
    }
  }
}
```
Expected:
- Result includes concise usage guidance.

### MCP-TK-008 call_gem
Purpose: Delegate to Gemini CLI.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 17,
  "method": "tools/call",
  "params": {
    "name": "call_gem",
    "arguments": {
      "cwd": ".",
      "task_name": "mcp-gem-test",
      "prompt": "Return the word OK.",
      "mode": "observe",
      "timeout_ms": 60000
    }
  }
}
```
Expected:
- Result includes job id and initial status.

### MCP-TK-009 call_codex
Purpose: Delegate to Codex CLI.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 18,
  "method": "tools/call",
  "params": {
    "name": "call_codex",
    "arguments": {
      "cwd": ".",
      "task_name": "mcp-codex-test",
      "prompt": "Echo OK and exit.",
      "mode": "observe",
      "timeout_ms": 60000
    }
  }
}
```
Expected:
- Result includes job id and initial status.

### MCP-TK-010 call_cc
Purpose: Delegate to Claude Code CLI.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 19,
  "method": "tools/call",
  "params": {
    "name": "call_cc",
    "arguments": {
      "cwd": ".",
      "task_name": "mcp-cc-test",
      "prompt": "Return OK.",
      "mode": "observe",
      "timeout_ms": 60000
    }
  }
}
```
Expected:
- Result includes job id and initial status.

### MCP-TK-011 call_status
Purpose: Fetch status for a prior job.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 20,
  "method": "tools/call",
  "params": {
    "name": "call_status",
    "arguments": {
      "job_id": "REPLACE_JOB_ID"
    }
  }
}
```
Expected:
- Result includes status, timestamps, and any outputs/errors.

### MCP-TK-012 call_jobs
Purpose: List recent jobs.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 21,
  "method": "tools/call",
  "params": {
    "name": "call_jobs",
    "arguments": {
      "limit": 5,
      "status_filter": "running"
    }
  }
}
```
Expected:
- Result includes job list with ids and statuses.

### MCP-TK-013 call_cancel
Purpose: Cancel a running job.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 22,
  "method": "tools/call",
  "params": {
    "name": "call_cancel",
    "arguments": {
      "job_id": "REPLACE_JOB_ID"
    }
  }
}
```
Expected:
- Result indicates cancellation requested or completed.

### MCP-TK-014 rethink
Purpose: Mark a record for revision.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 23,
  "method": "tools/call",
  "params": {
    "name": "rethink",
    "arguments": {
      "mode": "mark",
      "mark_type": "review",
      "marked_for": "federation",
      "note": "Verify entity definition",
      "target_id": "entity:REPLACE_ENTITY_ID_1"
    }
  }
}
```
Expected:
- Result includes a mark or revision record.

### MCP-TK-015 corrections
Purpose: List correction events.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 24,
  "method": "tools/call",
  "params": {
    "name": "corrections",
    "arguments": {
      "limit": 10
    }
  }
}
```
Expected:
- Result includes correction events array (possibly empty).

### MCP-TK-016 test_notification
Purpose: Validate notification delivery path.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 25,
  "method": "tools/call",
  "params": {
    "name": "test_notification",
    "arguments": {
      "level": "warn",
      "message": "Notification channel test"
    }
  }
}
```
Expected:
- Success result and a notification event on the client.

## 3) Error Handling Tests

### MCP-ER-001 Invalid JSON-RPC
Purpose: Reject malformed request (missing jsonrpc).
Request:
```json
{
  "id": 30,
  "method": "tools/list",
  "params": {}
}
```
Expected:
- Error response with code -32600 (Invalid Request).

### MCP-ER-002 Unknown Method
Purpose: Reject unknown method.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 31,
  "method": "tools/unknown",
  "params": {}
}
```
Expected:
- Error response with code -32601 (Method not found).

### MCP-ER-003 Unknown Tool
Purpose: tools/call with invalid tool name.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 32,
  "method": "tools/call",
  "params": {
    "name": "not_a_tool",
    "arguments": {}
  }
}
```
Expected:
- Error response indicating tool not found.

### MCP-ER-004 Missing Required Args
Purpose: tools/call missing required arguments.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 33,
  "method": "tools/call",
  "params": {
    "name": "call_status",
    "arguments": {}
  }
}
```
Expected:
- Error response indicating missing job_id.

### MCP-ER-005 Invalid Arg Types
Purpose: tools/call with wrong argument type.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 34,
  "method": "tools/call",
  "params": {
    "name": "corrections",
    "arguments": {
      "limit": "ten"
    }
  }
}
```
Expected:
- Error response indicating invalid type for limit.

### MCP-ER-006 Unauthorized Delegation (if restricted)
Purpose: Ensure delegation tools enforce auth/permissions.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 35,
  "method": "tools/call",
  "params": {
    "name": "call_gem",
    "arguments": {
      "cwd": "/root",
      "task_name": "unauthorized-test",
      "prompt": "noop",
      "mode": "observe"
    }
  }
}
```
Expected:
- Error response indicating permission denial or sandbox violation.

### MCP-ER-007 Timeout Simulation
Purpose: Validate timeout behavior.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 36,
  "method": "tools/call",
  "params": {
    "name": "call_codex",
    "arguments": {
      "cwd": ".",
      "task_name": "timeout-test",
      "prompt": "Sleep 120 seconds.",
      "mode": "observe",
      "timeout_ms": 1000
    }
  }
}
```
Expected:
- Error or status indicating timeout.

## 4) Integration Tests Between Tools

### MCP-IN-001 remember -> search
Purpose: Ensure remembered entity is discoverable by search.
Steps:
1) Run MCP-TK-003 remember to create entity with unique name.
2) Run search for that unique name.
Search Request:
```json
{
  "jsonrpc": "2.0",
  "id": 40,
  "method": "tools/call",
  "params": {
    "name": "search",
    "arguments": {
      "query": {
        "text": "SurrealMind MCP",
        "top_k": 5
      },
      "target": "all"
    }
  }
}
```
Expected:
- Search results include the newly created entity id.

### MCP-IN-002 remember -> wander
Purpose: Ensure relationships affect traversal.
Steps:
1) Create two entities (MCP-TK-003 twice).
2) Create relationship (MCP-TK-004).
3) Run wander from entity A.
Expected:
- Wander returns entity B or the relationship edge.

### MCP-IN-003 think -> search -> think
Purpose: Validate that think can leverage search results.
Steps:
1) Run search for "testing plan".
2) Run think asking to summarize the top result.
Think Request:
```json
{
  "jsonrpc": "2.0",
  "id": 41,
  "method": "tools/call",
  "params": {
    "name": "think",
    "arguments": {
      "content": "Summarize the top search hit in one sentence.",
      "needs_verification": true
    }
  }
}
```
Expected:
- Think output includes a summary grounded in search results.

### MCP-IN-004 Delegation lifecycle
Purpose: Validate job creation, status polling, and cancellation.
Steps:
1) Start call_gem (MCP-TK-008) with a long prompt.
2) call_status until running.
3) call_cancel.
4) call_status again.
Expected:
- Status transitions to cancelled; outputs reflect cancellation.

### MCP-IN-005 rethink -> corrections
Purpose: Ensure rethink events appear in corrections listing.
Steps:
1) Run rethink (MCP-TK-014).
2) Run corrections (MCP-TK-015).
Expected:
- corrections list contains the rethink event or a related record.

## 5) Edge Cases

### MCP-EC-001 Empty search results
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 50,
  "method": "tools/call",
  "params": {
    "name": "search",
    "arguments": {
      "query": {
        "text": "zzzz_nonexistent_query_zzzz",
        "top_k": 3
      },
      "target": "all"
    }
  }
}
```
Expected:
- Valid response with an empty results array.

### MCP-EC-002 Large payloads
Purpose: Ensure server handles large content without crashing.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 51,
  "method": "tools/call",
  "params": {
    "name": "think",
    "arguments": {
      "content": "REPLACE_WITH_20KB_TEXT_BLOCK",
      "hint": "question"
    }
  }
}
```
Expected:
- Response completes or fails gracefully with a size error.

### MCP-EC-003 High concurrency
Purpose: Validate stability under parallel calls.
Steps:
- Issue 10 concurrent tools/call requests (mix of search and howto).
Expected:
- No crashes; responses returned for all requests.

### MCP-EC-004 Idempotent cancel
Purpose: Cancel the same job twice.
Steps:
1) call_cancel on job id.
2) call_cancel again.
Expected:
- Second cancel returns a safe, non-fatal response (already cancelled).

### MCP-EC-005 Stale job id
Purpose: call_status with invalid job id.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 52,
  "method": "tools/call",
  "params": {
    "name": "call_status",
    "arguments": {
      "job_id": "job:does-not-exist"
    }
  }
}
```
Expected:
- Error response indicating job not found.

### MCP-EC-006 Rethink cascade
Purpose: Ensure cascade flag is honored.
Request:
```json
{
  "jsonrpc": "2.0",
  "id": 53,
  "method": "tools/call",
  "params": {
    "name": "rethink",
    "arguments": {
      "mode": "mark",
      "mark_type": "review",
      "cascade": true,
      "target_id": "entity:REPLACE_ENTITY_ID_1"
    }
  }
}
```
Expected:
- Response indicates cascade processing or related derivative marks.

## Acceptance Criteria
- All protocol tests pass.
- Each tool returns a valid response for at least one success path.
- Error handling returns structured errors (no panics).
- Integration tests confirm cross-tool consistency.
- Edge cases fail gracefully or return empty results without server instability.
