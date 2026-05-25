//! Minimal OAuth 2.1 authorization server for MCP remote access.
//!
//! Implements the OAuth 2.1 subset required by the MCP specification
//! (RFC 8414 metadata, RFC 7591 dynamic registration, PKCE) to satisfy
//! claude.ai's MCP connector proxy. Single-user system that auto-approves
//! all authorization requests.

use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Access token lifetime (1 hour). Clients refresh before expiry.
const TOKEN_EXPIRY_SECS: u64 = 3600;

/// Authorization code lifetime (2 minutes).
const CODE_EXPIRY_SECS: u64 = 120;

// ── State ───────────────────────────────────────────────────────────

/// Shared OAuth state. All interior mutability via Arc<Mutex<_>>.
#[derive(Clone)]
pub struct OAuthState {
    /// The bearer token we issue (matches SURR_BEARER_TOKEN so existing
    /// auth middleware accepts it without changes).
    bearer_token: String,
    /// External issuer URL (e.g. "https://mcp.samataganaphotography.com")
    issuer_url: String,
    /// Pre-configured client credentials — the ONLY client allowed to obtain tokens.
    allowed_client_id: String,
    allowed_client_secret: String,
    /// Pending authorization codes
    auth_codes: Arc<Mutex<HashMap<String, AuthCodeEntry>>>,
    /// Active refresh tokens → client_id
    refresh_tokens: Arc<Mutex<HashMap<String, String>>>,
}

struct AuthCodeEntry {
    client_id: String,
    redirect_uri: String,
    code_challenge: Option<String>,
    expires_at: u64,
}

