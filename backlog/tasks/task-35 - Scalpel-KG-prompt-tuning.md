# Task 35: Scalpel KG Tool Prompt Tuning

## Problem
The Scalpel agentic loop works with file I/O (`read_file`, `write_file`, `run_command`) but hits max iterations when attempting KG tools (`think`, `search`, `remember`).

**Root cause:** DeepSeek-Coder-V2-Lite may not be formatting KG tool calls correctly per the expected format:
```
```tool
{"name": "think", "params": {"content": "...", "tags": [...]}}
```
```

## Solution Options

1. **Few-shot examples in system prompt**
   - Add concrete examples of each KG tool call format
   - Pros: Simple, no code changes
   - Cons: Uses context tokens

2. **Structured output mode**
   - Use DeepSeek's native function calling if supported
   - Requires checking mistralrs-server function calling support

3. **Response format validation**
   - Add regex hints in prompt
   - Tell model to double-check format before responding

## Acceptance Criteria
- [ ] `think` tool works end-to-end
- [ ] `search` tool returns results
- [ ] `remember` tool creates entities
- [ ] No infinite loops / max iteration hits

## Recommended Approach
Start with Option 1 (few-shot examples). Update `SSG_SYSTEM_PROMPT` in `src/tools/scalpel.rs` with explicit examples.
