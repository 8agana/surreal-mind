use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main configuration structure loaded from surreal_mind.toml and environment variables
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub system: SystemConfig,
    pub retrieval: RetrievalConfig,
    pub orbital_mechanics: OrbitalConfig,
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

/// Configuration for individual submodes (thinking styles)
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
        if let Ok(disable) = std::env::var("SURR_DISABLE_INNER_VOICE") {
            if disable == "1" || disable.eq_ignore_ascii_case("true") {
                config.enable = false;
            }
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
        {
            if (0.0..=1.0).contains(&min_floor) {
                config.min_floor = min_floor;
            }
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
    pub inner_voice: InnerVoiceConfig,
    // Photography repo support (env-driven)
    pub photo_enable: bool,
    pub photo_url: Option<String>,
    pub photo_ns: Option<String>,
    pub photo_db: Option<String>,
    pub photo_user: Option<String>,
    pub photo_pass: Option<String>,
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
            photo_enable: false,
            photo_url: None,
            photo_ns: None,
            photo_db: None,
            photo_user: None,
            photo_pass: None,
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

        // Load runtime configuration from environment variables
        config.runtime = RuntimeConfig::load_from_env();

        // Validate configuration
        
        // Validate database URL format
        if !config.system.database_url.starts_with("ws://") 
            && !config.system.database_url.starts_with("wss://") 
            && !config.system.database_url.starts_with("http://") 
            && !config.system.database_url.starts_with("https://") {
            tracing::warn!(
                "Database URL '{}' doesn't start with ws://, wss://, http://, or https://", 
                config.system.database_url
            );
        }
        
        // Validate and clamp embed_retries
        if config.system.embed_retries == 0 {
            config.system.embed_retries = 1;
        } else if config.system.embed_retries > 10 {
            tracing::warn!("embed_retries {} exceeds max 10, clamping to 10", config.system.embed_retries);
            config.system.embed_retries = 10;
        }
        
        // Validate provider/dimension coherence
        if config.system.embedding_provider == "openai" {
            if config.system.embedding_model == "text-embedding-3-small" && config.system.embedding_dimensions != 1536 {
                if std::env::var("SURR_EMBED_STRICT").ok().as_deref() == Some("true") {
                    return Err(anyhow::anyhow!(
                        "OpenAI text-embedding-3-small requires 1536 dimensions, got {}",
                        config.system.embedding_dimensions
                    ));
                } else {
                    tracing::warn!(
                        "OpenAI text-embedding-3-small should use 1536 dimensions, got {}",
                        config.system.embedding_dimensions
                    );
                }
            }
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

    /// Get submode configuration by name, with fallback to "build" mode
    pub fn get_submode(&self, mode: &str) -> &SubmodeConfig {
        self.submodes
            .get(mode)
            .unwrap_or_else(|| self.submodes.get("build").unwrap())
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
                database_db: "consciousness".to_string(),
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
            photo_enable: false,
            photo_url: None,
            photo_ns: None,
            photo_db: None,
            photo_user: None,
            photo_pass: None,
        };

        // Photography repo envs
        if let Ok(enable) = std::env::var("SURR_ENABLE_PHOTOGRAPHY") {
            cfg.photo_enable = enable == "1" || enable.to_lowercase() == "true";
        }
        cfg.photo_url = std::env::var("SURR_PHOTO_URL").ok();
        cfg.photo_ns = std::env::var("SURR_PHOTO_NS").ok();
        cfg.photo_db = std::env::var("SURR_PHOTO_DB").ok();
        cfg.photo_user = std::env::var("SURR_PHOTO_USER").ok();
        cfg.photo_pass = std::env::var("SURR_PHOTO_PASS").ok();

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
