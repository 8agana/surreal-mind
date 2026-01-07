# Task 34: Scalpel Agentic Tool Access

## Objective
Give Scalpel (local DeepSeek model) access to Desktop Commander tools so it can actually read files, execute commands, etc.

---

## Architecture Options

### Option A: Implement Tool Loop in Rust (Recommended)
**Pros:** Full control, no external dependencies, fast
**Cons:** More code to write

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Primary LLM    │────▶│  scalpel.rs     │────▶│  mistral.rs     │
│  (Opus/Gemini)  │     │  (tool loop)    │     │  (DeepSeek)     │
└─────────────────┘     └────────┬────────┘     └─────────────────┘
                                 │
                        ┌────────▼────────┐
                        │  Tool Executor  │
                        │  (read_file,    │
                        │   write_file,   │
                        │   run_command)  │
                        └─────────────────┘
```

**Implementation:**
1. Define tool schemas (JSON) that DeepSeek can call
2. Parse tool calls from model response
3. Execute tools locally (file I/O, shell commands)
4. Feed results back to model
5. Loop until model says "done" or max iterations

### Option B: Shell to Aider
**Pros:** Already has tool support
**Cons:** External dependency, Python, slower

```bash
aider --model deepseek-coder --no-auto-commits --message "$TASK"
```

### Option C: MCP Client
**Pros:** Reuses existing Desktop Commander
**Cons:** Complexity of MCP client in Rust, circular dependency risk

---

## Recommended Implementation (Option A)

### 1. Tool Definitions
Add to `scalpel.rs`:
```rust
const SCALPEL_TOOLS: &str = r#"[
  // File I/O (Desktop Commander style)
  {"name": "read_file", "description": "Read file contents", "parameters": {"path": "string"}},
  {"name": "write_file", "description": "Write to file", "parameters": {"path": "string", "content": "string"}},
  {"name": "run_command", "description": "Execute shell command", "parameters": {"command": "string"}},
  
  // Surreal Mind (KG access)
  {"name": "think", "description": "Record a thought to the knowledge graph", "parameters": {"content": "string", "tags": "array"}},
  {"name": "search", "description": "Search knowledge graph", "parameters": {"query": "object"}},
  {"name": "remember", "description": "Create entity/relationship/observation", "parameters": {"kind": "string", "data": "object"}}
]"#;
```

### 2. Tool Executor
```rust
async fn execute_tool(name: &str, params: Value) -> Result<String> {
    match name {
        "read_file" => {
            let path = params["path"].as_str()?;
            std::fs::read_to_string(path)
        }
        "write_file" => {
            let path = params["path"].as_str()?;
            let content = params["content"].as_str()?;
            std::fs::write(path, content)?;
            Ok("File written".into())
        }
        "run_command" => {
            let cmd = params["command"].as_str()?;
            let output = std::process::Command::new("sh")
                .arg("-c").arg(cmd).output()?;
            Ok(String::from_utf8_lossy(&output.stdout).into())
        }
        _ => Err("Unknown tool")
    }
}
```

### 3. Agentic Loop
```rust
async fn scalpel_agent(task: &str) -> Result<String> {
    let mut messages = vec![system_prompt, user_task];
    
    for _ in 0..MAX_ITERATIONS {
        let response = call_model(&messages).await?;
        
        if let Some(tool_call) = parse_tool_call(&response) {
            let result = execute_tool(&tool_call.name, tool_call.params).await?;
            messages.push(tool_result(result));
        } else {
            return Ok(response); // Final answer
        }
    }
    Err("Max iterations reached")
}
```

---

## Acceptance Criteria
- [ ] Scalpel can read files when asked
- [ ] Scalpel can write files when asked
- [ ] Scalpel can run shell commands
- [ ] Scalpel can call `think` to record thoughts
- [ ] Scalpel can call `search` to query KG
- [ ] Scalpel can call `remember` to create entities
- [ ] Agentic loop terminates correctly
- [ ] Max iterations prevent infinite loops
