//! New main.rs demonstrating the modular architecture

use anyhow::Result;
use surreal_mind::server::SurrealMindServer;
use surreal_mind::config::Config;
use rmcp::transport::stdio;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = Config::load()?;

    // Initialize tracing
    let log_level = config.system.database_url; // Using this as example, should use proper log level
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting Surreal Mind MCP Server with modular architecture");

    // Create and start server
    let server = SurrealMindServer::new().await?;
    info!("Server initialized successfully");

    // Start MCP server
    let service = server.serve(stdio()).await?;
    service.waiting().await?;

    Ok(())
}