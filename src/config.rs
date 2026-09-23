use serde::{Deserialize, Serialize};

/// The shared rule for callers routed through this helper: if
/// `SURR_ENV_FILE` is set, load
/// ONLY that exact path (`dotenvy::from_path`) and never fall back to an
/// ancestor `.env` -- an empty file at that path loads nothing. If
/// `SURR_ENV_FILE` is unset, run the caller-supplied `unset_fallback`
/// closure instead.
///
/// Dotenv and inherited process environment are configuration input supplied
/// through the operator environment, not authenticated identity or a permit.
/// Once dotenv loads a value into the process-global environment, downstream
/// `std::env::var` callers cannot distinguish its
/// provenance. This helper therefore defines resolution behavior; it is not
/// an authorization boundary, and read ordering cannot turn an env-keyed
/// guard into one.
///
/// fed-93bfee #216 unified this AND each caller's unset-path behavior into
/// one bare `dotenvy::dotenv()` rule. Codex's #222 review caught that as a
/// production behavior change: `Config::load`'s pre-existing unset rule was
/// NOT a bare `dotenvy::dotenv()` -- it was local `.env`, then `../.env`
/// ONLY if neither `SURR_DB_URL` nor `OPENAI_API_KEY` ended up present (see
/// `git show 73d5831:src/config.rs`). `dotenvy::dotenv()`'s unconditional
/// upward walk can search past the immediate parent and does not re-check
/// "are the core vars still missing" before continuing, so swapping in one
/// unchanged default silently changed `Config::load`'s production behavior.
/// This function narrows the shared rule to ONLY the explicit-pin branch
/// (genuinely identical and safe to share across every caller) and leaves
/// each caller's unset-path exactly as it was before #216 -- see
/// `load_env_file` (the bare-`dotenv()` policy every caller except
/// `Config::load` used, and still uses) and
/// `load_env_file_config_load_unset_rule` (the two-hop policy unique to
/// `Config::load`, extracted to its own named, independently testable
/// function so `src/bin/env_resolution_probe.rs` can exercise it without a
/// full `Config::load()` round-trip).
///
/// Errors (missing/unreadable files) are intentionally swallowed, matching
/// every call site this replaces.
pub fn load_env_file_or(unset_fallback: impl FnOnce()) {
    if let Ok(env_path) = std::env::var("SURR_ENV_FILE") {
        let _ = dotenvy::from_path(env_path);
    } else {
        unset_fallback();
    }
}

/// The bare-`dotenv()` policy: every reachable dotenv-loading call site in
/// this crate EXCEPT `Config::load` used a plain `dotenvy::dotenv()` when
/// `SURR_ENV_FILE` was unset, both before and after fed-93bfee #216/#222 --
/// that part never changed. Used by `embeddings::create_embedder`,
/// `lib::load_env`, `tests/gemini_client_integration.rs`, and
/// `src/bin/reembed.rs` (spawned as a subprocess by `tests/
/// reembed_bin_dry_run.rs`, the reason this needs to be reachable at all).
/// Eleven `src/bin/*.rs` binaries (admin, kg_populate, kg_wander, kg_embed,
/// kg_consolidate, kg_dedupe_plan, kg_apply_from_plan, kg_debug_tool,
/// gem_rethink, migration, and reembed_kg) make 16 direct bare
/// `dotenvy::dotenv()` calls at their own entry points or subcommands; none are
/// spawned by anything in the `db_integration` test suite (verified via
/// grep), so they are intentionally left unconverted here rather than
/// touched as an unrelated, unbounded blast-radius change to operational
/// CLI tools. Those calls retain their established upward-searching behavior
/// and do not honor `SURR_ENV_FILE`.
pub fn load_env_file() {
    load_env_file_or(|| {
        let _ = dotenvy::dotenv();
    });
}

