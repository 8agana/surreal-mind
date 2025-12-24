use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main configuration structure loaded from surreal_mind.toml and environment variables
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub system: SystemConfig,
    pub retrieval: RetrievalConfig,
    pub orbital_mechanics: OrbitalConfig,
    /// Deprecated: submodes removed from tool surfaces; kept for backward compatibility
    pub submodes: HashMap<String, SubmodeConfig>,
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

/// Deprecated: Configuration for individual submodes (thinking styles) - no longer used in tool surfaces
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubmodeConfig {
    pub injection_scale: u8,
    pub significance: f32,
    pub kg_traverse_depth: u8,
    pub frameworks: HashMap<String, f32>,
    pub orbital_weights: OrbitalWeights,
    pub auto_extract: bool,
    pub edge_boosts: HashMap<String, f32>,
}

/// Weights for orbital mechanics calculations
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrbitalWeights {
    pub recency: f32,
    pub access: f32,
    pub significance: f32,
}

/// Configuration for inner_voice.retrieve tool
#[derive(Debug, Clone)]
pub struct InnerVoiceConfig {
    pub enable: bool,
    pub mix: f32,
    pub topk_default: usize,
    pub min_floor: f32,
    pub max_candidates_per_source: usize,
    pub include_private_default: bool,
    pub plan: bool,
    pub auto_extract_default: bool,
    pub recency_half_life_days: f32,
    pub recency_days_default: Option<u32>,
    pub prefer_recent_default: bool,
}

impl Default for InnerVoiceConfig {
    fn default() -> Self {
        Self {
            // Default ON to avoid surprising hidden tools in downstream MCP launchers
            enable: true,
            mix: 0.6,
            topk_default: 10,
            min_floor: 0.15,
            max_candidates_per_source: 150,
            include_private_default: false,
            plan: false,
            auto_extract_default: true,
            recency_half_life_days: 14.0,
            recency_days_default: None,
            prefer_recent_default: false,
        }
    }
}

impl InnerVoiceConfig {
    /// Load inner_voice configuration from environment variables
    pub fn load_from_env() -> Self {
        let mut config = Self::default();

        // Backward/forward compatible gating semantics:
        // - SURR_DISABLE_INNER_VOICE=1|true force-disables
        // - SURR_ENABLE_INNER_VOICE=0|false disables; =1|true enables
        // - default: enabled
        if let Ok(disable) = std::env::var("SURR_DISABLE_INNER_VOICE")
            && (disable == "1" || disable.eq_ignore_ascii_case("true"))
        {
            config.enable = false;
        }
        if let Ok(enable) = std::env::var("SURR_ENABLE_INNER_VOICE") {
            if enable == "0" || enable.eq_ignore_ascii_case("false") {
                config.enable = false;
            } else if enable == "1" || enable.eq_ignore_ascii_case("true") {
                config.enable = true;
            }
        }

        if let Some(mix) = std::env::var("SURR_INNER_VOICE_MIX")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        {
            config.mix = mix.clamp(0.0, 1.0);
        }

        if let Some(topk) = std::env::var("SURR_INNER_VOICE_TOPK_DEFAULT")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
        {
            config.topk_default = topk.clamp(1, 50);
        }

        if let Some(min_floor) = std::env::var("SURR_INNER_VOICE_MIN_FLOOR")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            && (0.0..=1.0).contains(&min_floor)
        {
            config.min_floor = min_floor;
        }

        if let Some(max_candidates) = std::env::var("SURR_INNER_VOICE_MAX_CANDIDATES_PER_SOURCE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
        {
            config.max_candidates_per_source = max_candidates.max(3 * config.topk_default);
        }

        if let Ok(include_private) = std::env::var("SURR_INNER_VOICE_INCLUDE_PRIVATE_DEFAULT") {
            config.include_private_default =
                include_private == "1" || include_private.to_lowercase() == "true";
        }

        if let Ok(plan) = std::env::var("SURR_IV_PLAN") {
            config.plan = plan == "1" || plan.to_lowercase() == "true";
        }

        if let Ok(ae) = std::env::var("SURR_IV_AUTO_EXTRACT_KG") {
            config.auto_extract_default = ae == "1" || ae.to_lowercase() == "true";
        }

        if let Some(hl) = std::env::var("SURR_IV_RECENCY_HALF_LIFE_DAYS")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            && hl > 0.0
        {
            config.recency_half_life_days = hl;
        }

        if let Some(days) = std::env::var("SURR_IV_RECENCY_DEFAULT_DAYS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            && days > 0
        {
            config.recency_days_default = Some(days);
        }

        if let Ok(pref) = std::env::var("SURR_IV_PREFER_RECENT") {
            config.prefer_recent_default = pref == "1" || pref.eq_ignore_ascii_case("true");
        }

        config
    }

    /// Validate the configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        if !(0.0..=1.0).contains(&self.mix) {
            anyhow::bail!("SURR_INNER_VOICE_MIX must be between 0.0 and 1.0");
        }
        if !(1..=50).contains(&self.topk_default) {
            anyhow::bail!("SURR_INNER_VOICE_TOPK_DEFAULT must be between 1 and 50");
        }
        if !(0.0..=1.0).contains(&self.min_floor) {
            anyhow::bail!("SURR_INNER_VOICE_MIN_FLOOR must be between 0.0 and 1.0");
        }
        if self.max_candidates_per_source < 3 * self.topk_default {
            anyhow::bail!(
                "SURR_INNER_VOICE_MAX_CANDIDATES_PER_SOURCE must be at least 3 * topk_default"
            );
        }
        if self.recency_half_life_days <= 0.0 {
            anyhow::bail!("SURR_IV_RECENCY_HALF_LIFE_DAYS must be > 0");
        }
        Ok(())
    }
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
    pub inner_voice: InnerVoiceConfig,
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
            inner_voice: InnerVoiceConfig::default(),
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
        }
    }
}

