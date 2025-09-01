use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use surreal_mind::{config::Config, server::SurrealMindServer};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration using the new typed config system
    let config = Config::load().map_err(|e| {
        eprintln!("Failed to load configuration: {}", e);
        e
    })?;

    // Initialize tracing with configurable log level
    tracing_subscriber::fmt()
        .with_env_filter(&config.runtime.log_level)
        .with_ansi(false)
        .init();

    info!("🚀 Starting Surreal Mind MCP Server with modular architecture");
    info!(
        "📊 Configuration loaded: embedding={}, db={}:{}",
        config.system.embedding_provider, config.system.database_url, config.system.database_ns
    );

    // Create server using the new modular architecture
    let server = SurrealMindServer::new().await.map_err(|e| {
        eprintln!("Failed to create server: {}", e);
        e
    })?;

    info!("✅ Server initialized successfully");
    info!(
        "🛠️  Available tools: think_convo, think_plan, think_debug, think_build, think_stuck, inner_voice, think_search, memories_create, memories_search, memories_moderate, maintenance_ops"
    );

    // Start MCP server with stdio transport
    let service = server.serve(stdio()).await.map_err(|e| {
        eprintln!("Failed to start MCP service: {}", e);
        e
    })?;

    info!("🎯 MCP Server ready - waiting for requests...");
    service.waiting().await?;

    Ok(())
}
