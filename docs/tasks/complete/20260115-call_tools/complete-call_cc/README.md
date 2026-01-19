# call_cc Tool

MCP tool for delegating tasks to Claude Code CLI from within surreal-mind.

Status: **In Progress**

## Overview

Build `call_cc` following the **synchronous** pattern established by `call_codex` and `call_gem`. The tool handler validates inputs, executes the Claude Code CLI directly, and returns structured results in the same MCP call. No background worker or job queue.

## CLI Mapping (Verified)

| Concern | Codex | Claude Code | Notes |
| --- | --- | --- | --- |
| Binary | `codex` | `claude` | |
| Non-interactive | `exec <prompt>` | `-p <prompt>` | No subcommand for Claude |
| JSON output | `--json` (NDJSON) | `--output-format stream-json` | Verified via NBLM |
| Permissions | `--full-auto` | `--dangerously-skip-permissions` | CC's "yolo mode" |
| Model | `--model <name>` | `ANTHROPIC_MODEL=<name>` | Env var only |
| Continue latest | `resume --last` | `-c` / `--continue` | Resume most recent at cwd |
| Resume specific | `resume <id>` | `--resume <id>` | Must have session ID |
| CWD | `--cd <path>` | `Command::current_dir()` | No --cd flag |

## Key Findings (from NBLM Research)

### Output & Formatting

- `--output-format stream-json` sends NDJSON to **stdout**
- No `--no-color` needed - stream-json auto-suppresses spinners/interactive elements
- Env var `CLAUDE_CODE_DISABLE_TERMINAL_TITLE=1` available if needed

### Session ID Discovery

- Session ID field in stream events: **TBD (need empirical test)**
- Environment exposes `${CLAUDE_SESSION_ID}` - may be in events too
- Will need to parse actual output to find field name

### Timeout Environment Variables (Milliseconds!)

| Variable | Purpose | Default |
|----------|---------|---------|
| `MCP_TOOL_TIMEOUT` | Tool execution timeout | 600,000ms (10 min) |
| `BASH_DEFAULT_TIMEOUT_MS` | Bash command timeout | varies |
| `BASH_MAX_TIMEOUT_MS` | Max bash timeout | varies |

### Error Detection

Errors follow MCP protocol with `isError: true`:

```json
{
  "isError": true,
  "content": [{ "type": "text", "text": "Error message..." }]
}
```

## Implementation Plan

### Files to Create

#### 1. `src/clients/claude.rs` (~200 lines)

```rust
pub struct ClaudeClient {
    model: String,
    cwd: Option<String>,
    resume_session_id: Option<String>,
    continue_latest: bool,
    tool_timeout_ms: u64,
    expose_stream: bool,
}
```

**Command construction:**

```rust
let mut cmd = Command::new("claude");
cmd.arg("-p").arg(prompt);
cmd.arg("--dangerously-skip-permissions");
cmd.arg("--output-format").arg("stream-json");
cmd.stdin(Stdio::null());
cmd.stdout(Stdio::piped());
cmd.stderr(Stdio::piped());

// Resume handling (mutually exclusive)
if let Some(ref session_id) = self.resume_session_id {
    cmd.arg("--resume").arg(session_id);
} else if self.continue_latest {
    cmd.arg("-c");
}

// Model via env var
cmd.env("ANTHROPIC_MODEL", &self.model);

// Tool timeout via env var (MS)
cmd.env("MCP_TOOL_TIMEOUT", self.tool_timeout_ms.to_string());

// CWD via Command method
if let Some(ref cwd) = self.cwd {
    cmd.current_dir(cwd);
}
```

**Response parsing:**

- Parse NDJSON lines from stdout
- Look for `isError: true` → return error
- Extract session_id from events (field TBD)
- Accumulate response text from content events

#### 2. `src/tools/call_cc.rs` (~160 lines)

Synchronous handler following `call_codex.rs` pattern:

- Parse and validate params
- Require non-empty prompt and cwd
- Disallow `resume_session_id` + `continue_latest` combo
- Build ClaudeClient, execute with timeout
- Return structured result

### Files to Modify

1. **`src/clients/mod.rs`** - Add `pub mod claude;`
2. **`src/tools/mod.rs`** - Add `pub mod call_cc;`
3. **`src/schemas.rs`** - Add `call_cc_schema()` with model enum
4. **Router** - Register `handle_call_cc` handler

## Reference Files

- `src/clients/codex.rs` (client pattern)
- `src/tools/call_codex.rs` (handler pattern)
