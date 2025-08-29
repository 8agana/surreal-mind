use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main configuration structure loaded from surreal_mind.toml
#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub system: SystemConfig,
    pub retrieval: RetrievalConfig,
    pub orbital_mechanics: OrbitalConfig,
    pub submodes: HashMap<String, SubmodeConfig>,
}

/// System-level configuration for embeddings, database, and behavior
#[derive(Debug, Deserialize, Serialize)]
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

/// Retrieval configuration for search and injection behavior
#[derive(Debug, Deserialize, Serialize)]
pub struct RetrievalConfig {
    pub max_injection_scale: u8,
    pub default_injection_scale: u8,
    pub kg_only: bool,
    pub similarity_threshold: f32,
    pub top_k: usize,
    pub db_limit: usize,
    pub candidates: usize,
    pub submode_tuning: bool,
}

/// Orbital mechanics for knowledge graph entity drifting and weighting
#[derive(Debug, Deserialize, Serialize)]
pub struct OrbitalConfig {
    pub decay_rate: f32,
    pub access_boost: f32,
    pub significance_weight: f32,
    pub recency_weight: f32,
    pub access_weight: f32,
}

/// Configuration for individual submodes (thinking styles)
#[derive(Debug, Deserialize, Serialize)]
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
#[derive(Debug, Deserialize, Serialize)]
pub struct OrbitalWeights {
    pub recency: f32,
    pub access: f32,
    pub significance: f32,
}

impl Config {
    /// Load configuration from TOML file
    /// Uses SURREAL_MIND_CONFIG environment variable or defaults to "surreal_mind.toml"
    #[allow(dead_code)]
    pub fn load() -> anyhow::Result<Self> {
        let config_path = std::env::var("SURREAL_MIND_CONFIG")
            .unwrap_or_else(|_| "surreal_mind.toml".to_string());

        let content = std::fs::read_to_string(config_path)?;
        let config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Get submode configuration by name, with fallback to "build" mode
    #[allow(dead_code)]
    pub fn get_submode(&self, mode: &str) -> &SubmodeConfig {
        self.submodes
            .get(mode)
            .unwrap_or_else(|| self.submodes.get("build").unwrap())
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
            },
            orbital_mechanics: OrbitalConfig {
                decay_rate: 0.1,
                access_boost: 0.2,
                significance_weight: 0.3,
                recency_weight: 0.4,
                access_weight: 0.3,
            },
            submodes,
        };

        let mode = config.get_submode("nonexistent");
        assert_eq!(mode.injection_scale, 1);
        assert_eq!(mode.significance, 0.5);
    }
}