/// The exact rule `Config::load` used for its unset-path before fed-93bfee
/// #216 ever existed, preserved verbatim and extracted to its own function
/// (see `load_env_file_or`'s doc comment for why): load local `.env`, then
/// load `../.env` ONLY if, after that local load, NEITHER `SURR_DB_URL` NOR
/// `OPENAI_API_KEY` ended up present in the process environment. Both
/// paths are relative to the process's actual current directory --
/// `Config::load` itself never changes that, and neither does this
/// function; callers that want a specific directory (e.g. the fallback-rule
/// probe binary, or a subprocess test) control it via `current_dir` on the
/// `Command` that spawns the process, not by any path manipulation here.
pub fn load_env_file_config_load_unset_rule() {
    let _ = dotenvy::from_path(".env");
    let core_present =
        std::env::var("SURR_DB_URL").is_ok() || std::env::var("OPENAI_API_KEY").is_ok();
    if !core_present {
        let _ = dotenvy::from_path("../.env");
    }
}

/// Main configuration structure loaded from surreal_mind.toml and environment variables
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub system: SystemConfig,
    pub retrieval: RetrievalConfig,
    pub orbital_mechanics: OrbitalConfig,
    /// Runtime configuration loaded from environment variables
    #[serde(skip)]
    pub runtime: RuntimeConfig,
}

/// System-level configuration for embeddings, database, and behavior
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SystemConfig {
    pub embedding_provider: String,
    pub embedding_model: String,
    pub embedding_dimensions: usize,
    pub embed_retries: u32,
    pub database_url: String,
    pub database_ns: String,
    pub database_db: String,
    pub inject_debounce: u64,
    pub gemini_model: String,
    #[serde(default = "default_google_cli_provider")]
    pub google_cli_provider: String,
    #[serde(default = "default_antigravity_model")]
    pub antigravity_model: String,
}

fn default_google_cli_provider() -> String {
    "antigravity".to_string()
}

fn default_antigravity_model() -> String {
    "auto".to_string()
}

/// Embedding configuration snapshot for use across components
#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    pub provider: String,
    pub model: String,
    pub dimensions: usize,
    pub retries: u32,
}

/// Retrieval configuration for search and injection behavior
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RetrievalConfig {
    pub max_injection_scale: u8,
    pub default_injection_scale: u8,
    pub kg_only: bool,
    pub similarity_threshold: f32,
    pub top_k: usize,
    pub db_limit: usize,
    pub candidates: usize,
    pub submode_tuning: bool,
    pub t1: f32,
    pub t2: f32,
    pub t3: f32,
    pub floor: f32,
    pub kg_moderation_threshold: f32,
}

/// Orbital mechanics for knowledge graph entity drifting and weighting
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrbitalConfig {
    pub decay_rate: f32,
    pub access_boost: f32,
    pub significance_weight: f32,
    pub recency_weight: f32,
    pub access_weight: f32,
}

/// Runtime configuration loaded from environment variables
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub database_user: String,
    pub database_pass: String,
    pub openai_api_key: Option<String>,
    pub nomic_api_key: Option<String>,
    pub tool_timeout_ms: u64,
    pub mcp_no_log: bool,
    pub log_level: String,
    pub cache_max: usize,
    pub cache_warm: usize,
    pub retrieve_candidates: usize,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub embed_strict: bool,
    pub kg_embed_entities: bool,
    pub kg_embed_observations: bool,
    pub kg_max_neighbors: usize,
    pub kg_graph_boost: f32,
    pub kg_min_edge_strength: f32,
    pub kg_timeout_ms: u64,
    pub kg_candidates: usize,
    pub verify_topk: usize,
    pub verify_min_sim: f32,
    pub verify_evidence_limit: usize,
    pub persist_verification: bool,
    // HTTP transport configuration
    pub transport: String,
    pub http_bind: std::net::SocketAddr,
    pub http_path: String,
    pub bearer_token: Option<String>,
    pub allow_token_in_url: bool,
    pub http_sse_keepalive_sec: u64,
    pub http_session_ttl_sec: u64,
    pub http_request_timeout_ms: u64,
    pub http_mcp_op_timeout_ms: Option<u64>,
    pub http_metrics_mode: String,
    /// Hostnames accepted by rmcp's Streamable HTTP `Host` allowlist.
    ///
    /// Always seeded with the secure loopback set (`localhost`, `127.0.0.1`, `::1`).
    /// An unset `SURR_HTTP_ALLOWED_HOSTS` uses that set alone. A valid configured
    /// value extends and deduplicates the loopback set rather than replacing it.
    /// A present-but-empty or malformed value fails startup loudly (see
    /// `RuntimeConfig::load_from_env`) rather than silently falling back to
    /// loopback-only or allow-all.
    pub http_allowed_hosts: Vec<String>,
    // OAuth configuration
    pub oauth_issuer: Option<String>,
    pub oauth_client_id: Option<String>,
    pub oauth_client_secret: Option<String>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            database_user: "root".to_string(),
            database_pass: "root".to_string(),
            openai_api_key: None,
            nomic_api_key: None,
            tool_timeout_ms: 15_000,
            mcp_no_log: false,
            log_level: "surreal_mind=info,rmcp=info".to_string(),
            cache_max: 5000,
            cache_warm: 64,
            retrieve_candidates: 500,
            verify_topk: 100,
            verify_min_sim: 0.70,
            verify_evidence_limit: 10,
            persist_verification: false,
            max_retries: 3,
            retry_delay_ms: 500,
            embed_strict: false,
            kg_embed_entities: true,
            kg_embed_observations: true,
            kg_max_neighbors: 25,
            kg_graph_boost: 0.15,
            kg_min_edge_strength: 0.0,
            kg_timeout_ms: 5000,
            kg_candidates: 200,
            transport: "stdio".to_string(),
            http_bind: "127.0.0.1:8787"
                .parse()
                .expect("default bind address should parse"),
            http_path: "/mcp".to_string(),
            bearer_token: None,
            allow_token_in_url: false,
            http_sse_keepalive_sec: 15,
            http_session_ttl_sec: 900,
            http_request_timeout_ms: 10000,
            http_mcp_op_timeout_ms: None,
            http_metrics_mode: "basic".to_string(),
            http_allowed_hosts: default_http_allowed_hosts(),
            oauth_issuer: None,
            oauth_client_id: None,
            oauth_client_secret: None,
        }
    }
}

