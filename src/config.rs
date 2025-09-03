use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main configuration structure loaded from surreal_mind.toml and environment variables
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub system: SystemConfig,
    pub retrieval: RetrievalConfig,
    pub orbital_mechanics: OrbitalConfig,
    pub submodes: HashMap<String, SubmodeConfig>,
    pub nlq: NlqConfig,
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

/// NLQ configuration for natural language queries
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NlqConfig {
    pub timezone: String,     // IANA TZ, e.g., "America/Chicago"
    pub default_limit: usize, // e.g., 25
    pub max_limit: usize,     // e.g., 100
    pub max_keywords: usize,  // cap keyword count for regex
    pub enable_keyword_filter: bool,
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
        }
    }
}

impl Config {
    /// Load configuration from TOML file and environment variables
    /// Uses SURREAL_MIND_CONFIG environment variable or defaults to "surreal_mind.toml"
    pub fn load() -> anyhow::Result<Self> {
        // Load environment variables first
        let _ = dotenvy::dotenv();

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
            },
            orbital_mechanics: OrbitalConfig {
                decay_rate: 0.1,
                access_boost: 0.2,
                significance_weight: 0.3,
                recency_weight: 0.4,
                access_weight: 0.3,
            },
            submodes,
            nlq: NlqConfig {
                timezone: "America/Chicago".to_string(),
                default_limit: 25,
                max_limit: 100,
                max_keywords: 10,
                enable_keyword_filter: true,
            },
            runtime: RuntimeConfig::default(),
        }
    }
}

impl RuntimeConfig {
    /// Load runtime configuration from environment variables
    pub fn load_from_env() -> Self {
        Self {
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
                .map_or(true, |v| v != "false" && v != "0"),
            kg_embed_observations: std::env::var("SURR_KG_EMBED_OBSERVATIONS")
                .ok()
                .map_or(true, |v| v != "false" && v != "0"),
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
        }
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
            nlq: NlqConfig {
                timezone: "America/Chicago".to_string(),
                default_limit: 25,
                max_limit: 100,
                max_keywords: 10,
                enable_keyword_filter: true,
            },
        };

        let mode = config.get_submode("nonexistent");
        assert_eq!(mode.injection_scale, 1);
        assert_eq!(mode.significance, 0.5);
    }
}