impl OAuthState {
    pub fn new(
        bearer_token: String,
        issuer_url: String,
        client_id: String,
        client_secret: String,
    ) -> Self {
        Self {
            bearer_token,
            issuer_url,
            allowed_client_id: client_id,
            allowed_client_secret: client_secret,
            auth_codes: Arc::new(Mutex::new(HashMap::new())),
            refresh_tokens: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Verify PKCE: SHA-256(code_verifier) base64url-encoded == code_challenge
fn verify_pkce(code_verifier: &str, code_challenge: &str) -> bool {
    let hash = Sha256::digest(code_verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hash) == code_challenge
}

// ── Metadata Discovery (RFC 8414) ───────────────────────────────────

/// GET /.well-known/oauth-authorization-server
async fn metadata_handler(State(st): State<OAuthState>) -> impl IntoResponse {
    let iss = &st.issuer_url;
    (
        StatusCode::OK,
        [("content-type", "application/json")],
        json!({
            "issuer": iss,
            "authorization_endpoint": format!("{}/authorize", iss),
            "token_endpoint": format!("{}/token", iss),
            "registration_endpoint": format!("{}/register", iss),
            "response_types_supported": ["code"],
            "grant_types_supported": [
                "authorization_code",
                "refresh_token",
                "client_credentials"
            ],
            "token_endpoint_auth_methods_supported": [
                "client_secret_post",
                "none"
            ],
            "code_challenge_methods_supported": ["S256"],
            "scopes_supported": ["mcp"]
        })
        .to_string(),
    )
}

// ── Dynamic Client Registration (RFC 7591) ──────────────────────────

#[derive(Deserialize, Default)]
struct RegisterRequest {
    #[serde(default)]
    client_name: Option<String>,
    #[serde(default)]
    redirect_uris: Option<Vec<String>>,
    #[serde(default)]
    grant_types: Option<Vec<String>>,
}

/// POST /register — returns the pre-configured client credentials.
/// Dynamic registration is not supported; this endpoint exists for
/// MCP spec compliance but always returns the same client.
async fn register_handler(State(st): State<OAuthState>, body: String) -> impl IntoResponse {
    let req: RegisterRequest = serde_json::from_str(&body).unwrap_or_default();

    tracing::info!("OAuth: registration request, returning pre-configured client");
    (
        StatusCode::CREATED,
        [("content-type", "application/json")],
        json!({
            "client_id": st.allowed_client_id,
            "client_secret": st.allowed_client_secret,
            "client_name": req.client_name.unwrap_or_else(|| "mcp-client".to_string()),
            "redirect_uris": req.redirect_uris.unwrap_or_default(),
            "grant_types": req.grant_types.unwrap_or_else(|| vec!["authorization_code".to_string()]),
            "token_endpoint_auth_method": "client_secret_post",
        })
        .to_string(),
    )
}

// ── Authorization Endpoint ──────────────────────────────────────────

#[derive(Deserialize)]
struct AuthorizeParams {
    response_type: String,
    client_id: String,
    redirect_uri: String,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    code_challenge: Option<String>,
    #[serde(default)]
    code_challenge_method: Option<String>,
    #[serde(default)]
    scope: Option<String>,
}

/// GET /authorize — auto-approves for the pre-configured client only
async fn authorize_handler(
    State(st): State<OAuthState>,
    Query(params): Query<AuthorizeParams>,
) -> impl IntoResponse {
    let _ = params.code_challenge_method;
    let _ = params.scope;

    if params.response_type != "code" {
        return (
            StatusCode::BAD_REQUEST,
            [("content-type", "application/json")],
            json!({"error": "unsupported_response_type"}).to_string(),
        )
            .into_response();
    }

    // Only the pre-configured client can authorize
    if params.client_id != st.allowed_client_id {
        tracing::warn!("OAuth: rejected unknown client_id {}", params.client_id);
        return (
            StatusCode::FORBIDDEN,
            [("content-type", "application/json")],
            json!({"error": "invalid_client", "error_description": "Unknown client"}).to_string(),
        )
            .into_response();
    }

    let code = Uuid::new_v4().to_string();
    {
        let mut codes = st.auth_codes.lock().await;
        let now = now_unix();
        codes.retain(|_, v| v.expires_at > now);
        codes.insert(
            code.clone(),
            AuthCodeEntry {
                client_id: params.client_id.clone(),
                redirect_uri: params.redirect_uri.clone(),
                code_challenge: params.code_challenge,
                expires_at: now + CODE_EXPIRY_SECS,
            },
        );
    }

    let sep = if params.redirect_uri.contains('?') {
        '&'
    } else {
        '?'
    };
    let mut redirect = format!("{}{}code={}", params.redirect_uri, sep, code);
    if let Some(s) = params.state {
        redirect.push_str(&format!("&state={}", s));
    }

    tracing::info!("OAuth: authorized client {}", params.client_id);
    Redirect::temporary(&redirect).into_response()
}

// ── Token Endpoint ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct TokenRequest {
    grant_type: String,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    redirect_uri: Option<String>,
    #[serde(default)]
    code_verifier: Option<String>,
    #[serde(default)]
    client_id: Option<String>,
    #[serde(default)]
    client_secret: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
}

fn token_ok(
    access_token: &str,
    refresh_token: &str,
) -> (StatusCode, [(&'static str, &'static str); 1], String) {
    (
        StatusCode::OK,
        [("content-type", "application/json")],
        json!({
            "access_token": access_token,
            "token_type": "bearer",
            "expires_in": TOKEN_EXPIRY_SECS,
            "refresh_token": refresh_token,
            "scope": "mcp"
        })
        .to_string(),
    )
}

fn token_err(
    error: &str,
    desc: &str,
) -> (StatusCode, [(&'static str, &'static str); 1], String) {
    (
        StatusCode::BAD_REQUEST,
        [("content-type", "application/json")],
        json!({"error": error, "error_description": desc}).to_string(),
    )
}

/// POST /token — handles authorization_code, refresh_token, client_credentials
async fn token_handler(State(st): State<OAuthState>, body: String) -> impl IntoResponse {
    // OAuth token endpoint accepts application/x-www-form-urlencoded;
    // try form first, then JSON as fallback.
    let params: TokenRequest = match serde_qs::from_str(&body) {
        Ok(p) => p,
        Err(_) => match serde_json::from_str(&body) {
            Ok(p) => p,
            Err(_) => {
                return token_err("invalid_request", "Could not parse request body")
                    .into_response()
            }
        },
    };

    match params.grant_type.as_str() {
        "authorization_code" => handle_auth_code(&st, params).await,
        "refresh_token" => handle_refresh(&st, params).await,
        "client_credentials" => handle_client_creds(&st, params).await,
        _ => token_err("unsupported_grant_type", "Unsupported grant type").into_response(),
    }
}

async fn handle_auth_code(st: &OAuthState, params: TokenRequest) -> axum::response::Response {
    let code = match params.code {
        Some(c) => c,
        None => return token_err("invalid_request", "Missing code").into_response(),
    };

    let entry = {
        let mut codes = st.auth_codes.lock().await;
        codes.remove(&code)
    };
    let entry = match entry {
        Some(e) if e.expires_at > now_unix() => e,
        Some(_) => return token_err("invalid_grant", "Code expired").into_response(),
        None => return token_err("invalid_grant", "Invalid code").into_response(),
    };

    if let Some(ref uri) = params.redirect_uri
        && *uri != entry.redirect_uri
    {
        return token_err("invalid_grant", "redirect_uri mismatch").into_response();
    }

    // PKCE verification
    if let Some(ref challenge) = entry.code_challenge {
        match params.code_verifier {
            Some(ref v) if verify_pkce(v, challenge) => {}
            Some(_) => {
                return token_err("invalid_grant", "PKCE verification failed").into_response()
            }
            None => return token_err("invalid_request", "Missing code_verifier").into_response(),
        }
    }

    let refresh = Uuid::new_v4().to_string();
    {
        let mut tokens = st.refresh_tokens.lock().await;
        tokens.insert(refresh.clone(), entry.client_id.clone());
    }

    tracing::info!(
        "OAuth: issued token via auth_code for client {}",
        entry.client_id
    );
    token_ok(&st.bearer_token, &refresh).into_response()
}

async fn handle_refresh(st: &OAuthState, params: TokenRequest) -> axum::response::Response {
    let rt = match params.refresh_token {
        Some(t) => t,
        None => return token_err("invalid_request", "Missing refresh_token").into_response(),
    };

    let client_id = {
        let tokens = st.refresh_tokens.lock().await;
        tokens.get(&rt).cloned()
    };

    match client_id {
        Some(cid) => {
            // Rotate refresh token
            let new_refresh = Uuid::new_v4().to_string();
            {
                let mut tokens = st.refresh_tokens.lock().await;
                tokens.remove(&rt);
                tokens.insert(new_refresh.clone(), cid.clone());
            }
            tracing::info!("OAuth: refreshed token for client {}", cid);
            token_ok(&st.bearer_token, &new_refresh).into_response()
        }
        None => {
            tracing::warn!("OAuth: rejected invalid refresh token");
            token_err("invalid_grant", "Invalid refresh token").into_response()
        }
    }
}

async fn handle_client_creds(st: &OAuthState, params: TokenRequest) -> axum::response::Response {
    let client_id = match params.client_id {
        Some(id) => id,
        None => return token_err("invalid_request", "Missing client_id").into_response(),
    };

    // Validate against pre-configured credentials
    let valid = client_id == st.allowed_client_id
        && params.client_secret.as_deref() == Some(&st.allowed_client_secret);

    if !valid {
        tracing::warn!("OAuth: rejected client_credentials for {}", client_id);
        return token_err("invalid_client", "Invalid client credentials").into_response();
    }

    let refresh = Uuid::new_v4().to_string();
    {
        let mut tokens = st.refresh_tokens.lock().await;
        tokens.insert(refresh.clone(), client_id.clone());
    }

    tracing::info!("OAuth: client_credentials token for {}", client_id);
    token_ok(&st.bearer_token, &refresh).into_response()
}

// ── Router ──────────────────────────────────────────────────────────

/// Build the OAuth router. State is resolved internally (returns Router<()>).
/// These routes require NO bearer auth — they are how clients GET a token.
pub fn oauth_router(state: OAuthState) -> Router {
    Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            get(metadata_handler),
        )
        .route("/authorize", get(authorize_handler))
        .route("/token", post(token_handler))
        .route("/register", post(register_handler))
        .with_state(state)
}
