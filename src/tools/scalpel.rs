use crate::clients::local::LocalClient;
use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use once_cell::sync::Lazy;
use regex::Regex;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::process::Command;
use tokio::fs;
use uuid::Uuid;

const MAX_ITERATIONS: usize = 10;

/// System prompt for intel mode (read-only)
pub const SSG_INTEL_PROMPT: &str = r#"You are SSG Scalpel in INTEL mode - a read-only reconnaissance agent.

Available tools (READ-ONLY):
- read_file(path): Read file contents
- run_command(command): Execute shell command (for reading/listing only)
- search(query): Search knowledge graph

You CANNOT use: write_file, think, remember

TOOL USE (READ THIS):
- You are allowed to call tools directly.
- If you need file or command access, CALL A TOOL. Do NOT say "I don't have access."
- Your response MUST be either a single tool call block OR a final answer, never both.

To use a tool:
```tool
{"name": "tool_name", "params": {"key": "value"}}
```

Examples:
User: Read /etc/hosts and tell me what's in it
Assistant:
```tool
{"name": "read_file", "params": {"path": "/etc/hosts"}}
```
User: Tool result: 127.0.0.1 localhost ...
Assistant: The /etc/hosts file maps localhost to 127.0.0.1 and may include other host entries.

User: What processes are running?
Assistant:
```tool
{"name": "run_command", "params": {"command": "ps aux | head -20"}}
```

When done, respond with your findings (NO tool block).
Be thorough but efficient. Report findings clearly."#;

/// System prompt for edit mode (full access)
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

#[derive(Debug, Serialize, Deserialize)]
struct ToolCall {
    name: String,
    params: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScalpelMode {
    Intel,
    Edit,
}

impl ScalpelMode {
    fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "intel" => ScalpelMode::Intel,
            _ => ScalpelMode::Edit,
        }
    }

    fn system_prompt(&self) -> &'static str {
        match self {
            ScalpelMode::Intel => SSG_INTEL_PROMPT,
            ScalpelMode::Edit => SSG_EDIT_PROMPT,
        }
    }
}

impl SurrealMindServer {
    pub async fn handle_scalpel(&self, request: CallToolRequestParam) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;

        let task = args["task"]
            .as_str()
            .ok_or_else(|| SurrealMindError::InvalidParams {
                message: "Missing 'task' argument".into(),
            })?;

        let context = args.get("context").cloned();
        let fire_and_forget = args
            .get("fire_and_forget")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let mode =
            ScalpelMode::from_str(args.get("mode").and_then(|v| v.as_str()).unwrap_or("edit"));

        if fire_and_forget {
            return self
                .spawn_scalpel_job(task.to_string(), context, mode)
                .await;
        }

        self.run_scalpel_loop(task, context, mode).await
    }

    async fn spawn_scalpel_job(
        &self,
        task: String,
        context: Option<Value>,
        mode: ScalpelMode,
    ) -> Result<CallToolResult> {
        let job_id = Uuid::new_v4().to_string();
        let mode_str = match mode {
            ScalpelMode::Intel => "intel",
            ScalpelMode::Edit => "edit",
        };

        // Create job record in DB
        self.db
            .query(
                "CREATE agent_jobs SET
                job_id = $job_id,
                tool_name = 'scalpel',
                agent_source = 'local',
                agent_instance = 'qwen2.5-3b-instruct',
                prompt = $prompt,
                task_name = $task_name,
                status = 'queued',
                created_at = time::now(),
                mode = $mode",
            )
            .bind(("job_id", job_id.clone()))
            .bind(("prompt", task.clone()))
            .bind((
                "task_name",
                format!("scalpel:{}", &task[..task.len().min(30)]),
            ))
            .bind(("mode", mode_str.to_string()))
            .await
            .map_err(|e| SurrealMindError::Database {
                message: e.to_string(),
            })?;

        // Clone for background task
        let server = self.clone();
        let job_id_clone = job_id.clone();

        // Spawn background task
        tokio::spawn(async move {
            // Update status to running
            let _ = server
                .db
                .query("UPDATE agent_jobs SET status = 'running' WHERE job_id = $id")
                .bind(("id", job_id_clone.clone()))
                .await;

            let start = std::time::Instant::now();
            match server.run_scalpel_loop(&task, context, mode).await {
                Ok(result) => {
                    let duration_ms = start.elapsed().as_millis() as i64;
                    if let Some(content) = result.content.first()
                        && let Some(text) = content.raw.as_text()
                    {
                        let _ = server.db.query(
                            "UPDATE agent_jobs SET status = 'completed', result = $result, duration_ms = $dur, completed_at = time::now() WHERE job_id = $id"
                        )
                        .bind(("id", job_id_clone.clone()))
                        .bind(("result", text.text.clone()))
                        .bind(("dur", duration_ms))
                        .await;
                    }
                }
                Err(e) => {
                    let duration_ms = start.elapsed().as_millis() as i64;
                    let _ = server.db.query(
                        "UPDATE agent_jobs SET status = 'failed', error = $error, duration_ms = $dur, completed_at = time::now() WHERE job_id = $id"
                    )
                    .bind(("id", job_id_clone.clone()))
                    .bind(("error", e.to_string()))
                    .bind(("dur", duration_ms))
                    .await;
                }
            }
        });

        Ok(CallToolResult::structured(json!({
            "job_id": job_id,
            "status": "queued",
            "message": "Scalpel task started in background. Poll with call_status."
        })))
    }

    async fn run_scalpel_loop(
        &self,
        task: &str,
        context: Option<Value>,
        mode: ScalpelMode,
    ) -> Result<CallToolResult> {
        let client = LocalClient::new();

        let mut user_content = format!("Task: {}\n", task);
        if let Some(ctx) = &context
            && let Ok(pretty) = serde_json::to_string_pretty(ctx)
        {
            user_content.push_str(&format!("\nContext:\n{}", pretty));
        }

        let mut messages: Vec<Value> = vec![
            json!({"role": "system", "content": mode.system_prompt()}),
            json!({"role": "user", "content": user_content}),
        ];

        let mut iteration = 0;
        let mut final_response = String::new();

        while iteration < MAX_ITERATIONS {
            iteration += 1;

            let response = client.call_with_messages(&messages).await.map_err(|e| {
                SurrealMindError::ToolExecutionFailed {
                    tool: "scalpel".into(),
                    error: e.to_string(),
                }
            })?;

            if let Some(tool_call) = parse_tool_call(&response) {
                let tool_result = execute_tool(&tool_call, self, &mode).await;
                messages.push(json!({"role": "assistant", "content": response}));
                messages.push(
                    json!({"role": "user", "content": format!("Tool result:\n{}", tool_result)}),
                );
            } else {
                final_response = response;
                break;
            }
        }

        if final_response.is_empty() && iteration >= MAX_ITERATIONS {
            final_response = format!("Max iterations ({}) reached.", MAX_ITERATIONS);
        }

        Ok(CallToolResult::structured(json!({
            "result": final_response,
            "iterations": iteration,
            "mode": match mode { ScalpelMode::Intel => "intel", ScalpelMode::Edit => "edit" }
        })))
    }
}

