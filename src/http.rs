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
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager,
    tower::{StreamableHttpServerConfig, StreamableHttpService},
};
use serde_json::json;
use std::{cmp::Ordering, sync::Arc, time::Duration};
use surreal_mind::{config::Config, error::Result, server::SurrealMindServer};
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};

/// Shared state for HTTP server
#[derive(Clone)]
pub struct HttpState {
    pub config: Arc<Config>,
    pub metrics: Arc<Mutex<HttpMetrics>>,
    pub session_mgr: Arc<LocalSessionManager>,
    pub db_ping_cache: Arc<Mutex<Option<(u64, u64)>>>,
}

/// Metrics for HTTP server
#[derive(Debug, Clone)]
pub struct HttpMetrics {
    pub total_requests: u64,
    pub last_request_unix: u64,

    pub http_total_sessions: u64,
    pub errors_total: u64,
    pub latencies: Vec<f64>, // ring buffer for p95
    pub tools_count: std::collections::HashMap<String, u64>,
}

impl HttpMetrics {
    fn new() -> Self {
        Self {
            total_requests: 0,
            last_request_unix: std::time::SystemTime::UNIX_EPOCH
                .elapsed()
                .unwrap_or_default()
                .as_secs(),
            http_total_sessions: 0,
            errors_total: 0,
            latencies: Vec::with_capacity(256),
            tools_count: std::collections::HashMap::new(),
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

    // Check DB connection and ping
    let (db_connected, db_ping_ms) = if state.config.system.database_url.is_empty() {
        (false, None)
    } else {
        // Check cache
        let ttl_ms = std::env::var("SURR_DB_PING_TTL_MS")
            .unwrap_or_else(|_| "1500".to_string())
            .parse::<u64>()
            .unwrap_or(1500);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let cache = state.db_ping_cache.lock().await;
        if let Some((ts, ping)) = *cache {
            if now.saturating_sub(ts) < ttl_ms {
                (true, Some(ping))
            } else {
                // Cache expired, ping again
                drop(cache);
                let ping_result = ping_db(&state).await;
                let mut cache = state.db_ping_cache.lock().await;
                if let Some(p) = ping_result {
                    *cache = Some((now, p));
                    (true, Some(p))
                } else {
                    (false, None)
                }
            }
        } else {
            // No cache, ping
            let ping_result = ping_db(&state).await;
            let mut cache = state.db_ping_cache.lock().await;
            if let Some(p) = ping_result {
                *cache = Some((now, p));
                (true, Some(p))
            } else {
                (false, None)
            }
        }
    };

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
                "db": db.database_db,
                "connected": db_connected,
                "ping_ms": db_ping_ms
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
    // Read active sessions from session manager
    let active_sessions = state.session_mgr.sessions.read().await.len();

    // Compute latency stats
    let (avg_latency_ms, p95_latency_ms) = if metrics.latencies.is_empty() {
        (None, None)
    } else {
        let sum: f64 = metrics.latencies.iter().sum();
        let avg = sum / metrics.latencies.len() as f64;
        let mut sorted = metrics.latencies.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
        let p95_idx = (sorted.len() as f64 * 0.95) as usize;
        let p95 = sorted.get(p95_idx).copied();
        (Some(avg), p95)
    };

    // Top 5 tools
    let mut tools_vec: Vec<_> = metrics.tools_count.iter().collect();
    tools_vec.sort_by(|a, b| b.1.cmp(a.1));
    let tools_top_5: Vec<_> = tools_vec
        .into_iter()
        .take(5)
        .map(|(k, v)| json!({ "name": k, "count": v }))
        .collect();

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        json!({
            "metrics_version": "1",
            "total_requests": metrics.total_requests,
            "last_request_unix": metrics.last_request_unix,
            "http_active_sessions": active_sessions,
            "http_total_sessions": metrics.http_total_sessions,
            "errors_total": metrics.errors_total,
            "avg_latency_ms": avg_latency_ms,
            "p95_latency_ms": p95_latency_ms,
            "tools_top_5": tools_top_5
        })
        .to_string(),
    )
}

/// DB health endpoint (optional, gated by SURR_DB_STATS=1)
pub async fn db_health_handler(State(state): State<HttpState>) -> impl IntoResponse {
    if std::env::var("SURR_DB_STATS").ok().as_deref() != Some("1") {
        return (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "application/json")],
            json!({"error": "DB stats not enabled"}).to_string(),
        );
    }

    let (connected, ping_ms, thoughts_count, recalls_count) =
        if state.config.system.database_url.is_empty() {
            (false, None, None, None)
        } else {
            // Check cache or ping
            let ttl_ms = std::env::var("SURR_DB_PING_TTL_MS")
                .unwrap_or_else(|_| "1500".to_string())
                .parse::<u64>()
                .unwrap_or(1500);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let cache = state.db_ping_cache.lock().await;
            let (ping_ms, _is_cached) = if let Some((ts, p)) = *cache {
                if now.saturating_sub(ts) < ttl_ms {
                    (Some(p), true)
                } else {
                    (None, false)
                }
            } else {
                (None, false)
            };
            drop(cache);

            let ping_result = if ping_ms.is_some() {
                ping_ms
            } else {
                ping_db(&state).await
            };

            let counts = if ping_result.is_some() {
                get_db_counts(&state).await
            } else {
                (None, None)
            };

            (ping_result.is_some(), ping_result, counts.0, counts.1)
        };

    // Optional photography DB health (if enabled)
    let mut photo = serde_json::Value::Null;
    if state.config.runtime.photo_enable {
        let purl = state
            .config
            .runtime
            .photo_url
            .clone()
            .unwrap_or_else(|| state.config.system.database_url.clone());
        if let (Some(ns), Some(db_name)) = (
            state.config.runtime.photo_ns.clone(),
            state.config.runtime.photo_db.clone(),
        ) {
            let (p_connected, p_ping_ms) = if let Some(p) = ping_db_params(
                &purl,
                &ns,
                &db_name,
                state.config.runtime.photo_user.as_deref().unwrap_or(""),
                state.config.runtime.photo_pass.as_deref().unwrap_or(""),
            )
            .await
            {
                (true, Some(p))
            } else {
                (false, None)
            };
            let (p_thoughts, p_entities) = if p_connected {
                get_db_counts_params(
                    &purl,
                    &ns,
                    &db_name,
                    state.config.runtime.photo_user.as_deref().unwrap_or(""),
                    state.config.runtime.photo_pass.as_deref().unwrap_or(""),
                )
                .await
            } else {
                (None, None)
            };
            photo = json!({
                "ns": ns,
                "db": db_name,
                "connected": p_connected,
                "ping_ms": p_ping_ms,
                "thoughts_count": p_thoughts,
                "entities_count": p_entities
            });
        }
    }

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        json!({
            "connected": connected,
            "ping_ms": ping_ms,
            "ns": state.config.system.database_ns,
            "db": state.config.system.database_db,
            "thoughts_count": thoughts_count,
            "recalls_count": recalls_count,
            "photography": photo
        })
        .to_string(),
    )
}

