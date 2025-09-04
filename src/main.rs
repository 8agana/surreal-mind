use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use surreal_mind::{config::Config, server::SurrealMindServer};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Respect MCP_NO_LOG early to avoid any non‑protocol bytes on stdio
    let no_log = std::env::var("MCP_NO_LOG")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    // Load configuration using the new typed config system
    let config = Config::load().map_err(|e| {
        if !no_log {
            eprintln!("Failed to load configuration: {}", e);
        }
        e
    })?;

    // Initialize tracing unless MCP_NO_LOG is set; default to error-only in stdio mode
    if !config.runtime.mcp_no_log {
        let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "error".to_string());
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .init();

        info!("🚀 Starting Surreal Mind MCP Server with modular architecture");
        info!(
            "📊 Configuration loaded: embedding={}, db={}:{}",
            config.system.embedding_provider, config.system.database_url, config.system.database_ns
        );
    }

    // Create server using the new modular architecture
    let server = SurrealMindServer::new(&config).await.map_err(|e| {
        if !config.runtime.mcp_no_log {
            eprintln!("Failed to create server: {}", e);
        }
        e
    })?;

    if !config.runtime.mcp_no_log {
        info!("✅ Server initialized successfully");
        info!(
            "🛠️  Available tools: think_convo, think_plan, think_debug, think_build, think_stuck, think_search, memories_create, memories_search, memories_moderate, maintenance_ops"
        );
    }

    // Start MCP server with stdio transport
    let service = server.serve(stdio()).await.map_err(|e| {
        if !config.runtime.mcp_no_log {
            eprintln!("Failed to start MCP service: {}", e);
        }
        e
    })?;

    if !config.runtime.mcp_no_log {
        info!("🎯 MCP Server ready - waiting for requests...");
    }
    service.waiting().await?;

    Ok(())
}
