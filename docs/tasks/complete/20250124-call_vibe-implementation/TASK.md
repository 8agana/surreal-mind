# call_vibe Implementation Plan

## Overview

Add a new `call_vibe` tool to surreal-mind MCP server for delegating tasks to Mistral's Vibe CLI. Vibe is a one-shot executor similar to `call_warp` - no stream-json output, no session persistence.

**CLI Location:** `~/.local/bin/vibe` (already in PATH via run-http.sh fix)

**Invocation Pattern:**
```bash
vibe --auto-approve -p "<prompt>" [--agent <profile_name>]
```

**Tool Count After Addition: 16 tools**

## Vibe CLI Characteristics

| Feature | Support | Notes |
|---------|---------|-------|
| JSON output | ❌ | Raw text/stdout only |
| Model selection | Via `--agent` | Profiles in `~/.vibe/agents/*.toml` |
| Session resume | ❌ | Not yet supported |
| Auto-approve | ✅ | `--auto-approve` flag |

## Step-by-Step Implementation

### Phase 1: Client Implementation

#### 1. Create Vibe Client
- **File**: `src/clients/vibe.rs` (NEW)
- **Action**: Create file with contents:

```rust
//! Vibe CLI client for call_vibe tool
//! Mistral Vibe - one-shot executor, no session persistence

use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use tokio::process::Command;

use crate::clients::traits::{AgentError, AgentResponse, CognitiveAgent};

#[derive(Debug, Clone)]
pub struct VibeClient {
    agent: Option<String>,  // --agent flag (profile name)
    cwd: Option<PathBuf>,
    timeout_ms: u64,
}

#[derive(Debug)]
pub struct VibeExecution {
    pub response: String,
    pub stdout: String,
    pub stderr: String,
    pub is_error: bool,
}

impl VibeClient {
    pub fn new(agent: Option<String>) -> Self {
        Self {
            agent,
            cwd: None,
            timeout_ms: 60_000,
        }
    }

    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    pub async fn execute(&self, prompt: &str) -> Result<VibeExecution, AgentError> {
        let mut cmd = Command::new("vibe");
        cmd.kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(ref cwd) = self.cwd {
            cmd.current_dir(cwd);
        }

        // vibe --auto-approve -p "<prompt>" [--agent <name>]
        cmd.arg("--auto-approve");
        cmd.arg("-p").arg(prompt);

        if let Some(ref agent) = self.agent {
            cmd.arg("--agent").arg(agent);
        }

        // Execute with timeout
        let timeout = std::time::Duration::from_millis(self.timeout_ms);
        let output = tokio::time::timeout(timeout, cmd.output())
            .await
            .map_err(|_| AgentError::Timeout {
                timeout_ms: self.timeout_ms,
            })?
            .map_err(map_spawn_err)?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if stdout.trim().is_empty() && !stderr.trim().is_empty() {
            return Err(AgentError::CliError(format!(
                "vibe produced no stdout: {}",
                truncate_snippet(stderr.trim(), 500)
            )));
        }

        if !output.status.success() {
            return Err(AgentError::CliError(format!(
                "vibe exit {}: {}",
                output.status,
                truncate_snippet(stderr.trim(), 500)
            )));
        }

        let response = stdout.trim().to_string();
        if response.is_empty() {
            return Err(AgentError::CliError(
                "Empty Vibe response: no content captured.".to_string(),
            ));
        }

        Ok(VibeExecution {
            response,
            stdout,
            stderr,
            is_error: false,
        })
    }
}

#[async_trait]
impl CognitiveAgent for VibeClient {
    async fn call(
        &self,
        prompt: &str,
        _session_id: Option<&str>, // Vibe doesn't support sessions
    ) -> Result<AgentResponse, AgentError> {
        let execution = self.execute(prompt).await?;
        Ok(AgentResponse {
            session_id: String::new(), // Vibe has no sessions
            response: execution.response,
            exchange_id: None,
            stream_events: None,
        })
    }
}

fn map_spawn_err(err: std::io::Error) -> AgentError {
    if err.kind() == std::io::ErrorKind::NotFound {
        AgentError::NotFound
    } else {
        AgentError::CliError(err.to_string())
    }
}

fn truncate_snippet(input: &str, max: usize) -> String {
    if input.len() <= max {
        return input.to_string();
    }
    format!("{}...", &input[..max])
}
```

#### 2. Export Vibe Client
- **File**: `src/clients/mod.rs`
- **Action**: Add two lines after existing warp exports:

```rust
pub mod vibe;
pub use vibe::VibeClient;
```

### Phase 2: Tool Handler Implementation

#### 3. Create Tool Handler
- **File**: `src/tools/call_vibe.rs` (NEW)
- **Action**: Create file with contents:

```rust
//! call_vibe tool handler - synchronous Vibe CLI execution
//! Vibe is a one-shot executor - no session persistence or resume support

use crate::clients::vibe::VibeClient;
use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::Deserialize;
use serde_json::{json, Value};

const DEFAULT_TIMEOUT_MS: u64 = 60_000;

/// Parameters for the call_vibe tool
#[derive(Debug, Deserialize)]
pub struct CallVibeParams {
    pub prompt: String,
    /// Working directory for the Vibe CLI subprocess (required)
    pub cwd: String,
    /// Agent profile name from ~/.vibe/agents/*.toml
    #[serde(default)]
    pub agent: Option<String>,
    /// Mode: "execute" (normal) or "observe" (read-only analysis)
    #[serde(default)]
    pub mode: Option<String>,
    /// Timeout in milliseconds (default: 60000)
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Max characters for response (default: 100000)
    #[serde(default)]
    pub max_response_chars: Option<i64>,
}

impl SurrealMindServer {
    /// Handle the call_vibe tool call - synchronous execution
    pub async fn handle_call_vibe(&self, request: CallToolRequestParam) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: CallVibeParams = serde_json::from_value(Value::Object(args)).map_err(|e| {
            SurrealMindError::InvalidParams {
                message: format!("Invalid parameters: {}", e),
            }
        })?;

        let prompt = params.prompt.trim().to_string();
        if prompt.is_empty() {
            return Err(SurrealMindError::InvalidParams {
                message: "prompt cannot be empty".into(),
            });
        }

        let cwd = params.cwd.trim().to_string();
        if cwd.is_empty() {
            return Err(SurrealMindError::InvalidParams {
                message: "cwd is required and cannot be empty".into(),
            });
        }

        // Apply federation context and observe mode prefix
        let observe_prefix = if params.mode.as_deref() == Some("observe") {
            "You are in OBSERVE mode. Analyze and report only. Do NOT make any file changes.\n\n"
        } else {
            ""
        };
        let prompt = format!(
            "[FEDERATION CONTEXT: You are being invoked as a subagent by surreal-mind MCP. Your output will be returned to the calling agent.]\n\n{}{}",
            observe_prefix, prompt
        );

        let agent = normalize_optional_string(params.agent);
        let timeout_ms = params.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);

        // Build and execute VibeClient synchronously
        let mut vibe = VibeClient::new(agent);
        vibe = vibe.with_cwd(&cwd).with_timeout_ms(timeout_ms);

        // Execute with timeout
        let timeout = std::time::Duration::from_millis(timeout_ms);
        let execution = tokio::time::timeout(timeout, vibe.execute(&prompt))
            .await
            .map_err(|_| SurrealMindError::Mcp {
                message: format!("Vibe execution timed out after {}ms", timeout_ms),
            })?
            .map_err(|e| SurrealMindError::Mcp {
                message: format!("Vibe execution failed: {}", e),
            })?;

        // Build response
        let mut metadata = serde_json::Map::new();
        if !execution.stderr.trim().is_empty() {
            metadata.insert(
                "stderr".to_string(),
                Value::String(execution.stderr.clone()),
            );
        }

        Ok(CallToolResult::structured(json!({
            "status": "completed",
            "response": truncate_response(execution.response, params.max_response_chars),
            "metadata": if metadata.is_empty() { Value::Null } else { Value::Object(metadata) }
        })))
    }
}

/// Default max response chars (100KB)
const DEFAULT_MAX_RESPONSE_CHARS: usize = 100_000;

/// Truncate response if over limit
fn truncate_response(response: String, max_chars: Option<i64>) -> String {
    let limit = match max_chars {
        Some(n) if n > 0 => n as usize,
        Some(0) => return response, // 0 = no limit
        _ => DEFAULT_MAX_RESPONSE_CHARS,
    };

    if response.len() <= limit {
        response
    } else {
        let truncated = &response[..limit];
        format!(
            "{}...\n\n[TRUNCATED: Response was {} chars, limit is {}]",
            truncated,
            response.len(),
            limit
        )
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vibe_defaults() {
        assert_eq!(DEFAULT_TIMEOUT_MS, 60_000);
        assert_eq!(DEFAULT_MAX_RESPONSE_CHARS, 100_000);
    }
}
```

#### 4. Export Tool Handler
- **File**: `src/tools/mod.rs`
- **Action**: Add line after existing call_warp export:

```rust
pub mod call_vibe;
```

### Phase 3: Schema and Router Updates

#### 5. Add Schema Definition
- **File**: `src/schemas.rs`
- **Action**: Add function after `call_warp_schema()`:

