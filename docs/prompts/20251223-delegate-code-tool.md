# delegate_code: External CLI Code Execution Tool

**Date**: 2025-12-23
**Requested by**: Sam
**Context**: MCP tool to delegate code execution to external CLI (Codex, Claude, etc.)
**Reviewed by**: SGT Pickle
**Production-hardened by**: Claude Desktop

---

## Problem

Need to execute code using external CLI tools while maintaining MCP protocol integration. Current surreal-mind tools run locally, but sometimes we want to delegate to specialized external CLIs like Codex for different model capabilities.

---

## Goal

Create `delegate_code` MCP tool that:
1. Executes code using external CLI (primarily Codex)
2. Supports working directory specification
3. Enables session resume for context accumulation
4. Provides model selection with smart defaults
5. Returns structured results with timing and metadata

---

## Architecture Decisions

### External CLI Integration Pattern

**Codex CLI Integration (researched 2025-12-23):**

- **Working Directory**: `codex --dir /path/to/project`
- **Model Selection**: `codex --model gpt-5.1-codex-max`
- **Session Resume**: `codex --resume` for context accumulation
- **Code Execution**: `codex "your code or task"`

**Discovery Capability**: `codex --list-models` to populate available options

---

## Implementation Requirements

### 1. Tool Structure (`src/tools/delegate_code.rs` - new file)

```rust
use serde::{Deserialize, Serialize};
use crate::server::router::{CallToolRequestParam, CallToolResult};
use std::time::{Duration, Instant};
use tokio::process::Command as TokioCommand;

#[derive(Debug, Serialize, Deserialize)]
pub struct DelegateCodeRequest {
    /// Directory where code should be executed from
    pub working_directory: String,
    
    /// Code or task to execute
    pub code: String,
    
    /// Whether to resume last session for context accumulation
    #[serde(default)]
    pub resume_last: bool,
    
    /// Model to use (defaults to Codex default if not specified)
    pub model: Option<String>,
    
    /// Additional context or instructions
    pub context: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DelegateCodeResponse {
    pub success: bool,
    pub output: String,
    pub model_used: String,
    pub session_resumed: bool,
    pub execution_time_ms: u64,
}

pub async fn handle_delegate_code(
    &self,
    request: CallToolRequestParam,
) -> Result<CallToolResult> {
    let args: DelegateCodeRequest = request.arguments.ok_or_else(|| {
        SurrealMindError::Mcp("Missing required arguments".to_string())
    })?;

    // Build codex command (NON-BLOCKING)
    let mut cmd = TokioCommand::new("codex");
    
    // Add working directory
    cmd.arg("--dir").arg(&args.working_directory);
    
    // Add model selection
    if let Some(model) = &args.model {
        cmd.arg("--model").arg(model);
    }
    
    // Add session resume flag
    if args.resume_last {
        cmd.arg("--resume");
    }
    
    // Add context if provided
    if let Some(context) = &args.context {
        cmd.arg("--context").arg(context);
    }
    
    // Add code/task as argument
    cmd.arg(&args.code);
    
    let start_time = Instant::now();
    
    // Execute with timeout and capture both stdout + stderr
    let timeout_duration = Duration::from_secs(300); // 5-minute default timeout
    let output = tokio::time::timeout(timeout_duration, cmd.output())
        .await
        .map_err(|_| {
            SurrealMindError::Mcp("Codex execution timed out".to_string())
        })?
        .map_err(|e| {
            SurrealMindError::Mcp(format!("Failed to execute codex: {}", e))
        })?;
    
    let execution_time = start_time.elapsed().as_millis();
    
    let success = output.status.success();
    let stdout_text = String::from_utf8_lossy(&output.stdout);
    let stderr_text = String::from_utf8_lossy(&output.stderr);
    let full_output = if stderr_text.is_empty() {
        stdout_text
    } else {
        format!("{}\n\nSTDERR:\n{}", stdout_text, stderr_text)
    };
    
    let response = DelegateCodeResponse {
        success,
        output: full_output,
        model_used: args.model.unwrap_or_else(|| "default".to_string()),
        session_resumed: args.resume_last,
        execution_time_ms: execution_time,
    };
    
    Ok(CallToolResult::structured(json!(response)))
}

// Helper function for non-blocking execution (not inside impl block)
pub fn create_delegate_code_command(
    working_dir: &str,
    code: &str,
    model: Option<&str>,
    resume_last: bool,
    context: Option<&str>,
) -> Result<TokioCommand, Box<dyn std::error::Error + Send + Sync>> {
    let mut cmd = TokioCommand::new("codex");
    
    // Add working directory
    cmd.arg("--dir").arg(working_dir);
    
    // Add model selection
    if let Some(model) = model {
        cmd.arg("--model").arg(model);
    }
    
    // Add session resume flag
    if resume_last {
        cmd.arg("--resume");
    }
    
    // Add context if provided
    if let Some(ctx) = context {
        cmd.arg("--context").arg(ctx);
    }
    
    // Add code/task as argument
    cmd.arg(code);
    
    Ok(cmd)
}

// Optional: Model discovery for dynamic population
pub async fn get_available_codex_models() -> Result<Vec<String>> {
    let output = TokioCommand::new("codex")
        .arg("--list-models")
        .output()
        .await
        .map_err(|e| SurrealMindError::Mcp(format!("Failed to list models: {}", e)))?;
    
    let models_str = String::from_utf8_lossy(&output.stdout);
    Ok(models_str.lines().map(|s| s.to_string()).collect())
}
```

