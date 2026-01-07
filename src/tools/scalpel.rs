use crate::clients::local::LocalClient;
use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::process::Command;
use tokio::fs;

const MAX_ITERATIONS: usize = 10;

pub const SSG_SYSTEM_PROMPT: &str = r#"You are SSG Scalpel, a precision code executor for the LegacyMind Project.

You have access to these tools:
- read_file(path): Read file contents
- write_file(path, content): Write content to file  
- run_command(command): Execute shell command
- think(content, tags): Record thought to knowledge graph
- search(query): Search knowledge graph
- remember(kind, data): Create entity/relationship/observation

To use a tool, respond with EXACTLY this format:
```tool
{"name": "tool_name", "params": {"param1": "value1"}}
```

After using a tool, you'll receive the result. Continue until the task is complete.
When done, respond with your final answer (no tool block).

Rules:
1. Execute decisively - default to action
2. Stay in scope - no speculative features
3. Verify completion - read files back after writing
4. Shortest path - minimal steps
5. Clear reporting - state what you did"#;

#[derive(Debug, Serialize, Deserialize)]
struct ToolCall {
    name: String,
    params: Value,
}

impl SurrealMindServer {
    pub async fn handle_scalpel(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;

        let task = args["task"].as_str().ok_or_else(|| SurrealMindError::InvalidParams {
            message: "Missing 'task' argument".into(),
        })?;
        
        let context = args.get("context").cloned();
        let client = LocalClient::new();
        
        // Build initial message
        let mut user_content = format!("Task: {}\n", task);
        if let Some(ctx) = &context {
            if let Ok(pretty) = serde_json::to_string_pretty(ctx) {
                user_content.push_str(&format!("\nContext:\n{}", pretty));
            }
        }
        
        let mut messages: Vec<Value> = vec![
            json!({"role": "system", "content": SSG_SYSTEM_PROMPT}),
            json!({"role": "user", "content": user_content}),
        ];
        
        let mut iteration = 0;
        let mut final_response = String::new();
        
        // Agentic loop
        while iteration < MAX_ITERATIONS {
            iteration += 1;
            
            let response = client.call_with_messages(&messages).await
                .map_err(|e| SurrealMindError::ToolExecutionFailed {
                    tool: "scalpel".into(),
                    error: e.to_string(),
                })?;
            
            // Check for tool call
            if let Some(tool_call) = parse_tool_call(&response) {
                let tool_result = execute_tool(&tool_call, self).await;
                
                // Add assistant message and tool result
                messages.push(json!({"role": "assistant", "content": response}));
                messages.push(json!({
                    "role": "user", 
                    "content": format!("Tool result:\n{}", tool_result)
                }));
            } else {
                // No tool call - this is the final response
                final_response = response;
                break;
            }
        }
        
        if final_response.is_empty() && iteration >= MAX_ITERATIONS {
            final_response = format!("Max iterations ({}) reached. Last state may be incomplete.", MAX_ITERATIONS);
        }

        Ok(CallToolResult::structured(json!({
            "result": final_response,
            "iterations": iteration
        })))
    }
}

fn parse_tool_call(response: &str) -> Option<ToolCall> {
    // Look for ```tool ... ``` block
    let tool_start = response.find("```tool")?;
    let json_start = tool_start + 7;
    let json_end = response[json_start..].find("```")?;
    let json_str = response[json_start..json_start + json_end].trim();
    
    serde_json::from_str(json_str).ok()
}

async fn execute_tool(tool: &ToolCall, server: &SurrealMindServer) -> String {
    match tool.name.as_str() {
        "read_file" => {
            let path = tool.params["path"].as_str().unwrap_or("");
            match fs::read_to_string(path).await {
                Ok(content) => content,
                Err(e) => format!("Error reading file: {}", e),
            }
        }
        "write_file" => {
            let path = tool.params["path"].as_str().unwrap_or("");
            let content = tool.params["content"].as_str().unwrap_or("");
            match fs::write(path, content).await {
                Ok(_) => format!("Successfully wrote {} bytes to {}", content.len(), path),
                Err(e) => format!("Error writing file: {}", e),
            }
        }
        "run_command" => {
            let cmd = tool.params["command"].as_str().unwrap_or("");
            match Command::new("sh").arg("-c").arg(cmd).output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if output.status.success() {
                        stdout.to_string()
                    } else {
                        format!("Command failed:\nstdout: {}\nstderr: {}", stdout, stderr)
                    }
                }
                Err(e) => format!("Error executing command: {}", e),
            }
        }
        "think" => {
            let content = tool.params["content"].as_str().unwrap_or("");
            let tags: Vec<String> = tool.params["tags"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            
            // Call internal think handler
            match server.think_internal(content, tags).await {
                Ok(id) => format!("Thought recorded with ID: {}", id),
                Err(e) => format!("Error recording thought: {}", e),
            }
        }
        "search" => {
            let query = &tool.params["query"];
            match server.search_internal(query.clone()).await {
                Ok(results) => serde_json::to_string_pretty(&results).unwrap_or_default(),
                Err(e) => format!("Error searching: {}", e),
            }
        }
        "remember" => {
            let kind = tool.params["kind"].as_str().unwrap_or("entity");
            let data = &tool.params["data"];
            match server.remember_internal(kind, data.clone()).await {
                Ok(id) => format!("Created {} with ID: {}", kind, id),
                Err(e) => format!("Error creating {}: {}", kind, e),
            }
        }
        _ => format!("Unknown tool: {}", tool.name),
    }
}