```rust
pub fn call_vibe_schema() -> Arc<Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "prompt": {"type": "string"},
            "cwd": {"type": "string"},
            "agent": {
                "type": "string",
                "description": "Agent profile name from ~/.vibe/agents/*.toml"
            },
            "mode": {
                "type": "string",
                "enum": ["execute", "observe"],
                "default": "execute",
                "description": "execute: normal operation with file changes. observe: analyze and report only, no file modifications."
            },
            "timeout_ms": {"type": "number", "default": 60000},
            "max_response_chars": {
                "type": "integer",
                "default": 100000,
                "description": "Max chars for response (0 = no limit, default 100000)"
            }
        },
        "required": ["prompt", "cwd"]
    });
    Arc::new(schema.as_object().cloned().unwrap_or_else(Map::new))
}
```

#### 6. Register Schema in Router
- **File**: `src/server/router.rs`
- **Action**: In `list_tools()` function, add schema variable declaration near other call_* schemas:

```rust
let call_vibe_schema = crate::schemas::call_vibe_schema();
```

#### 7. Add Tool to Tools List
- **File**: `src/server/router.rs`
- **Action**: In `list_tools()` function, add to tools vector after call_warp:

```rust
tools.push(Tool {
    name: "call_vibe".into(),
    title: Some("Call Vibe".into()),
    description: Some("Delegate a task to Vibe CLI with full context and tracking".into()),
    input_schema: call_vibe_schema.clone(),
    icons: None,
    annotations: None,
    output_schema: None,
    meta: None,
});
```

#### 8. Add Call Handler Route
- **File**: `src/server/router.rs`
- **Action**: In `call_tool()` match statement, add after call_warp:

```rust
"call_vibe" => self.handle_call_vibe(request).await.map_err(|e| e.into()),
```

### Phase 4: Howto Tool Integration

#### 9. Add to Howto Overview
- **File**: `src/tools/howto.rs`
- **Action**: Add to tools vector in overview mode (after call_warp entry):

```rust
json!({"name": "call_vibe", "one_liner": "Delegate a prompt to the Vibe CLI agent", "key_params": ["prompt", "cwd", "agent", "mode"]}),
```

#### 10. Add Detailed Help Case
- **File**: `src/tools/howto.rs`
- **Action**: Add case in detailed help match (after call_warp):

```rust
"call_vibe" => json!({
    "name": "call_vibe",
    "description": "Delegate a prompt to the Vibe CLI agent. Supports agent profiles and observe mode.",
    "arguments": {
        "prompt": "string (required) — the prompt text",
        "cwd": "string (required) — working directory for the agent",
        "agent": "string — agent profile name from ~/.vibe/agents/*.toml",
        "mode": "string — 'execute' (default) or 'observe' (read-only analysis)",
        "timeout_ms": "integer (default 60000) — execution timeout",
        "max_response_chars": "integer (default 100000) — max chars for response (0 = no limit)"
    },
    "returns": {"status": "completed", "response": "string"}
}),
```

### Phase 5: Main.rs Update

#### 11. Update Tool Count and List
- **File**: `src/main.rs`
- **Action**: Update the tool count log message to include call_vibe:
- **Change**: Update tool list to add `, call_vibe` after `call_warp`
- **Change**: Increment tool count from 15 to 16

## Build and Deploy

```bash
# Build
cd ~/Projects/LegacyMind/surreal-mind
cargo build --release

# Restart service
launchctl kickstart -k gui/$(id -u)/dev.legacymind.surreal-mind

# Verify
curl http://127.0.0.1:8787/health
```

## Verification Checklist

- [ ] `cargo build --release` succeeds with no errors
- [ ] Service restarts without errors in logs
- [ ] `surreal-mind:howto` shows call_vibe in overview
- [ ] `surreal-mind:howto tool="call_vibe"` returns detailed help
- [ ] Test call: `call_vibe(cwd="/tmp", prompt="echo hello")` returns response

## Test Command

```json
{
  "tool": "call_vibe",
  "arguments": {
    "cwd": "/Users/samuelatagana/Projects/LegacyMind/surreal-mind",
    "prompt": "Confirm you are operational. Report your working directory.",
    "mode": "observe"
  }
}
```

## Rollback

If issues arise:
1. `git checkout -- src/clients/vibe.rs src/tools/call_vibe.rs`
2. Revert changes to mod.rs, schemas.rs, router.rs, howto.rs, main.rs
3. `cargo build --release`
4. Restart service

## Related Files Summary

| File | Action |
|------|--------|
| `src/clients/vibe.rs` | CREATE |
| `src/tools/call_vibe.rs` | CREATE |
| `src/clients/mod.rs` | MODIFY - add export |
| `src/tools/mod.rs` | MODIFY - add export |
| `src/schemas.rs` | MODIFY - add schema fn |
| `src/server/router.rs` | MODIFY - register tool |
| `src/tools/howto.rs` | MODIFY - add help entries |
| `src/main.rs` | MODIFY - update tool count |