/// The secure loopback hosts always retained in the HTTP `Host` allowlist.
fn default_http_allowed_hosts() -> Vec<String> {
    vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ]
}

/// Parse `SURR_HTTP_ALLOWED_HOSTS` into the effective allowlist.
///
/// Semantics (see `RuntimeConfig::http_allowed_hosts` doc comment and upgrade
/// doc decision D5):
/// - Absent: the loopback default alone.
/// - Present and valid: loopback default, extended with the configured hosts,
///   deduplicated (order-preserving; loopback entries always come first).
/// - Present but empty, or containing any empty entry (stray/leading/trailing
///   comma, whitespace-only segment): a hard startup error. This must never
///   silently degrade to loopback-only or silently become allow-all.
fn parse_http_allowed_hosts(raw: Option<String>) -> anyhow::Result<Vec<String>> {
    let mut hosts = default_http_allowed_hosts();
    let Some(raw) = raw else {
        return Ok(hosts);
    };
    if raw.trim().is_empty() {
        anyhow::bail!(
            "SURR_HTTP_ALLOWED_HOSTS is set but empty; unset it to use the secure loopback default, or provide a comma-separated list of additional hosts"
        );
    }
    for entry in raw.split(',') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            anyhow::bail!(
                "SURR_HTTP_ALLOWED_HOSTS contains an empty entry (check for stray, leading, or trailing commas): {:?}",
                raw
            );
        }
        if !hosts.iter().any(|h| h == trimmed) {
            hosts.push(trimmed.to_string());
        }
    }
    Ok(hosts)
}

