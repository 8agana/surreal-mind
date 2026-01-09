# Task 35: Scalpel Simplification (Surgical Knife Mode)

## Problem
Scalpel is currently over-engineered with Knowledge Graph access (`think`, `search`, `remember`) which complicates the system prompt and increases failure rates. It has not yet reliably demonstrated basic file operations.

## Goal
Simplify Scalpel to be a pure **Code/File Operator** ("Surgical Knife"). Its only purpose is to perform file I/O and shell commands on behalf of the main agent to save context and tokens.

## Scope of Changes

1. **Remove KG Tools from Prompt**
   - Remove `think`, `search`, `remember` from `SSG_EDIT_PROMPT`.
   - Remove examples related to KG tools.

2. **Refine Prompt for Core Operations**
   - Focus `SSG_EDIT_PROMPT` exclusively on:
     - `read_file`
     - `write_file`
     - `run_command`
   - Add explicit, robust examples for these three tools to ensure reliable JSON formatting.

3. **Code Cleanup (Optional/Later)**
   - The backing code in `scalpel_helpers.rs` and `execute_tool` can remain for now (invisible to the model) or be commented out to prevent accidental usage. For this task, we will just hide them from the prompt.

## Acceptance Criteria
- [ ] Scalpel prompt no longer advertises KG tools.
- [ ] Scalpel reliably performs `read_file`.
- [ ] Scalpel reliably performs `write_file`.
- [ ] Scalpel reliably performs `run_command`.

## Implementation Plan

Modify `src/tools/scalpel.rs`:

```rust
pub const SSG_EDIT_PROMPT: &str = r#"You are SSG Scalpel, a precision code executor.

Available tools:
- read_file(path): Read file contents
- write_file(path, content): Write content to file
- run_command(command): Execute shell command

TOOL USE (READ THIS):
- You are allowed to call tools directly.
- If you need file or command access, CALL A TOOL. Do NOT say "I don't have access."
- Your response MUST be either a single tool call block OR a final answer, never both.
- Use the exact JSON structure shown in examples.

To use a tool:
```tool
{"name": "tool_name", "params": {"key": "value"}}
```

Examples:

1. READ FILE:
User: Read /etc/hosts
Assistant:
```tool
{"name": "read_file", "params": {"path": "/etc/hosts"}}
```

2. WRITE FILE:
User: Create /tmp/hello.txt with "world"
Assistant:
```tool
{"name": "write_file", "params": {"path": "/tmp/hello.txt", "content": "world"}}
```

3. RUN COMMAND:
User: List files in current directory
Assistant:
```tool
{"name": "run_command", "params": {"command": "ls -la"}}
```

When done, respond with your final answer (NO tool block).
Execute decisively. Stay in scope. Verify completion."#;
```

## Test Findings (2026-01-08)
- **Overall Status:** FAILED. The tool is currently a "Blunt Mallet," not a Scalpel.
- **Critical Failures:**
    - **Destructive Writing:** In `edit` mode, the tool repeatedly overwrote entire files with small snippets of text, even when explicitly provided with the full content to preserve. It failed to distinguish between "append" and "replace."
    - **Path Blindness:** The tool failed to locate files using relative paths, requiring absolute paths (`/Users/...`) to function, which is brittle and inconsistent with standard agent expectations.
    - **Instruction Drift:** Even when given synchronous, step-by-step instructions via `fire_and_forget: false`, the underlying Ministral 3B agent failed to follow logical constraints (e.g., "do not overwrite").
- **Partial Successes:**
    - **Read Operations:** Successfully read and summarized `surreal-mind/README.md` ONLY when provided with an absolute path.
    - **Shell Execution:** Successfully appended text to this file ONLY by bypassing its own `write_file` logic and using a raw shell command (`printf >>`). 
- **Conclusion:** Scalpel is currently unreliable for any surgical file operations. The acceptance criteria for `read_file`, `write_file`, and `run_command` remain UNMET in any production-ready sense.

## New Remediation Plan (Safe Surgical Knife)

### 1. Fix Path Blindness
- Implement `resolve_path(path: &str) -> PathBuf` helper.
- If path is relative, join with `std::env::current_dir()`.
- Canonicalize resulting path to resolve `..` and symlinks.

### 2. Prevent Destructive Writes
- **Action:** Modify `write_file` implementation.
- **Logic:** check if file exists. If yes, return Error: "File exists. Use 'append_file' to add content or 'run_command' to overwrite/patch."
- **Note:** We can add an `overwrite: true` param later if needed, but safety first.

### 3. Add `append_file` Tool
- **New Tool:** `append_file(path, content)`
- **Implementation:** Use `fs::OpenOptions::new().append(true).open(path)`.
- **Why:** Safest way for a smaller model (3B) to add content without reading/rewriting the whole file (context limits) or risking data loss.

### 4. Update System Prompt
- Add `append_file` to tool list.
- Explicitly warn about `write_file` safety check.

## Test Findings (2026-01-08 Round 2 - Model Mismatch)
- **Status:** FAILED.
- **Issue:** The tool crashed with a 500 error because the code defaulted to requesting `Qwen` while the server hosted `Hermes`.
- **Resolution:**
    - Updated `src/clients/local.rs` to remove hardcoded defaults and REQUIRE `SURR_SCALPEL_MODEL` from .env.
    - Updated `src/tools/scalpel.rs` to log the correct agent instance.
    - Added config to `.env.example`.
- **Current State:** Config mismatch resolved. Ready for functional testing.

## Test Findings (2026-01-08 Round 3 - Functional Test via CC)
- **Test 1 (Echo):** `scalpel test ok` → **PASSED**. Model is reachable and responding.
- **Test 2 (Create File):** `Create ... scalpel-test.txt` → **PARTIAL FAIL**.
    - Model claimed success: "The file ... has been successfully created with the content..."
    - Reality: File created but **EMPTY (0 bytes)**.
- **Test 3 (Append File):** `Append ...` → **FAILED**.
    - Model hit max iterations (10).
    - Likely kept trying to append, failing (or confusing itself), and retrying.
    - Underlying cause: working with an empty file might have confused the "append" logic or the model's verification step.

## Diagnosis (Zero-Byte Write)
The model claimed it wrote content, but the file was empty. This strongly suggests a bug in `src/tools/scalpel.rs` where the `content` parameter is being dropped or misparsed from the tool call arguments before being passed to `fs::write`.

**Next Steps:**
1. Investigate `execute_tool` in `scalpel.rs`.
2. Verify how `tool.params["content"]` is extracted.
3. Check `normalize_tool_call` logic to ensure parameters aren't lost during parsing.
