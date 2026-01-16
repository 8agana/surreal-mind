# MCP Delegation Tools Evaluation

**Date:** 2026-01-15
**Purpose:** Evaluate existing MCP delegation patterns to inform `call_cc` tool design
**Status:** In Progress

---

## 1. Overview

Testing four MCP delegation implementations to understand interface patterns, parameter design, and operational characteristics before implementing `call_cc`.

### Tools Under Evaluation

| Tool | Server | Purpose |
|------|--------|---------|
| `call_gem` | surreal-mind | Gemini CLI delegation |
| `mcp__claude-code-mcp__*` | claude-code-mcp | Claude Code delegation |
| `mcp__gemini-cli__*` | gemini-cli | Gemini CLI (MCP wrapper) |
| `mcp__codex-cli__*` | codex-cli | OpenAI Codex delegation |
| `mcp__vibe-cli__*` | vibe-cli | Mistral Vibe delegation |

---

## 2. Evaluation Criteria

### 2.1 Interface Design
- Tool naming conventions
- Parameter structure (flat vs nested)
- Required vs optional parameters
- Default value patterns

### 2.2 Parameter Patterns
- How prompts are passed
- Model selection mechanism
- Timeout/resource configuration
- Working directory handling

### 2.3 Error Handling
- Error response format
- Timeout behavior
- Partial failure handling
- Recovery mechanisms

### 2.4 Output Format
- Response structure
- Streaming vs blocking
- Metadata inclusion
- Status reporting

### 2.5 Context Passing
- How context is provided to delegated agent
- File/code context mechanisms
- Conversation history handling

### 2.6 Session Support
- Fire-and-forget vs tracked jobs
- Job status querying
- Cancellation support
- Session persistence

---

## 3. Test Results

### 3.1 claude-code-mcp

| Criterion | Observation | Notes |
|-----------|-------------|-------|
| Interface Design | | |
| Parameter Patterns | | |
| Error Handling | | |
| Output Format | | |
| Context Passing | | |
| Session Support | | |

**Raw Test Output:**
```
(paste test results here)
```

---

### 3.2 gemini-cli (MCP)

| Criterion | Observation | Notes |
|-----------|-------------|-------|
| Interface Design | | |
| Parameter Patterns | | |
| Error Handling | | |
| Output Format | | |
| Context Passing | | |
| Session Support | | |

**Raw Test Output:**
```
(paste test results here)
```

---

### 3.3 call_gem (surreal-mind native)

| Criterion | Observation | Notes |
|-----------|-------------|-------|
| Interface Design | | |
| Parameter Patterns | | |
| Error Handling | | |
| Output Format | | |
| Context Passing | | |
| Session Support | | |

**Raw Test Output:**
```
(paste test results here)
```

---

### 3.4 codex-cli

| Criterion | Observation | Notes |
|-----------|-------------|-------|
| Interface Design | | |
| Parameter Patterns | | |
| Error Handling | | |
| Output Format | | |
| Context Passing | | |
| Session Support | | |

**Raw Test Output:**
```
(paste test results here)
```

---

### 3.5 vibe-cli

| Criterion | Observation | Notes |
|-----------|-------------|-------|
| Interface Design | | |
| Parameter Patterns | | |
| Error Handling | | |
| Output Format | | |
| Context Passing | | |
| Session Support | | |

**Raw Test Output:**
```
(paste test results here)
```

---

## 4. Best Practices Identified

### 4.1 Parameter Design
-

### 4.2 Error Handling
-

### 4.3 Output Format
-

### 4.4 Context Management
-

### 4.5 Session/Job Tracking
-

---

## 5. Synthesis: Unified Pattern for `call_cc`

### 5.1 Recommended Interface

```rust
// Parameter structure
struct CallCcParams {
    // TBD based on evaluation
}
```

### 5.2 Key Design Decisions

| Decision | Chosen Approach | Rationale |
|----------|-----------------|-----------|
| | | |

### 5.3 Differentiation from Existing Tools

What `call_cc` should do differently:
-

### 5.4 Implementation Notes

-

---

## Appendix: Tool Schemas

### claude-code-mcp schema
```json
(paste schema here)
```

### gemini-cli schema
```json
(paste schema here)
```

### codex-cli schema
```json
(paste schema here)
```

### vibe-cli schema
```json
(paste schema here)
```