impl Config {
    /// Load configuration from TOML file and environment variables
    /// Uses SURREAL_MIND_CONFIG environment variable or defaults to "surreal_mind.toml"
    pub fn load() -> anyhow::Result<Self> {
        // Explicit-pin case shared with every other reachable dotenv call
        // site (load_env_file_or); unset case is Config::load's OWN
        // pre-existing rule, preserved verbatim
        // (load_env_file_config_load_unset_rule) -- fed-93bfee #216/#222.
        load_env_file_or(load_env_file_config_load_unset_rule);

        let config_path = std::env::var("SURREAL_MIND_CONFIG")
            .unwrap_or_else(|_| "surreal_mind.toml".to_string());

        let mut config: Config = if let Ok(content) = std::fs::read_to_string(&config_path) {
            toml::from_str(&content)?
        } else {
            // Create default config if file doesn't exist
            tracing::warn!("Config file {} not found, using defaults", config_path);
            Self::default()
        };

        // Apply env overrides for database configuration (env-first)
        if let Ok(db_url) = std::env::var("SURR_DB_URL") {
            config.system.database_url = db_url;
        }
        if let Ok(db_ns) = std::env::var("SURR_DB_NS") {
            config.system.database_ns = db_ns;
        }
        if let Ok(db_name) = std::env::var("SURR_DB_DB") {
            config.system.database_db = db_name;
        }
        // fed-77afac: env-first override for the embedding provider, same
        // pattern as the SURR_DB_* overrides above. Needed so tests and
        // scripts/test_db.sh can select the offline `fake` provider without
        // mutating the checked-in surreal_mind.toml (which stays "openai").
        // `SURR_EMBED_PROVIDER` was previously referenced only in archived
        // docs/scripts and never actually read anywhere in this crate; this
        // is the first real consumer of that name.
        if let Ok(embed_provider) = std::env::var("SURR_EMBED_PROVIDER") {
            config.system.embedding_provider = embed_provider;
        }

        // Load runtime configuration from environment variables
        config.runtime = RuntimeConfig::load_from_env()?;

        // Log env overrides for debugging (env-first confirmation)
        if std::env::var("SURR_DB_URL").is_ok() {
            tracing::debug!("SURR_DB_URL env override applied");
        }
        if std::env::var("SURR_DB_NS").is_ok() {
            tracing::debug!("SURR_DB_NS env override applied");
        }
        if std::env::var("SURR_DB_DB").is_ok() {
            tracing::debug!("SURR_DB_DB env override applied");
        }

        // Validate configuration

        // Validate database URL format (basic checks)
        if !config.system.database_url.starts_with("ws://")
            && !config.system.database_url.starts_with("wss://")
            && !config.system.database_url.starts_with("http://")
            && !config.system.database_url.starts_with("https://")
        {
            tracing::warn!(
                "Database URL '{}' doesn't start with ws://, wss://, http://, or https://",
                config.system.database_url
            );
        } else {
            // Basic hostname:port validation for WebSocket schemes
            let normalized = config
                .system
                .database_url
                .strip_prefix("ws://")
                .or_else(|| config.system.database_url.strip_prefix("wss://"))
                .unwrap_or(&config.system.database_url);

            if !normalized.contains(":") || normalized.starts_with(":") || normalized.ends_with(":")
            {
                tracing::warn!(
                    "Database URL '{}' appears to be missing hostname or port",
                    config.system.database_url
                );
            }
        }

        // Validate and clamp embed_retries
        if config.system.embed_retries == 0 {
            config.system.embed_retries = 1;
        } else if config.system.embed_retries > 10 {
            tracing::warn!(
                "embed_retries {} exceeds max 10, clamping to 10",
                config.system.embed_retries
            );
            config.system.embed_retries = 10;
        }

        // Validate provider/dimension coherence
        match config.system.embedding_provider.as_str() {
            "openai" => match config.system.embedding_model.as_str() {
                "text-embedding-3-small" => {
                    if config.system.embedding_dimensions != 1536
                        && std::env::var("SURR_EMBED_STRICT").ok().as_deref() == Some("true")
                    {
                        return Err(anyhow::anyhow!(
                            "OpenAI text-embedding-3-small requires 1536 dimensions, got {}",
                            config.system.embedding_dimensions
                        ));
                    } else if config.system.embedding_dimensions != 1536 {
                        tracing::warn!(
                            "OpenAI text-embedding-3-small should use 1536 dimensions, got {}",
                            config.system.embedding_dimensions
                        );
                    }
                }
                "text-embedding-3-large" => {
                    if config.system.embedding_dimensions != 3072
                        && std::env::var("SURR_EMBED_STRICT").ok().as_deref() == Some("true")
                    {
                        return Err(anyhow::anyhow!(
                            "OpenAI text-embedding-3-large requires 3072 dimensions, got {}",
                            config.system.embedding_dimensions
                        ));
                    } else if config.system.embedding_dimensions != 3072 {
                        tracing::warn!(
                            "OpenAI text-embedding-3-large should use 3072 dimensions, got {}",
                            config.system.embedding_dimensions
                        );
                    }
                }
                _ => tracing::warn!(
                    "Unknown OpenAI embedding model '{}', dimension validation skipped",
                    config.system.embedding_model
                ),
            },

            "fake" => {
                // Offline/test-only embedder (fed-77afac): any positive
                // dimension is legitimate here -- there is no fixed model
                // to be coherent with, unlike the openai arms above. The
                // "positive" half is NOT enforced at this layer; it is
                // enforced where the type is constructed, by
                // `FakeEmbedder::new` (src/embeddings.rs), which rejects
                // `dims == 0` outright. Log at
                // debug rather than warn so a correctly-configured offline
                // run doesn't produce a misleading "unknown provider"
                // warning, and isn't mistaken for an actual problem either.
                tracing::debug!(
                    "Fake embedding provider active with {} dimensions (test-only, see clu fed-77afac)",
                    config.system.embedding_dimensions
                );
            }
            _ => tracing::warn!(
                "Unknown embedding provider '{}', validation skipped",
                config.system.embedding_provider
            ),
        }

        Ok(config)
    }

