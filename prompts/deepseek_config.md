# DeepSeek: Config File and Submode Defaults

## Context
We're moving all refined parameters to a config file so the API can be simplified to just `think(content, mode)`. You'll create the config structure and migrate existing parameters.

## Your Task
Create a comprehensive config file system and update submode defaults.

## 1. Create surreal_mind.toml

Location: `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/surreal_mind.toml`

```toml
[system]
embedding_provider = "openai"
embedding_model = "text-embedding-3-small" 
embedding_dimensions = 768
database_url = "ws://127.0.0.1:8000"
database_ns = "surreal_mind"
database_db = "consciousness"

[retrieval]
max_injection_scale = 3
default_injection_scale = 1
kg_only = true  # New: only pull from KG, not thoughts
similarity_threshold = 0.5
top_k = 5

[orbital_mechanics]
# How entities drift in the KG
decay_rate = 0.1  # Per day
access_boost = 0.2  # Per access
significance_weight = 0.3
recency_weight = 0.4
access_weight = 0.3

# Submode configurations
[submodes.plan]
injection_scale = 3  # Mars orbit - maximum context
significance = 0.8
kg_traverse_depth = 2
frameworks = { SystemsThinking = 0.6, FirstPrinciples = 0.4 }
orbital_weights = { recency = 0.2, access = 0.3, significance = 0.5 }
auto_extract = true
edge_boosts = { depends_on = 1.5, blocks = 1.3, relates_to = 1.1 }

[submodes.build]  
injection_scale = 1  # Mercury orbit - focused
significance = 0.5
kg_traverse_depth = 1
frameworks = { OODA = 0.8, Lateral = 0.2 }
orbital_weights = { recency = 0.7, access = 0.2, significance = 0.1 }
auto_extract = false  # Don't pollute KG while building
edge_boosts = { implements = 1.5, uses = 1.3 }

[submodes.debug]
injection_scale = 1  # Mercury orbit - current issue only
significance = 0.9  # Debug thoughts are important
kg_traverse_depth = 1
frameworks = { RootCause = 0.7, OODA = 0.3 }
orbital_weights = { recency = 0.8, access = 0.1, significance = 0.1 }
auto_extract = true
edge_boosts = { causes = 2.0, fixes = 1.8, related_to = 1.2 }

[submodes.sarcastic]
injection_scale = 2  # Venus orbit - needs context for callbacks
significance = 0.6
kg_traverse_depth = 1
frameworks = { Socratic = 0.6, OODA = 0.3, Lateral = 0.1 }
orbital_weights = { recency = 0.5, access = 0.3, significance = 0.2 }
auto_extract = true
edge_boosts = { mocks = 1.5, references = 1.3 }

[submodes.empathetic]
injection_scale = 2  # Venus orbit - emotional context
significance = 0.7
kg_traverse_depth = 1
frameworks = { Socratic = 0.8, Lateral = 0.2 }
orbital_weights = { recency = 0.4, access = 0.2, significance = 0.4 }
auto_extract = true
edge_boosts = { relates_to = 1.5, supports = 1.3 }

[submodes.philosophical]
injection_scale = 3  # Mars orbit - deep connections
significance = 0.9
kg_traverse_depth = 3  # Follow deep chains
frameworks = { FirstPrinciples = 0.5, SystemsThinking = 0.3, Dialectical = 0.2 }
orbital_weights = { recency = 0.1, access = 0.2, significance = 0.7 }
auto_extract = true
edge_boosts = { implies = 1.8, contradicts = 1.6, relates_to = 1.2 }

[submodes.problem_solving]
injection_scale = 2  # Venus orbit - balanced
significance = 0.7
kg_traverse_depth = 2
frameworks = { RootCause = 0.4, SystemsThinking = 0.3, OODA = 0.3 }
orbital_weights = { recency = 0.4, access = 0.3, significance = 0.3 }
auto_extract = true
edge_boosts = { solves = 2.0, blocks = 1.5, depends_on = 1.3 }
```

## 2. Create Config Loader in Rust

Location: `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/src/config.rs`

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub system: SystemConfig,
    pub retrieval: RetrievalConfig,
    pub orbital_mechanics: OrbitalConfig,
    pub submodes: HashMap<String, SubmodeConfig>,
}

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

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = std::env::var("SURREAL_MIND_CONFIG")
            .unwrap_or_else(|_| "surreal_mind.toml".to_string());
        
        let content = std::fs::read_to_string(config_path)?;
        toml::from_str(&content).map_err(Into::into)
    }
    
    pub fn get_submode(&self, mode: &str) -> &SubmodeConfig {
        self.submodes.get(mode)
            .unwrap_or_else(|| self.submodes.get("build").unwrap())
    }
}
```

## 3. Update Environment Variables

Remove from .env (now in config):
- SURR_EMBED_DIM (→ config: embedding_dimensions)
- SURR_SIM_THRESH (→ config: similarity_threshold)
- SURR_TOP_K (→ config: top_k)
- SURR_DB_LIMIT (→ config: retrieval settings)

Keep in .env (secrets only):
- OPENAI_API_KEY
- NOMIC_API_KEY
- GROQ_API_KEY

## 4. Add Config Hot Reload (Optional)

```rust
// Watch config file for changes
use notify::{Watcher, RecursiveMode, watcher};

fn watch_config(config: Arc<RwLock<Config>>) {
    let mut watcher = watcher(Duration::from_secs(2)).unwrap();
    watcher.watch("surreal_mind.toml", RecursiveMode::NonRecursive).unwrap();
    
    loop {
        match rx.recv() {
            Ok(DebouncedEvent::Write(_)) => {
                if let Ok(new_config) = Config::load() {
                    *config.write().unwrap() = new_config;
                    info!("Config reloaded");
                }
            }
            _ => {}
        }
    }
}
```

## Success Criteria
1. All parameters moved from function calls to config
2. Config file is well-organized and documented
3. Submodes have sensible defaults that match their purpose
4. Easy to adjust without recompiling
5. API can now be simplified to just `think(content, mode)`