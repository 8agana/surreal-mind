use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
mod http;
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

    // Optional startup dim-hygiene preflight
    if config.runtime.embed_strict
        || std::env::var("SURR_EMBED_STRICT")
            .map(|v| v == "1")
            .unwrap_or(false)
    {
        if let Err(e) = server.check_embedding_dims().await {
            return Err(anyhow::anyhow!(
                "Embedding dimension mismatch detected: {}",
                e
            ));
        }
    } else if !config.runtime.mcp_no_log {
        if let Err(e) = server.check_embedding_dims().await {
            tracing::warn!(
                "Embedding dimension hygiene issue detected: {}. Re-embed to fix.",
                e
            );
        } else {
            tracing::debug!("Embedding dimensions are consistent");
        }
    }

    if !config.runtime.mcp_no_log {
        info!("✅ Server initialized successfully");
        info!(
            "🛠️  Available tools: think_convo, think_plan, think_debug, think_build, think_stuck, memories_create, memories_moderate, maintenance_ops, detailed_help, inner_voice, photography_think, photography_memories, legacymind_search, photography_search"
        );
    }

    // Check transport selection
    if config.runtime.transport == "http" {
        if !config.runtime.mcp_no_log {
            info!("🌐 Starting HTTP server for MCP transport");
        }
        http::start_http_server(server).await.map_err(|e| {
            if !config.runtime.mcp_no_log {
                eprintln!("Failed to start HTTP server: {}", e);
            }
            e
        })?;
    } else {
        // Default stdio transport
        if !config.runtime.mcp_no_log {
            info!("🎯 MCP Server ready - waiting for requests...");
        }
        let service = server.serve(stdio()).await.map_err(|e| {
            if !config.runtime.mcp_no_log {
                eprintln!("Failed to start MCP service: {}", e);
            }
            e
        })?;
        service.waiting().await?;
    }

    Ok(())
}
