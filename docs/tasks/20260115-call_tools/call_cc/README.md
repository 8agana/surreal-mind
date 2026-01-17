# call_cc Tool

MCP tool for delegating tasks to Claude Code CLI from within surreal-mind.

Status: Planned (implementation plan below).

## Overview

Build `call_cc` following the **synchronous** pattern established by `call_codex` and `call_gem`. The tool handler validates inputs, executes the Claude Code CLI directly, and returns structured results in the same MCP call. No background worker or job queue.

Claude Code CLI differs from Codex in invocation shape (no subcommand, `-p` prompt), output format (stream-json), session continuation flags (`--resume` or `-c`), and model selection (`ANTHROPIC_MODEL` env var instead of `--model` flag).

## CLI Mapping

| Concern | Codex (existing) | Claude Code (new) | Notes |
| --- | --- | --- | --- |
| Binary | `codex` | `claude` | |
| Non-interactive | `exec <prompt>` | `-p <prompt>` | `claude` has no `exec` subcommand |
| JSON output | `--json` (NDJSON) | `--output-format stream-json` | stream-json is the preferred event stream |
| Permissions | `--full-auto` | `--dangerously-skip-permissions` | bypass prompts |
| Model | `--model <name>` | `ANTHROPIC_MODEL=<name>` | env var only |
| Resume | `resume <id>` / `resume --last` | `--resume <id>` / `-c` | `-c` continues last session |
| CWD | `--cd <path>` | `Command::current_dir()` | no `--cd` flag |

## Implementation Plan

### Files to Create

#### 1. `src/clients/claude.rs`

Create `ClaudeClient` following the pattern in `src/clients/codex.rs`:

```rust
pub struct ClaudeClient {
    model: String,
    cwd: Option<String>,
    resume_session_id: Option<String>,
    continue_latest: bool,
    tool_timeout_ms: u64,
    expose_stream: bool,
}

impl ClaudeClient {
    pub fn new(model: Option<String>) -> Self { ... }
    
    // Builder methods
    pub fn with_cwd(mut self, cwd: &str) -> Self { ... }
    pub fn with_resume_session_id(mut self, id: String) -> Self { ... }
    pub fn with_continue_latest(mut self, val: bool) -> Self { ... }
    pub fn with_tool_timeout_ms(mut self, ms: u64) -> Self { ... }
    pub fn with_expose_stream(mut self, val: bool) -> Self { ... }
    
    /// Execute the Claude Code CLI and return parsed result
    pub async fn execute(&self, prompt: &str) -> Result<ClaudeResponse, AgentError> { ... }
}
```

**Command construction:**

```rust
let mut cmd = Command::new("claude");
cmd.arg("-p").arg(prompt);
cmd.arg("--dangerously-skip-permissions");
cmd.arg("--output-format").arg("stream-json");

// Resume handling
if let Some(ref session_id) = self.resume_session_id {
    cmd.arg("--resume").arg(session_id);
} else if self.continue_latest {
    cmd.arg("-c");
}

// Model via env var (not CLI flag)
cmd.env("ANTHROPIC_MODEL", &self.model);

// CWD via Command method (no --cd flag)
if let Some(ref cwd) = self.cwd {
    cmd.current_dir(cwd);
}
```

**Response parsing:**

- Parse stream-json output for session id and response text
- Extract session id from relevant event type
- Map CLI errors to `AgentError` with helpful hints

#### 2. `src/tools/call_cc.rs`

Create **synchronous** tool handler following `src/tools/call_codex.rs` (160 lines):