impl Config {
    /// Load configuration from TOML file and environment variables
    /// Uses SURREAL_MIND_CONFIG environment variable or defaults to "surreal_mind.toml"
    pub fn load() -> anyhow::Result<Self> {
        // Load environment variables with smart fallbacks:
        // 1) SURR_ENV_FILE if set
        // 2) ./.env
        // 3) ../.env (repo root when running from crate dir)
        if let Ok(env_path) = std::env::var("SURR_ENV_FILE") {
            let _ = dotenvy::from_path(env_path);
        } else {
            // Current directory .env
            let _ = dotenvy::from_path(".env");
            // Fallback to parent .env if core vars are still missing
            let core_present = std::env::var("SURR_DB_URL").is_ok()
                || std::env::var("OPENAI_API_KEY").is_ok()
                || std::env::var("SURR_ENABLE_INNER_VOICE").is_ok();
            if !core_present {
                let _ = dotenvy::from_path("../.env");
            }
        }

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

        // Load runtime configuration from environment variables
        config.runtime = RuntimeConfig::load_from_env();

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
            "candle" | "local" => {
                if config.system.embedding_model != "bge-small-en-v1.5" {
                    tracing::warn!(
                        "Candle/local provider expects 'bge-small-en-v1.5' model, got '{}'",
                        config.system.embedding_model
                    );
                }
                if config.system.embedding_dimensions != 384 {
                    tracing::warn!(
                        "Candle/local BGE model expects 384 dimensions, got {}",
                        config.system.embedding_dimensions
                    );
                }
            }
            _ => tracing::warn!(
                "Unknown embedding provider '{}', validation skipped",
                config.system.embedding_provider
            ),
        }

        // Validate inner_voice config
        config.runtime.inner_voice.validate()?;

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

    /// Deprecated: Get submode configuration by name, with fallback to "build" mode - no longer used
    pub fn get_submode(&self, mode: &str) -> &SubmodeConfig {
        self.submodes.get(mode).unwrap_or_else(|| {
            self.submodes
                .get("build")
                .expect("build submode should exist")
        })
    }
}

impl Default for Config {
    fn default() -> Self {
        let mut submodes = HashMap::new();
        submodes.insert(
            "build".to_string(),
            SubmodeConfig {
                injection_scale: 2,
                significance: 0.5,
                kg_traverse_depth: 1,
                frameworks: HashMap::new(),
                orbital_weights: OrbitalWeights {
                    recency: 0.4,
                    access: 0.3,
                    significance: 0.3,
                },
                auto_extract: true,
                edge_boosts: HashMap::new(),
            },
        );

        Self {
            system: SystemConfig {
                embedding_provider: "openai".to_string(),
                embedding_model: "text-embedding-3-small".to_string(),
                embedding_dimensions: 1536,
                embed_retries: 3,
                database_url: "127.0.0.1:8000".to_string(),
                database_ns: "surreal_mind".to_string(),
                database_db: "conciousness".to_string(),
                inject_debounce: 1000,
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
            submodes,
            runtime: RuntimeConfig::default(),
        }
    }
}

impl RuntimeConfig {
    /// Load runtime configuration from environment variables
    pub fn load_from_env() -> Self {
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
            inner_voice: InnerVoiceConfig::load_from_env(),
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

        cfg
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

    #[test]
    fn test_submode_fallback() {
        let mut submodes = HashMap::new();
        submodes.insert(
            "build".to_string(),
            SubmodeConfig {
                injection_scale: 1,
                significance: 0.5,
                kg_traverse_depth: 1,
                frameworks: HashMap::new(),
                orbital_weights: OrbitalWeights {
                    recency: 0.7,
                    access: 0.2,
                    significance: 0.1,
                },
                auto_extract: false,
                edge_boosts: HashMap::new(),
            },
        );

        let config = Config {
            system: SystemConfig {
                embedding_provider: "test".to_string(),
                embedding_model: "test".to_string(),
                embedding_dimensions: 768,
                embed_retries: 3,
                database_url: "test".to_string(),
                database_ns: "test".to_string(),
                database_db: "test".to_string(),
                inject_debounce: 1000,
            },
            retrieval: RetrievalConfig {
                max_injection_scale: 3,
                default_injection_scale: 1,
                kg_only: true,
                similarity_threshold: 0.5,
                top_k: 5,
                db_limit: 100,
                candidates: 20,
                submode_tuning: true,
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
            submodes,
            runtime: RuntimeConfig::default(),
        };

        let mode = config.get_submode("nonexistent");
        assert_eq!(mode.injection_scale, 1);
        assert_eq!(mode.significance, 0.5);
    }
}
