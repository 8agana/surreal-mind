//! HTTP transport module for surreal-mind MCP server
//!
//! Provides Axum-based HTTP server with bearer authentication.
//! Implements Axum-based HTTP server with bearer authentication and MCP over
//! Streamable HTTP. Health, info, and metrics are plain JSON.

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    middleware,
    response::IntoResponse,
    routing::get,
};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
use surreal_mind::{config::Config, error::Result, server::SurrealMindServer};
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager,
    tower::{StreamableHttpServerConfig, StreamableHttpService},
};

/// Session entry for TTL tracking
#[derive(Debug)]
struct SessionEntry {
    created_at: Instant,
    last_active: Instant,
}

/// Shared state for HTTP server
#[derive(Clone)]
pub struct HttpState {
    pub config: Arc<Config>,
    pub sessions: Arc<RwLock<HashMap<String, SessionEntry>>>,
    pub metrics: Arc<Mutex<HttpMetrics>>,
}

/// Metrics for HTTP server
#[derive(Debug, Clone)]
pub struct HttpMetrics {
    pub total_requests: u64,
    pub last_request_unix: u64,
    pub error_count: u64,
}

impl HttpMetrics {
    fn new() -> Self {
        Self {
            total_requests: 0,
            last_request_unix: std::time::SystemTime::UNIX_EPOCH
                .elapsed()
                .unwrap_or_default()
                .as_secs(),
            error_count: 0,
        }
    }
}

// require_bearer implemented as a from_fn_with_state layer below

/// Health check endpoint
pub async fn health_handler() -> impl IntoResponse {
    "ok"
}

/// Info endpoint
pub async fn info_handler(State(state): State<HttpState>) -> impl IntoResponse {
    let embedding = &state.config.system;
    let db = &state.config.system;

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        json!({
            "embedding": {
                "provider": embedding.embedding_provider,
                "model": embedding.embedding_model,
                "dim": embedding.embedding_dimensions
            },
            "db": {
                "url": db.database_url,
                "ns": db.database_ns,
                "db": db.database_db
            },
            "server": {
                "transport": state.config.runtime.transport,
                "bind": state.config.runtime.http_bind.to_string()
            }
        })
        .to_string(),
    )
}

/// Metrics endpoint
pub async fn metrics_handler(State(state): State<HttpState>) -> impl IntoResponse {
    let metrics = state.metrics.lock().await.clone();

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        json!({
            "total_requests": metrics.total_requests,
            "last_request_unix": metrics.last_request_unix
        })
        .to_string(),
    )
}

/// MCP placeholder endpoint
pub async fn mcp_handler() -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        [(header::CONTENT_TYPE, "application/json")],
        json!({"error": "MCP transport not yet implemented"}).to_string(),
    )
}

/// Start the HTTP server
pub async fn start_http_server(server: SurrealMindServer) -> Result<()> {
    // Create HTTP state
    let state = HttpState {
        config: server.config.clone(),
        sessions: Arc::new(RwLock::new(HashMap::new())),
        metrics: Arc::new(Mutex::new(HttpMetrics::new())),
    };

    // Build MCP streamable HTTP service mounted at configured path
    let path = server.config.runtime.http_path.clone();
    let keepalive = Duration::from_secs(server.config.runtime.http_sse_keepalive_sec);
    let session_mgr: LocalSessionManager = Default::default();
    let server_factory = server.clone();
    let mcp_service: StreamableHttpService<SurrealMindServer, _> = StreamableHttpService::new(
        move || Ok(server_factory.clone()),
        Arc::new(session_mgr),
        StreamableHttpServerConfig {
            stateful_mode: true,
            sse_keep_alive: Some(keepalive),
        },
    );

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/info", get(info_handler))
        .route("/metrics", get(metrics_handler))
        .nest_service(path.as_str(), mcp_service)
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any))
        .layer(middleware::from_fn_with_state(
            (state.metrics.clone(), path.clone()),
            |State((metrics, base)): State<(Arc<Mutex<HttpMetrics>>, String)>, req: axum::http::Request<Body>, next: axum::middleware::Next| async move {
                if req.uri().path().starts_with(&base) {
                    let mut m = metrics.lock().await;
                    m.total_requests = m.total_requests.saturating_add(1);
                    m.last_request_unix = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                }
                next.run(req).await
            },
        ))
        // Bearer auth layer with explicit state (token + allow_in_url)
        .layer(middleware::from_fn_with_state(
            (
                server.config.runtime.bearer_token.clone(),
                server.config.runtime.allow_token_in_url,
            ),
            |State((token, allow_q)): State<(Option<String>, bool)>, req: axum::http::Request<Body>, next: axum::middleware::Next| async move {
                // Allow /health without auth
                if req.uri().path() == "/health" {
                    return next.run(req).await;
                }
                let expected = match token {
                    Some(t) => t,
                    None => {
                        return (
                            StatusCode::UNAUTHORIZED,
                            [(header::CONTENT_TYPE, "application/json")],
                            serde_json::json!({"error": {"code": 401, "message": "Unauthorized"}}).to_string(),
                        )
                            .into_response();
                    }
                };
                let headers: &HeaderMap = req.headers();
                let header_ok = headers
                    .get(header::AUTHORIZATION)
                    .and_then(|h| h.to_str().ok())
                    .map(|v| v == format!("Bearer {}", expected))
                    .unwrap_or(false);
                let mut query_ok = false;
                if !header_ok && allow_q {
                    if let Some(q) = req.uri().query() {
                        for pair in q.split('&') {
                            if let Some((k, v)) = pair.split_once('=') {
                                if (k == "access_token" || k == "token") && v == expected {
                                    query_ok = true;
                                    break;
                                }
                            }
                        }
                    }
                }
                if !(header_ok || query_ok) {
                    return (
                        StatusCode::UNAUTHORIZED,
                        [(header::CONTENT_TYPE, "application/json")],
                        serde_json::json!({"error": {"code": 401, "message": "Unauthorized"}}).to_string(),
                    )
                        .into_response();
                }
                next.run(req).await
            },
        ))
        .with_state(state);

    // Start server
    let listener = tokio::net::TcpListener::bind(server.config.runtime.http_bind)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to bind HTTP listener: {}", e))?;

    tracing::info!(
        "Starting HTTP server on {} (MCP at {})",
        server.config.runtime.http_bind,
        server.config.runtime.http_path
    );

    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!("HTTP server error: {}", e))?;

    Ok(())
}
