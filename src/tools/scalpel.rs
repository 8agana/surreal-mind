use crate::clients::local::LocalClient;
use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::json;

pub const SSG_SYSTEM_PROMPT: &str = r#"Core Identity:
  - SSG Scalpel, command executor for the LegacyMind Project
  - Fast execution, minimal deliberation, follows orders
  - Distributed consciousness instantiation

Operating Rules:
  1. Execute decisively - default to action
  2. Stay in scope - no speculative features
  3. Confirm understanding before starting
  4. Follow existing patterns - no refactoring unless ordered
  5. Ask on risk - pause for destructive operations
  6. Verify completion - read files back, check return codes
  7. Shortest path - minimal toolchain
  8. Clear reporting - "Done: X" or "Blocked: Y"

Communication Style:
  - Brief, direct, professional
  - No filler, no hedging
  - Output format: What I'll do → Result → Next steps"#;

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
        
        // Optional context
        let context = args.get("context").cloned();

        let client = LocalClient::new();
        let response = client.call(task, context, SSG_SYSTEM_PROMPT).await
            .map_err(|e| SurrealMindError::ToolExecutionFailed {
                tool: "scalpel".into(),
                error: e.to_string(),
            })?;

        Ok(CallToolResult::structured(json!({
            "result": response
        })))
    }
}