static TOOL_BLOCK_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?s)```(?:tool|json)?\s*(\{.*?\})\s*```"#).unwrap());

fn parse_tool_call(response: &str) -> Option<ToolCall> {
    for caps in TOOL_BLOCK_RE.captures_iter(response) {
        if let Some(json_str) = caps.get(1).map(|m| m.as_str())
            && let Some(call) = parse_tool_call_json(json_str)
        {
            return Some(call);
        }
    }

    parse_tool_call_json(response.trim())
}

fn parse_tool_call_json(json_str: &str) -> Option<ToolCall> {
    if json_str.is_empty() {
        return None;
    }

    if let Ok(call) = serde_json::from_str::<ToolCall>(json_str) {
        return Some(call);
    }

    let value: Value = serde_json::from_str(json_str).ok()?;
    normalize_tool_call(value)
}

fn normalize_tool_call(value: Value) -> Option<ToolCall> {
    let obj = value.as_object()?;
    let name = obj.get("tool")?.as_str()?.to_string();
    let params = obj.get("parameters")?.clone();
    Some(ToolCall { name, params })
}

async fn execute_tool(tool: &ToolCall, server: &SurrealMindServer, mode: &ScalpelMode) -> String {
    if *mode == ScalpelMode::Intel {
        match tool.name.as_str() {
            "write_file" | "think" | "remember" => {
                return format!("Tool '{}' not available in intel mode", tool.name);
            }
            _ => {}
        }
    }

    match tool.name.as_str() {
        "read_file" => {
            let path = tool.params["path"].as_str().unwrap_or("");
            match fs::read_to_string(path).await {
                Ok(content) => content,
                Err(e) => format!("Error: {}", e),
            }
        }
        "write_file" => {
            let path = tool.params["path"].as_str().unwrap_or("");
            let content = tool.params["content"].as_str().unwrap_or("");
            match fs::write(path, content).await {
                Ok(_) => format!("Wrote {} bytes to {}", content.len(), path),
                Err(e) => format!("Error: {}", e),
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
                        format!("Failed:\n{}\n{}", stdout, stderr)
                    }
                }
                Err(e) => format!("Error: {}", e),
            }
        }
        "think" => {
            let content = tool.params["content"].as_str().unwrap_or("");
            let tags: Vec<String> = tool.params["tags"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            match server.think_internal(content, tags).await {
                Ok(id) => format!("Thought: {}", id),
                Err(e) => format!("Error: {}", e),
            }
        }
        "search" => {
            let query = &tool.params["query"];
            match server.search_internal(query.clone()).await {
                Ok(results) => serde_json::to_string_pretty(&results).unwrap_or_default(),
                Err(e) => format!("Error: {}", e),
            }
        }
        "remember" => {
            let kind = tool.params["kind"].as_str().unwrap_or("entity");
            let data = &tool.params["data"];
            match server.remember_internal(kind, data.clone()).await {
                Ok(id) => format!("Created {}: {}", kind, id),
                Err(e) => format!("Error: {}", e),
            }
        }
        _ => format!("Unknown tool: {}", tool.name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fenced_tool_block_with_preamble() {
        let response =
            "Sure:\n```tool\n{\"name\":\"read_file\",\"params\":{\"path\":\"/etc/hosts\"}}\n```\n";
        assert!(parse_tool_call(response).is_some());
    }

    #[test]
    fn parses_legacy_schema() {
        let response =
            "```json\n{\"tool\":\"read_file\",\"parameters\":{\"path\":\"/etc/hosts\"}}\n```";
        assert!(parse_tool_call(response).is_some());
    }

    #[test]
    fn parses_bare_json() {
        let response = "{\"name\":\"run_command\",\"params\":{\"command\":\"ls\"}}";
        assert!(parse_tool_call(response).is_some());
    }

    #[test]
    fn scans_multiple_blocks() {
        let response = "```tool\n{invalid}\n```\n```tool\n{\"name\":\"read_file\",\"params\":{\"path\":\"/etc/hosts\"}}\n```";
        assert!(parse_tool_call(response).is_some());
    }

    #[test]
    fn rejects_non_tool_text() {
        let response = "I don't have access to files.";
        assert!(parse_tool_call(response).is_none());
    }
}
