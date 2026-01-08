# Task 37: Fix Hermes-3 Tool-Use Protocol Issues

## Objective
Fix the three critical issues preventing reliable tool-use with the local Hermes-3 model (default: Hermes-3-Llama-3.2-3B GGUF) in scalpel implementation:
1. **Protocol Constraints** - Model doesn't know it can use JSON tool syntax
2. **Parser Robustness** - Current parser breaks on valid but verbose responses
3. **Missing Examples** - No few-shot demonstrations of correct tool usage

---

## Background Context

Current failure mode (from investigation):
```
User: Read /Users/samuelatagana/.zshrc and tell me what's in it

Scalpel: I'd be happy to help! However, I don't have direct access to your file system...
```

**Root Cause**: Model treats tool schemas as informational only, doesn't know it's *allowed* to call them.

---

## Priority Fixes

### Priority 1: System Prompt Enhancement (CRITICAL)

**Problem**: The prompts list tools but never explicitly authorize tool calls or show correct usage examples.

**Fix**: Update the existing prompt constants in `surreal-mind/src/tools/scalpel.rs`:
- `SSG_INTEL_PROMPT`
- `SSG_EDIT_PROMPT`

Add:
1. Explicit permission to call tools (no "I don't have access" phrasing).
2. "One tool call OR final answer" rule.
3. 2-3 concrete examples that use the **current schema**:
   `{"name": "read_file", "params": {"path": "/etc/hosts"}}`
4. Reminder that a tool call response must contain ONLY the tool block.

**Location**: `surreal-mind/src/tools/scalpel.rs` (prompt constants).

---

### Priority 2: Robust Tool-Call Parser (HIGH)

**Problem**: `parse_tool_call()` only handles the first fenced block and assumes a single schema. Any preamble text or alternate schema causes a miss.

**Fix**: Update `parse_tool_call()` in `surreal-mind/src/tools/scalpel.rs` to:
1. Scan **all** ```tool```/```json``` blocks and attempt to parse each.
2. Accept both schemas:
   - Canonical: `{"name": "...", "params": {...}}`
   - Legacy: `{"tool": "...", "parameters": {...}}`
3. Fallback: if the whole response is JSON, attempt to parse it.

**Implementation Notes**:
- Use `regex` + `once_cell::sync::Lazy` (both already in Cargo.toml).
- Normalize legacy schema into `ToolCall { name, params }` before use.

**Example Parsing Strategy** (pseudo):
1. For each fenced block match, try parse as `ToolCall`.
2. If that fails, parse as `serde_json::Value` and map `tool/parameters` -> `name/params`.
3. If no fences match, try the whole response as JSON.

---

### Priority 3: Optional ToolDefinition Refactor (MEDIUM)

**Problem**: Tool lists are currently embedded in prompt strings. That is fine for this fix.

**Fix (optional)**: If you want structured tool docs later, add a `ToolDefinition` struct and a builder to generate prompt text. This is not required to fix tool-use.

**Location**: `surreal-mind/src/tools/scalpel.rs` (only if you choose to refactor).

---

## Implementation Plan

### Phase 1: Parser Fix (30 minutes)
1. Update `parse_tool_call()` to scan all fences and accept both schemas
2. Add unit tests for: fenced JSON, surrounding text, bare JSON, and legacy schema
3. Verify parser returns `ToolCall` consistently

### Phase 2: System Prompt Enhancement (45 minutes)
1. Update `SSG_INTEL_PROMPT` and `SSG_EDIT_PROMPT` with explicit tool permission and examples
2. Keep the schema consistent with `ToolCall` (`name/params`)
3. Test with a simple read request

### Phase 3: Integration Testing (30 minutes)
Use the connected `scalpel` MCP tool (not CLI) to verify tool calls:
1. "Read ~/.zshrc and tell me what's in it" (intel)
2. "List files in /tmp directory" (intel)
3. "Create /tmp/test.txt with 'hello'" (edit)

### Phase 4: Documentation (15 minutes)
1. Update scalpel CLI help text with tool-use examples
2. Document expected tool-call format in code comments
3. Add troubleshooting section for common failures

---

## Testing Plan

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_tool_parsing_variations() {
        // Code block with json tag
        // Code block without tag
        // Bare JSON
        // JSON with surrounding text
        // Invalid responses
    }

    #[test]
    fn test_system_prompt_contains_examples() {
        assert!(SSG_INTEL_PROMPT.contains("TOOL USE"));
        assert!(SSG_INTEL_PROMPT.contains("read_file"));
    }
}
```

### Integration Tests (MCP)
Use the `scalpel` MCP tool and confirm tool calls occur:
- Intel: "Read /etc/hosts and summarize it"
- Intel: "What's my username?"
- Edit: "Create /tmp/scalpel-test.txt with content 'working'"
- Error: "Read /nonexistent/file.txt"

---

## Acceptance Criteria

- [ ] Parser extracts tool calls from code blocks (with/without `json` tag)
- [ ] Parser extracts tool calls from responses with surrounding text
- [ ] Parser handles bare JSON (backward compatibility)
- [ ] System prompt explicitly tells model to use tools
- [ ] System prompt includes 2+ examples of tool usage
- [ ] Unit tests pass for all parser variations
- [ ] Integration test: File read succeeds
- [ ] Integration test: Command execution succeeds
- [ ] Integration test: File write succeeds (Edit mode)
- [ ] Model no longer responds "I don't have access" when tools are available

---

## Success Metrics

**Before Fix**:
- Tool-use success rate: ~0% (model doesn't attempt)
- "I don't have access" responses: 100%

**After Fix (Target)**:
- Tool-use success rate: >80% on simple file/command requests
- "I don't have access" responses: <5%
- Parser handles verbose responses: 100%

---

## Dependencies

**Rust Crates**:
- `regex` (already present)
- `once_cell` (already present)
- `serde_json` (existing)

**Files Modified**:
- `surreal-mind/src/tools/scalpel.rs` (primary changes)
**Files Created**:
- None

---

## Risk Assessment

**Low Risk**:
- Parser changes are backward compatible (fallback to old behavior)
- System prompt changes only affect new conversations
- Tool definitions don't change execution logic

**Medium Risk**:
- Regex could have edge cases (mitigated by unit tests)
- Examples in prompt could confuse model (test empirically)

**Mitigation**:
- Comprehensive unit test suite
- Gradual rollout: test with Intel mode first
- Keep old parser as fallback if new one fails

---

## Related Tasks

- **Task-34**: Scalpel Agentic Tool Access (provides context for this fix)
- **Task-35**: Scalpel KG Prompt Tuning (builds on working tool-use)
- **Task-36**: Scalpel Fire-and-Forget Modes (requires reliable tool execution)

**Blocking**: Task-35 and Task-36 are blocked until tool-use protocol works reliably

---

## Notes

**Key Insight from Investigation**: The model is *capable* of tool-use (Hermes-3 is trained for function calling), but the current implementation doesn't give it permission or examples. This is a prompting + parsing issue, not a model capability issue.

**Alternative Considered**: Switch to different model (DeepSeek-Coder, Qwen). Rejected because Hermes-3 is already proven in other contexts, and fixing the protocol is faster than model migration.

**Future Enhancement**: After this works, consider adding reflexion loop where model can self-correct failed tool calls (Task-38 candidate).