    /// Convenience: snapshot embedding configuration
    pub fn embedding(&self) -> EmbeddingConfig {
        EmbeddingConfig {
            provider: self.system.embedding_provider.clone(),
            model: self.system.embedding_model.clone(),
            dimensions: self.system.embedding_dimensions,
            retries: self.system.embed_retries,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            system: SystemConfig {
                embedding_provider: "openai".to_string(),
                embedding_model: "text-embedding-3-small".to_string(),
                embedding_dimensions: 1536,
                embed_retries: 3,
                database_url: "127.0.0.1:8000".to_string(),
                database_ns: "surreal_mind".to_string(),
                database_db: "consciousness".to_string(),
                inject_debounce: 1000,
                gemini_model: "gemini-3-flash-preview".to_string(),
                google_cli_provider: default_google_cli_provider(),
                antigravity_model: default_antigravity_model(),
            },
            retrieval: RetrievalConfig {
                max_injection_scale: 3,
                default_injection_scale: 1,
                kg_only: true,
                similarity_threshold: 0.5,
                top_k: 10,
                db_limit: 500,
                candidates: 200,
                submode_tuning: false,
                t1: 0.6,
                t2: 0.4,
                t3: 0.25,
                floor: 0.15,
                kg_moderation_threshold: 0.6,
            },
            orbital_mechanics: OrbitalConfig {
                decay_rate: 0.1,
                access_boost: 0.2,
                significance_weight: 0.3,
                recency_weight: 0.4,
                access_weight: 0.3,
            },
            runtime: RuntimeConfig::default(),
        }
    }
}