```rust
//! call_cc tool handler - synchronous Claude Code CLI execution

use crate::clients::claude::ClaudeClient;
use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::Deserialize;
use serde_json::{Value, json};

const FALLBACK_MODEL: &str = "claude-sonnet-4-5";
const DEFAULT_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_TOOL_TIMEOUT_MS: u64 = 300_000;

fn get_default_model() -> String {
    std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| FALLBACK_MODEL.to_string())
}

#[derive(Debug, Deserialize)]
pub struct CallCcParams {
    pub prompt: String,
    #[serde(default)]
    pub task_name: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub resume_session_id: Option<String>,
    #[serde(default)]
    pub continue_latest: bool,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub tool_timeout_ms: Option<u64>,
    #[serde(default)]
    pub expose_stream: bool,
}

impl SurrealMindServer {
    pub async fn handle_call_cc(&self, request: CallToolRequestParam) -> Result<CallToolResult> {
        // Parse and validate params (same pattern as call_codex)
        // - Require non-empty prompt
        // - Require cwd
        // - Disallow resume_session_id + continue_latest combo
        
        // Build ClaudeClient
        let mut claude = ClaudeClient::new(Some(model));
        claude = claude.with_cwd(&cwd).with_tool_timeout_ms(tool_timeout_ms);
        // ... set resume options, expose_stream
        
        // Execute with timeout
        let timeout = std::time::Duration::from_millis(timeout_ms);
        let execution = tokio::time::timeout(timeout, claude.execute(&prompt))
            .await
            .map_err(|_| SurrealMindError::Mcp { message: "timeout" })?
            .map_err(|e| SurrealMindError::Mcp { message: e.to_string() })?;
        
        // Return structured result
        Ok(CallToolResult::structured(json!({
            "status": "completed",
            "response": execution.response,
            "session_id": execution.session_id,
            "metadata": metadata
        })))
    }
}
```

### Files to Modify

1. **`src/clients/mod.rs`** - Add: `pub mod claude; pub use claude::ClaudeClient;`

2. **`src/tools/mod.rs`** - Add: `pub mod call_cc;`

3. **`src/schemas.rs`** - Add `call_cc_schema()` with:
   - Model enum from `ANTHROPIC_MODELS` env var (like call_codex)
   - Default model from `ANTHROPIC_MODEL` env var

4. **Router registration** - Add handler wiring in tool router

### Schema Definition

```rust
pub fn call_cc_schema() -> (String, serde_json::Value) {
    // Read models from ANTHROPIC_MODELS env var (comma-separated)
    let models = std::env::var("ANTHROPIC_MODELS")
        .unwrap_or_else(|_| "claude-sonnet-4-5,claude-haiku-4-5,claude-opus-4-5".to_string())
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    
    let default_model = std::env::var("ANTHROPIC_MODEL")
        .unwrap_or_else(|_| "claude-sonnet-4-5".to_string());
    
    (
        "call_cc".to_string(),
        json!({
            "prompt": { "type": "string", "description": "Task to delegate" },
            "cwd": { "type": "string", "description": "Working directory (required)" },
            "model": { "type": "string", "enum": models, "default": default_model },
            "resume_session_id": { "type": "string" },
            "continue_latest": { "type": "boolean", "default": false },
            "timeout_ms": { "type": "number", "default": 60000 },
            "tool_timeout_ms": { "type": "number", "default": 300000 },
            "expose_stream": { "type": "boolean", "default": false }
        })
    )
}
```

## Key Differences from Original Plan

1. **No async worker** - Removed `fire_and_forget` parameter and job queue pattern
2. **Synchronous execution** - Handler directly executes CLI and returns result
3. **No DB writes** - No agent_exchanges, tool_sessions, or agent_jobs tables
4. **Environment-based config** - Model list from `ANTHROPIC_MODELS` env var
5. **Simpler codebase** - ~160 lines for tool handler (vs. 700+ with async)

## Testing Plan

1. **Unit tests:**
   - `src/tools/call_cc.rs`: assert defaults (model, timeout)
   - Validate `resume_session_id` vs `continue_latest` guard
   - Test `normalize_optional_string` utility

2. **Schema tests:**
   - Ensure `call_cc` appears in tool listing
   - Verify model enum population from env var

3. **Integration (manual):**
   - Run a small prompt in a safe repo
   - Verify stream-json parsing and session id extraction
   - Test resume functionality with `-c` and `--resume`

## Reference Files

- `src/clients/codex.rs` (client pattern - ~200 lines)
- `src/tools/call_codex.rs` (handler pattern - ~160 lines)
- `docs/tasks/20260115-call_tools/notebooklm-research.md` (CLI details)