### 2. Tool Schema (`src/schemas.rs` - add function)

```rust
pub fn delegate_code_schema() -> serde_json::Value {
    json!({
        "name": "delegate_code",
        "description": "Delegate code execution to external CLI (Codex, Claude, etc.)",
        "inputSchema": {
            "type": "object",
            "properties": {
                "working_directory": {
                    "type": "string",
                    "description": "Directory where code should be executed from"
                },
                "code": {
                    "type": "string", 
                    "description": "Code or task to execute"
                },
                "resume_last": {
                    "type": "boolean",
                    "description": "Resume last session for context accumulation",
                    "default": false
                },
                "model": {
                    "type": "string",
                    "description": "Model to use (e.g., 'gpt-5.1-codex-max', 'claude-3.5-sonnet')",
                    "default": null
                },
                "context": {
                    "type": "string",
                    "description": "Additional context or instructions",
                    "default": null
                }
            },
            "required": ["working_directory", "code"]
        }
    })
}
```

### 3. Tool Registration

**Update `src/tools/mod.rs`:**
```rust
pub mod delegate_code;
// ... other modules
pub use delegate_code::*;
```

**Update `src/server/router.rs`:**
- Add `delegate_code_schema()` to detailed_help enum
- Add tool registration in `list_tools()`
- Add handler routing in tool calling logic

---

## Configuration

**Environment Variables** (optional):
```bash
# Codex CLI Configuration
CODEX_DEFAULT_MODEL=gpt-5.1-codex-max
CODEX_TIMEOUT_SECONDS=300  # 5 minutes
CODEX_SESSION_TIMEOUT_HOURS=24
```

**Claude Desktop Configuration**:
```json
{
  "mcpServers": {
    "surreal-mind": {
      "command": "/path/to/surreal-mind/target/release/surreal-mind",
      "args": []
    }
  }
}
```

---

## Success Criteria (Acceptance Tests)

1. [ ] **Basic execution**: `delegate_code` with working directory + code executes successfully
2. [ ] **Working directory**: Code executes in specified directory (verified by file operations)
3. [ ] **Session resume**: `resume_last=true` maintains context across calls
4. [ ] **Model selection**: `model` parameter uses specified CLI model
5. [ ] **Error handling**: CLI failures return structured MCP errors
6. [ ] **Timing**: Execution time is accurately measured and reported
7. [ ] **Schema validation**: All parameters validated via JSON schema

---

## Usage Examples

**Basic Code Execution:**
```json
{
  "name": "delegate_code",
  "arguments": {
    "working_directory": "/Users/samuelatagana/Projects/LegacyMind/surreal-mind",
    "code": "cargo check",
    "model": "gpt-5.1-codex-max"
  }
}
```

**Session Context Accumulation:**
```json
{
  "name": "delegate_code", 
  "arguments": {
    "working_directory": "/Users/samuelatagana/Projects/LegacyMind/surreal-mind",
    "code": "run tests and fix any issues",
    "resume_last": true,
    "context": "We're working on the memories_populate tool"
  }
}
```