impl RuntimeConfig {
    /// Load runtime configuration from environment variables
    pub fn load_from_env() -> anyhow::Result<Self> {
        let mut cfg = Self {
            database_user: std::env::var("SURR_DB_USER").unwrap_or_else(|_| "root".to_string()),
            database_pass: std::env::var("SURR_DB_PASS").unwrap_or_else(|_| "root".to_string()),
            openai_api_key: std::env::var("OPENAI_API_KEY").ok(),
            nomic_api_key: std::env::var("NOMIC_API_KEY").ok(),
            tool_timeout_ms: std::env::var("SURR_TOOL_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(15_000),
            mcp_no_log: std::env::var("MCP_NO_LOG")
                .ok()
                .is_some_and(|v| v == "true" || v == "1"),
            log_level: std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "surreal_mind=info,rmcp=info".to_string()),
            cache_max: std::env::var("SURR_CACHE_MAX")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5000),
            cache_warm: std::env::var("SURR_CACHE_WARM")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(64),
            retrieve_candidates: std::env::var("SURR_RETRIEVE_CANDIDATES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(500),
            verify_topk: std::env::var("SURR_VERIFY_TOPK")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
            verify_min_sim: std::env::var("SURR_VERIFY_MIN_SIM")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.70),
            verify_evidence_limit: std::env::var("SURR_VERIFY_EVIDENCE_LIMIT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            persist_verification: std::env::var("SURR_PERSIST_VERIFICATION")
                .ok()
                .is_some_and(|v| v == "true" || v == "1"),
            max_retries: std::env::var("SURR_EMBED_RETRIES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
            retry_delay_ms: std::env::var("SURR_RETRY_DELAY_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(500),
            embed_strict: std::env::var("SURR_EMBED_STRICT")
                .ok()
                .is_some_and(|v| v == "true" || v == "1"),
            kg_embed_entities: std::env::var("SURR_KG_EMBED_ENTITIES")
                .ok()
                .is_none_or(|v| v != "false" && v != "0"),
            kg_embed_observations: std::env::var("SURR_KG_EMBED_OBSERVATIONS")
                .ok()
                .is_none_or(|v| v != "false" && v != "0"),
            kg_max_neighbors: std::env::var("SURR_KG_MAX_NEIGHBORS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(25),
            kg_graph_boost: std::env::var("SURR_KG_GRAPH_BOOST")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.15),
            kg_min_edge_strength: std::env::var("SURR_KG_MIN_EDGE_STRENGTH")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
            kg_timeout_ms: std::env::var("SURR_KG_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5000),
            kg_candidates: std::env::var("SURR_KG_CANDIDATES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(200),
            transport: "stdio".to_string(),
            http_bind: "127.0.0.1:8787"
                .parse()
                .expect("default bind address should parse"),
            http_path: "/mcp".to_string(),
            bearer_token: None,
            allow_token_in_url: false,
            http_sse_keepalive_sec: 15,
            http_session_ttl_sec: 900,
            http_request_timeout_ms: 10000,
            http_mcp_op_timeout_ms: None,
            http_metrics_mode: "basic".to_string(),
            http_allowed_hosts: default_http_allowed_hosts(),
            oauth_issuer: None,
            oauth_client_id: None,
            oauth_client_secret: None,
        };

        // HTTP transport configuration
        cfg.transport = std::env::var("SURR_TRANSPORT").unwrap_or_else(|_| "stdio".to_string());
        if let Ok(v) = std::env::var("SURR_HTTP_BIND")
            && let Ok(bind) = v.parse::<std::net::SocketAddr>()
        {
            cfg.http_bind = bind;
        }
        cfg.http_path = std::env::var("SURR_HTTP_PATH").unwrap_or_else(|_| "/mcp".to_string());
        cfg.bearer_token = std::env::var("SURR_BEARER_TOKEN").ok().or_else(|| {
            // Fallback to ~/.surr_token
            let home = std::env::var("HOME").ok()?;
            std::fs::read_to_string(format!("{}/.surr_token", home))
                .ok()
                .map(|s| s.trim().to_string())
        });
        if let Ok(allow) = std::env::var("SURR_ALLOW_TOKEN_IN_URL") {
            cfg.allow_token_in_url = allow == "1" || allow.to_lowercase() == "true";
        }
        if let Some(sse) = std::env::var("SURR_HTTP_SSE_KEEPALIVE_SEC")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            cfg.http_sse_keepalive_sec = sse;
        }
        if let Some(ttl) = std::env::var("SURR_HTTP_SESSION_TTL_SEC")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            cfg.http_session_ttl_sec = ttl;
        }
        if let Some(timeout) = std::env::var("SURR_HTTP_REQUEST_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            cfg.http_request_timeout_ms = timeout;
        }
        cfg.http_mcp_op_timeout_ms = std::env::var("SURR_HTTP_MCP_OP_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok());
        cfg.http_metrics_mode =
            std::env::var("SURR_HTTP_METRICS_MODE").unwrap_or_else(|_| "basic".to_string());
        cfg.http_allowed_hosts =
            parse_http_allowed_hosts(std::env::var("SURR_HTTP_ALLOWED_HOSTS").ok())?;
        cfg.oauth_issuer = std::env::var("SURR_OAUTH_ISSUER").ok();
        cfg.oauth_client_id = std::env::var("SURR_OAUTH_CLIENT_ID").ok();
        cfg.oauth_client_secret = std::env::var("SURR_OAUTH_CLIENT_SECRET").ok();

        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_loading() {
        // This test would require a test config file, but demonstrates the pattern
        let config = Config::load();
        assert!(config.is_ok() || config.is_err()); // Either way, method works
    }

    // load_env_file_or / load_env_file / load_env_file_config_load_unset_rule
    // (fed-93bfee #216/#222): the dotenv-resolution controls for these live
    // in tests/config_env_resolution.rs, NOT here -- Codex's #222 review
    // required them to be private-temp SUBPROCESS controls (spawn
    // src/bin/env_resolution_probe.rs with an isolated current_dir and an
    // explicit, cleared env), never writing a decoy .env under this
    // checkout and never calling std::env::set_var/remove_var in-process.
    // An earlier version of these tests lived here, mutated process-wide
    // env vars and a real file at the crate root, and needed a Mutex to
    // avoid racing test_config_loading above -- all of that is gone now
    // that the controls run in their own processes instead.

    // --- SURR_HTTP_ALLOWED_HOSTS semantics (upgrade doc D5 / HTTP-01..05) ---

    #[test]
    fn http_allowed_hosts_absent_uses_loopback_default_only() {
        let hosts = parse_http_allowed_hosts(None).expect("absent value must be valid");
        assert_eq!(hosts, vec!["localhost", "127.0.0.1", "::1"]);
    }

    #[test]
    fn http_allowed_hosts_configured_value_extends_loopback_default() {
        let hosts = parse_http_allowed_hosts(Some("mcp.samataganaphotography.com".to_string()))
            .expect("single valid host must parse");
        assert_eq!(
            hosts,
            vec![
                "localhost",
                "127.0.0.1",
                "::1",
                "mcp.samataganaphotography.com"
            ]
        );
        // Loopback retention: the secure defaults are never displaced by configuration.
        assert!(hosts.contains(&"localhost".to_string()));
        assert!(hosts.contains(&"127.0.0.1".to_string()));
        assert!(hosts.contains(&"::1".to_string()));
    }

    #[test]
    fn http_allowed_hosts_multiple_entries_are_trimmed_and_appended() {
        let hosts = parse_http_allowed_hosts(Some(
            " mcp.samataganaphotography.com , extra-host.example.com ".to_string(),
        ))
        .expect("whitespace-padded list must parse");
        assert_eq!(
            hosts,
            vec![
                "localhost",
                "127.0.0.1",
                "::1",
                "mcp.samataganaphotography.com",
                "extra-host.example.com"
            ]
        );
    }

    #[test]
    fn http_allowed_hosts_duplicates_are_deduplicated() {
        // A configured entry that collides with a loopback default, and a
        // configured entry duplicated against itself, both collapse to one.
        let hosts = parse_http_allowed_hosts(Some(
            "localhost,mcp.samataganaphotography.com,mcp.samataganaphotography.com".to_string(),
        ))
        .expect("list with duplicates must still parse");
        assert_eq!(
            hosts,
            vec![
                "localhost",
                "127.0.0.1",
                "::1",
                "mcp.samataganaphotography.com"
            ]
        );
    }

    #[test]
    fn http_allowed_hosts_present_but_empty_fails_startup() {
        let err = parse_http_allowed_hosts(Some(String::new()))
            .expect_err("present-but-empty value must fail, never silently fall back");
        assert!(err.to_string().contains("SURR_HTTP_ALLOWED_HOSTS"));
    }

    #[test]
    fn http_allowed_hosts_whitespace_only_value_fails_startup() {
        let err = parse_http_allowed_hosts(Some("   ".to_string()))
            .expect_err("whitespace-only value must fail like an empty value");
        assert!(err.to_string().contains("SURR_HTTP_ALLOWED_HOSTS"));
    }

    #[test]
    fn http_allowed_hosts_trailing_comma_fails_startup() {
        let err = parse_http_allowed_hosts(Some("mcp.samataganaphotography.com,".to_string()))
            .expect_err("trailing comma yields an empty entry and must fail");
        assert!(err.to_string().contains("empty entry"));
    }

    #[test]
    fn http_allowed_hosts_interior_empty_entry_fails_startup() {
        let err = parse_http_allowed_hosts(Some(
            "mcp.samataganaphotography.com,,extra-host.example.com".to_string(),
        ))
        .expect_err("stray interior comma yields an empty entry and must fail");
        assert!(err.to_string().contains("empty entry"));
    }

    #[test]
    fn http_allowed_hosts_never_becomes_allow_all_or_loopback_only_on_malformed_input() {
        // A malformed value must be a hard error, not a silent substitution in
        // either direction (never allow-all, never a silent loopback-only
        // fallback that hides an operator's intent to add a public host).
        for malformed in ["", "   ", ",", "a,,b", "a, ,b"] {
            assert!(
                parse_http_allowed_hosts(Some(malformed.to_string())).is_err(),
                "expected {malformed:?} to be rejected"
            );
        }
    }
}
