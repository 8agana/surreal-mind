# Task 36: Scalpel Fire-and-Forget + Intel/Edit Modes

## Objectives

### 1. Fire-and-Forget Mode
Add `fire_and_forget: bool` parameter (default: false).
- When true: spawn agentic loop in background, return job ID immediately
- Poll status via `call_status` like `call_gem`
- Reuse existing job queue infrastructure

### 2. Intel vs Edit Modes
Add `mode: "intel" | "edit"` parameter (default: "edit").

**Intel mode (read-only):**
- Tools: `read_file`, `run_command` (non-destructive), `search`
- No: `write_file`, `think`, `remember`

**Edit mode (full access):**
- All tools available

## Implementation

### Schema Updates (`schemas.rs`)
```rust
"fire_and_forget": {"type": "boolean", "default": false},
"mode": {"type": "string", "enum": ["intel", "edit"], "default": "edit"}
```

### Handler Updates (`scalpel.rs`)
1. Parse new parameters
2. If `fire_and_forget`: spawn task, store job, return ID
3. Pass `mode` to `execute_tool` to filter available tools

### Tool Filtering
```rust
fn execute_tool(tool: &ToolCall, server: &Server, mode: &str) -> String {
    if mode == "intel" {
        match tool.name.as_str() {
            "write_file" | "think" | "remember" => {
                return "Tool not available in intel mode".to_string();
            }
            _ => {}
        }
    }
    // ... existing execution
}
```

## Acceptance Criteria
- [ ] `fire_and_forget: true` returns job ID immediately
- [ ] Job status pollable via `call_status`
- [ ] `mode: "intel"` blocks write operations
- [ ] `mode: "edit"` allows all operations
- [ ] Documentation updated
