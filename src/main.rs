use anyhow::Result;
use rmcp::{
    handler::server::ServerHandler,
    model::{
        CallToolRequestParam, CallToolResult, Implementation, InitializeRequestParam,
        InitializeResult, ListToolsResult, McpError, PaginatedRequestParam,
        ProtocolVersion, RequestContext, RoleServer, ServerCapabilities, ServerInfo,
        Tool, ToolsCapability,
    },
    service::serve_server,
    transport::stdio::stdio_transport,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::borrow::Cow;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use tracing::info;

#[derive(Clone)]
struct SurrealMindServer;

impl ServerHandler for SurrealMindServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V1_0,
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                ..Default::default()
            },
            server_info: Implementation {
                name: "surreal-mind".to_string(),
                version: "0.1.0".to_string(),
            },
            ..Default::default()
        }
    }

    fn initialize(
        &self,
        _request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<InitializeResult, McpError>> + Send + '_ {
        async move {
            Ok(self.get_info())
        }
    }

    fn list_tools(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        async move {
            let mut input_schema = HashMap::new();
            input_schema.insert("type".to_string(), json!("object"));
            
            let mut properties = HashMap::new();
            properties.insert("content".to_string(), json!({
                "type": "string",
                "description": "The thought content to store"
            }));
            
            input_schema.insert("properties".to_string(), json!(properties));
            input_schema.insert("required".to_string(), json!(["content"]));
            
            Ok(ListToolsResult {
                tools: vec![Tool {
                    name: Cow::Borrowed("convo_think"),
                    description: Some(Cow::Borrowed("Store thoughts with memory injection")),
                    input_schema: Arc::new(input_schema),
                    output_schema: None,
                    annotations: None,
                }],
                ..Default::default()
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        async move {
            match request.name.as_str() {
                "convo_think" => {
                    let content = request.arguments
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("empty");
                    
                    info!("convo_think called with: {}", content);
                    
                    Ok(CallToolResult::structured(json!({
                        "thought_id": "test-123",
                        "stored": true,
                        "content": content
                    })))
                }
                _ => Err(McpError {
                    code: -32601,
                    message: format!("Unknown tool: {}", request.name),
                    data: None,
                }),
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("surreal_mind=debug,rmcp=info")
        .init();
    
    info!("Starting Surreal Mind MCP Server");
    
    let server = SurrealMindServer;
    let transport = stdio_transport();
    
    serve_server(server, transport).await?;
    
    Ok(())
}