/// Start the HTTP server
pub async fn start_http_server(server: SurrealMindServer) -> Result<()> {
    // Warn on insecure token usage
    if server.config.runtime.allow_token_in_url {
        tracing::warn!(
            "Token authentication via query parameters is enabled; this can leak tokens in logs/proxies. Consider using Authorization header instead."
        );
    }

    // Create HTTP state
    let session_mgr = Arc::new(LocalSessionManager::default());
    let state = HttpState {
        config: server.config.clone(),
        metrics: Arc::new(Mutex::new(HttpMetrics::new())),
        session_mgr: session_mgr.clone(),
        db_ping_cache: Arc::new(Mutex::new(None)),
    };

    // Build MCP streamable HTTP service mounted at configured path
    let path = server.config.runtime.http_path.clone();
    let keepalive = Duration::from_secs(server.config.runtime.http_sse_keepalive_sec);
    let server_factory = server.clone();
    let mcp_service: StreamableHttpService<SurrealMindServer, _> = StreamableHttpService::new(
        move || Ok(server_factory.clone()),
        session_mgr.clone(),
        StreamableHttpServerConfig {
            stateful_mode: true,
            sse_keep_alive: Some(keepalive),
        },
    );

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/info", get(info_handler))
        .route("/metrics", get(metrics_handler))
        .route("/db_health", get(db_health_handler))
        .nest_service(path.as_str(), mcp_service)
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any))
        .layer(middleware::from_fn_with_state(
            (state.metrics.clone(), path.clone()),
            |State((metrics, base)): State<(Arc<Mutex<HttpMetrics>>, String)>,
             req: axum::http::Request<Body>,
             next: axum::middleware::Next| async move {
                let is_mcp = req.uri().path().starts_with(&base);
                let start = if is_mcp {
                    Some(std::time::Instant::now())
                } else {
                    None
                };
                let resp = next.run(req).await;
                if let Some(start_time) = start {
                    let latency_ms = start_time.elapsed().as_millis() as f64;
                    let mut m = metrics.lock().await;
                    if latency_ms > 0.0 {
                        m.latencies.push(latency_ms);
                        if m.latencies.len() > 256 {
                            m.latencies.remove(0);
                        }
                    }
                    if !resp.status().is_success() {
                        m.errors_total = m.errors_total.saturating_add(1);
                    }
                    m.total_requests = m.total_requests.saturating_add(1);
                    m.last_request_unix = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                }
                resp
            },
        ))
        // Bearer auth layer with explicit state (token + allow_in_url)
        .layer(middleware::from_fn_with_state(
            (
                server.config.runtime.bearer_token.clone(),
                server.config.runtime.allow_token_in_url,
            ),
            |State((token, allow_q)): State<(Option<String>, bool)>,
             req: axum::http::Request<Body>,
             next: axum::middleware::Next| async move {
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
                            serde_json::json!({"error": {"code": 401, "message": "Unauthorized"}})
                                .to_string(),
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
                        serde_json::json!({"error": {"code": 401, "message": "Unauthorized"}})
                            .to_string(),
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

async fn ping_db(state: &HttpState) -> Option<u64> {
    ping_db_params(
        &state.config.system.database_url,
        &state.config.system.database_ns,
        &state.config.system.database_db,
        &state.config.runtime.database_user,
        &state.config.runtime.database_pass,
    )
    .await
}

async fn ping_db_params(url: &str, ns: &str, db: &str, user: &str, pass: &str) -> Option<u64> {
    let timeout_ms = std::env::var("SURR_DB_PING_TIMEOUT_MS")
        .unwrap_or_else(|_| "250".to_string())
        .parse::<u64>()
        .unwrap_or(250);
    let timeout = Duration::from_millis(timeout_ms);

    // Create a temporary DB connection
    let db_result =
        surrealdb::Surreal::new::<surrealdb::engine::remote::ws::Ws>(url.to_string()).await;
    let client = match db_result {
        Ok(db) => db,
        Err(_) => return None,
    };

    // Authenticate if credentials are provided (root auth)
    if !user.is_empty() {
        let _ = client
            .signin(surrealdb::opt::auth::Root {
                username: user,
                password: pass,
            })
            .await
            .ok()?;
    }

    // Switch NS/DB
    client.use_ns(ns).use_db(db).await.ok()?;

    // Ping with SELECT 1
    let start = std::time::Instant::now();
    let query_result = tokio::time::timeout(timeout, client.query("SELECT 1")).await;
    let elapsed = start.elapsed().as_millis() as u64;
    match query_result {
        Ok(Ok(_)) => Some(elapsed),
        _ => None,
    }
}

async fn get_db_counts(state: &HttpState) -> (Option<u64>, Option<u64>) {
    get_db_counts_params(
        &state.config.system.database_url,
        &state.config.system.database_ns,
        &state.config.system.database_db,
        &state.config.runtime.database_user,
        &state.config.runtime.database_pass,
    )
    .await
}

async fn get_db_counts_params(
    url: &str,
    ns: &str,
    db: &str,
    user: &str,
    pass: &str,
) -> (Option<u64>, Option<u64>) {
    let timeout = Duration::from_millis(500);
    let db_result =
        surrealdb::Surreal::new::<surrealdb::engine::remote::ws::Ws>(url.to_string()).await;
    let client = match db_result {
        Ok(db) => db,
        Err(_) => return (None, None),
    };
    if !user.is_empty()
        && client
            .signin(surrealdb::opt::auth::Root {
                username: user,
                password: pass,
            })
            .await
            .is_err()
    {
        return (None, None);
    }
    if client.use_ns(ns).use_db(db).await.is_err() {
        return (None, None);
    }

    let thoughts_query = tokio::time::timeout(
        timeout,
        client.query("SELECT count() FROM thoughts LIMIT 100000"),
    )
    .await;
    let recalls_query = tokio::time::timeout(
        timeout,
        client.query("SELECT count() FROM kg_entities LIMIT 100000"),
    )
    .await;

    let thoughts_count = if let Ok(Ok(mut resp)) = thoughts_query {
        resp.take::<Option<u64>>(0).ok().flatten()
    } else {
        None
    };
    let recalls_count = if let Ok(Ok(mut resp)) = recalls_query {
        resp.take::<Option<u64>>(0).ok().flatten()
    } else {
        None
    };
    (thoughts_count, recalls_count)
}