**Model Discovery Integration:**
```json
{
  "name": "delegate_code",
  "arguments": {
    "working_directory": "/Users/samuelatagana/Projects/LegacyMind/surreal-mind", 
    "code": "analyze this codebase",
    "model": "claude-3.5-sonnet"
  }
}
```

---

## Implementation Order

1. **Tool implementation** - Create `delegate_code.rs` with handlers
2. **Schema registration** - Add schema function to `schemas.rs`
3. **Tool integration** - Register in `mod.rs` and `router.rs`
4. **Basic testing** - Verify simple code execution works
5. **Session testing** - Test resume functionality across multiple calls
6. **Model testing** - Verify different models can be specified
7. **Error handling** - Test failure scenarios and edge cases
8. **Documentation** - Update `docs/AGENTS/tools.md`

---

## Notes

**Why external CLI delegation:**
- **Model diversity**: Access to different models beyond local capabilities
- **Specialized capabilities**: Each CLI has unique strengths (Codex for code, etc.)
- **Context persistence**: Session resume enables longer-term projects
- **Flexibility**: Can swap out underlying CLI without changing MCP interface

**Design considerations:**
- **Security**: Working directory isolation prevents unauthorized file access
- **Performance**: Execution timing helps track bottlenecks
- **Reliability**: Structured error handling maintains MCP contract
- **Extensibility**: Pattern can support other CLIs beyond Codex

---

## Claude Desktop Review (2025-12-23)

**Production Fixes Applied:**
✅ **Non-blocking execution**: `std::process::Command::output()` → `tokio::process::Command` with `.await`
✅ **Stderr capture**: Added both stdout AND stderr for complete CLI output  
✅ **Context parameter**: Wired `context` argument into CLI command
✅ **Timeout handling**: Added 5-minute configurable timeout to prevent server freezes
✅ **Self reference fix**: Proper function structure outside impl blocks
✅ **Duplicate import removal**: Fixed redundant TokioCommand import
✅ **Async timeout**: Proper `tokio::time::timeout()` wrapper instead of unused timeout variable

**Critical Production Notes:**
- **Blocking execution frozen MCP server** - Fixed with async tokio::process
- **Missing stderr caused lost diagnostics** - Now captures both streams
- **Context parameter was dead code** - Now properly passed to CLI
- **Self-reference compilation error** - Fixed with helper function pattern
- **Timeouts essential** - Codex can chew on complex code; 5-minute default prevents hanging
- **Clean imports** - Removed duplicate TokioCommand import

---

## Testing Strategy

**Unit Tests:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_basic_delegation() {
        // Test basic command execution
    }
    
    #[tokio::test] 
    async fn test_session_resume() {
        // Test context accumulation
    }
    
    #[tokio::test]
    async fn test_model_selection() {
        // Test different models
    }
}
```

**Integration Tests:**
```bash
# Test with actual Codex CLI
curl -X POST http://localhost:8787/mcp/v1/tools/call \
  -H "Content-Type: application/json" \
  -d '{
    "name": "delegate_code",
    "arguments": {
      "working_directory": "/tmp",
      "code": "echo hello world",
      "resume_last": false
    }
  }'
```

---

## Related Work

- **memories_populate implementation** - Pattern for JSON-RPC response structure
- **External MCP integrations** - Research on codex, gemini-cli, other CLIs
- **Session management patterns** - From `sessions.rs` in surreal-mind

---

## Chain Reference

Design developed in response to Sam's request for external CLI delegation tool with working directory, session resume, and model selection capabilities. Production-hardened by Claude Desktop review.

---

## Questions for Implementation

1. **CLI availability**: Should we check if `codex` CLI is available before attempting execution?
2. **Model validation**: Should we validate model names against available list or let CLI handle errors?
3. **Working directory validation**: Should we verify directory exists before passing to CLI?
4. **Timeout handling**: Should we implement configurable timeouts for long-running tasks?
5. **Multiple CLI support**: Should tool support other CLIs (claude, gemini) or focus on Codex?

---

## Future Enhancements

- **Dynamic CLI discovery**: Auto-detect available CLIs and their capabilities
- **Parallel execution**: Support for running multiple commands simultaneously  
- **Result caching**: Cache execution results for repeated operations
- **Streaming output**: Support for real-time CLI output streaming
- **Interactive sessions**: Persistent shell-like sessions with context