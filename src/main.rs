use anyhow::{Context, Result};
use rmcp::{ServiceExt, transport::stdio};
mod http;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use surreal_mind::{config::Config, server::SurrealMindServer};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Respect MCP_NO_LOG early to avoid any non‑protocol bytes on stdio
    let no_log = std::env::var("MCP_NO_LOG")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    // Load configuration using the new typed config system
    let config = Config::load()
        .with_context(
            || "Failed to load Surreal Mind configuration from TOML and environment variables",
        )
        .map_err(|e| {
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
    let server = SurrealMindServer::new(&config)
        .await
        .with_context(|| "Failed to initialize SurrealDB connection and SurrealMindServer")
        .map_err(|e| {
            if !config.runtime.mcp_no_log {
                eprintln!("Failed to create server: {}", e);
            }
            e
        })?;

    // Optional startup dim-hygiene preflight (bypassed by SURR_SKIP_DIM_CHECK)
    let skip_dim_check = std::env::var("SURR_SKIP_DIM_CHECK")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);

    if !skip_dim_check
        && (config.runtime.embed_strict
            || std::env::var("SURR_EMBED_STRICT")
                .map(|v| v == "1")
                .unwrap_or(false))
    {
        if let Err(e) = server
            .check_embedding_dims()
            .await
            .with_context(|| "Startup dimension hygiene check failed")
        {
            return Err(anyhow::anyhow!(
                "Embedding dimension mismatch detected: {}",
                e
            ));
        }
    } else if !skip_dim_check && !config.runtime.mcp_no_log {
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
            "🛠️  Loaded 6 MCP tools: legacymind_think, maintenance_ops, memories_create, detailed_help, inner_voice, legacymind_search"
        );
    }

    // Write state.json for stdio session discovery if enabled
    if config.runtime.transport == "stdio"
        && std::env::var("SURR_WRITE_STATE").as_deref() == Ok("1")
        && let Some(data_dir) = dirs::data_dir()
    {
        let state_dir = data_dir.join("surreal-mind");
        if fs::create_dir_all(&state_dir).is_ok() {
            let state_file = state_dir.join("state.json");
            let temp_file = state_dir.join("state.json.tmp");
            let start_unix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let pid = std::process::id();
            let state_data = serde_json::json!({
                "start_unix": start_unix,
                "transport": "stdio",
                "pid": pid,
                "client": std::env::var("MCP_CLIENT").unwrap_or_else(|_| "unknown".to_string()),
                "sessions": 1
            });
            if fs::write(&temp_file, state_data.to_string()).is_ok()
                && fs::rename(&temp_file, &state_file).is_ok()
            {
                let mut perms = if let Ok(meta) = fs::metadata(&state_file) {
                    meta.permissions()
                } else {
                    fs::Permissions::from_mode(0o600)
                };
                perms.set_mode(0o600);
                let _ = fs::set_permissions(&state_file, perms);
            }
        }
    }

    // Check transport selection
    if config.runtime.transport == "http" {
        if !config.runtime.mcp_no_log {
            info!("🌐 Starting HTTP server for MCP transport");
        }
        http::start_http_server(server)
            .await
            .with_context(|| "Failed to start HTTP MCP server")
            .map_err(|e| {
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
        let service = server
            .serve(stdio())
            .await
            .with_context(|| "Failed to start stdio MCP service")
            .map_err(|e| {
                if !config.runtime.mcp_no_log {
                    eprintln!("Failed to start MCP service: {}", e);
                }
                e
            })?;
        service.waiting().await?;
    }

    // Clean up state.json on shutdown
    if config.runtime.transport == "stdio"
        && std::env::var("SURR_WRITE_STATE").as_deref() == Ok("1")
        && let Some(data_dir) = dirs::data_dir()
    {
        let state_file = data_dir.join("surreal-mind").join("state.json");
        let _ = fs::remove_file(state_file);
    }

    Ok(())
}
