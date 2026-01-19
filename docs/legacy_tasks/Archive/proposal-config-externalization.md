# SurrealMind Configuration Externalization Proposal

## Executive Summary

This proposal outlines a comprehensive plan to externalize all configuration in the SurrealMind MCP server, enabling runtime tuning without code changes. The plan identifies 80+ configurable items currently hardcoded or scattered across environment variables, and proposes a structured TOML-based configuration system that maintains full backward compatibility with existing environment variables.

### Key Principles
- **Environment Variables First**: All existing env vars remain supported and take precedence
- **Zero Behavior Change**: Default configuration exactly matches current behavior
- **Modular Organization**: Logical separation into 9 domain-specific TOML files
- **Security-First**: Secrets remain in environment variables only
- **Local-First**: No external dependencies, Docker-free, local embedder focus

### Non-Negotiable Constraints
1. Environment variables must remain the primary configuration method
2. No Docker dependencies - local-first architecture only
3. Local embedder (BGE with 384 dimensions) as the default
4. All changes must pass `make ci` with zero warnings
5. No external network calls in tests

## Inventory of Configurable Items

### Database Configuration (17 items)

| Key | Description | Type | Default | Current Source | Env Var | Secret |
|-----|-------------|------|---------|----------------|---------|--------|
| db.url | SurrealDB connection URL | string | 127.0.0.1:8000 | env/code | SURR_DB_URL | no |
| db.namespace | Database namespace | string | surreal_mind | env/code | SURR_DB_NS | no |
| db.database | Database name | string | consciousness | env/code | SURR_DB_DB | no |
| db.user | Database username | string | root | env | SURR_DB_USER | yes |
| db.password | Database password | string | root | env | SURR_DB_PASS | yes |
| db.query_limit | Max results per query | usize | 500 | env/code | SURR_DB_LIMIT | no |
| db.timeout_ms | Query timeout | u64 | 10000 | env/code | SURR_DB_TIMEOUT_MS | no |
| db.max_concurrency | Max concurrent operations | usize | 1 | code | SURR_DB_MAX_CONCURRENCY | no |
| db.operation_timeout_ms | Operation timeout | u64 | 5000 | code | SURR_OPERATION_TIMEOUT_MS | no |
| db.http_timeout_sec | HTTP client timeout | u64 | 20 | code | - | no |
| db.serialize_queries | Force serial execution | bool | false | code | SURR_DB_SERIAL | no |
| db.schema_init_enabled | Auto-initialize schema | bool | true | code | - | no |
| db.retention_days | Data retention period | i64 | 30 | env/code | SURR_RETENTION_DAYS | no |
| db.warm_cache_batch | Cache warming batch size | usize | 64 | env/code | SURR_CACHE_WARM | no |
| db.retrieve_candidates | Max candidates for similarity | usize | 500 | env/code | SURR_RETRIEVE_CANDIDATES | no |
| db.ws_normalize | Strip ws:// prefix | bool | true | code | - | no |
| db.http_normalize | Add http:// prefix | bool | true | code | - | no |

### Embedding Configuration (15 items)

| Key | Description | Type | Default | Current Source | Env Var | Secret |
|-----|-------------|------|---------|----------------|---------|--------|
| embeddings.provider | Provider (local/openai/nomic/fake) | string | local | env/toml | SURR_EMBED_PROVIDER | no |
| embeddings.model | Model name/path | string | BAAI/bge-small-en-v1.5 | env/toml | SURR_EMBED_MODEL | no |
| embeddings.dimensions | Embedding dimensions | usize | 384 | toml/code | SURR_EMBED_DIM | no |
| embeddings.openai_api_key | OpenAI API key | string | - | env | OPENAI_API_KEY | yes |
| embeddings.nomic_api_key | Nomic API key | string | - | env | NOMIC_API_KEY | yes |
| embeddings.max_retries | Retry attempts | u32 | 3 | env/code | SURR_EMBED_RETRIES | no |
| embeddings.retry_delay_ms | Base retry delay | u64 | 500 | env/code | SURR_RETRY_DELAY_MS | no |
| embeddings.strict_mode | Error if no provider | bool | false | env/code | SURR_EMBED_STRICT | no |
| embeddings.fake_noise | Enable test noise | bool | false | env/code | SURR_FAKE_NOISE | no |
| embeddings.fake_noise_amplitude | Noise amplitude | f32 | 0.01 | env/code | SURR_FAKE_NOISE_AMP | no |
| embeddings.batch_size | Embedding batch size | usize | 100 | code | - | no |
| embeddings.local_model_path | Local model directory | string | models/bge-small-en-v1.5 | code | - | no |
| embeddings.cache_embeddings | Cache computed embeddings | bool | true | code | - | no |
| embeddings.normalize_vectors | L2 normalize embeddings | bool | true | code | - | no |
| embeddings.pooling_strategy | Pooling (mean/cls) | string | mean | code | - | no |

### Cache Configuration (7 items)

| Key | Description | Type | Default | Current Source | Env Var | Secret |
|-----|-------------|------|---------|----------------|---------|--------|
| cache.max_entries | LRU cache capacity | usize | 5000 | env/code | SURR_CACHE_MAX | no |
| cache.warm_batch_size | Batch size for warming | usize | 64 | env/code | SURR_CACHE_WARM | no |
| cache.ttl_seconds | Entry time-to-live | u64 | 3600 | code | - | no |
| cache.enable_metrics | Track cache metrics | bool | false | code | - | no |
| cache.eviction_policy | Eviction strategy | string | lru | code | - | no |
| cache.shard_count | Number of shards | usize | 1 | code | - | no |
| cache.compression | Enable compression | bool | false | code | - | no |

### Search & Retrieval Configuration (11 items)

| Key | Description | Type | Default | Current Source | Env Var | Secret |
|-----|-------------|------|---------|----------------|---------|--------|
| search.similarity_threshold | Min similarity score | f32 | 0.5 | env/toml | SURR_SIM_THRESH | no |
| search.top_k | Default result count | usize | 5 | env/toml | SURR_TOP_K | no |
| search.max_injection_scale | Max memory injection | u8 | 3 | toml | - | no |
| search.default_injection_scale | Default injection | u8 | 1 | toml | - | no |
| search.submode_retrieval | Enable submode tuning | bool | false | env/toml | SURR_SUBMODE_RETRIEVAL | no |
| search.default_submode | Default thinking mode | string | build | toml | SURR_SUBMODE_DEFAULT | no |
| search.kg_only | Only use knowledge graph | bool | true | toml | - | no |
| search.expand_graph | Auto-expand KG results | bool | false | code | - | no |
| search.graph_depth | KG traversal depth | u8 | 2 | code | - | no |
| search.min_edge_strength | Min edge weight | f32 | 0.0 | env/code | SURR_KG_MIN_EDGE_STRENGTH | no |
| search.graph_boost | Graph neighbor boost | f32 | 0.15 | env/code | SURR_KG_GRAPH_BOOST | no |

### Knowledge Graph Configuration (10 items)

| Key | Description | Type | Default | Current Source | Env Var | Secret |
|-----|-------------|------|---------|----------------|---------|--------|
| kg.embed_entities | Embed entity names | bool | true | env/code | SURR_KG_EMBED_ENTITIES | no |
| kg.embed_observations | Embed observations | bool | true | env/code | SURR_KG_EMBED_OBSERVATIONS | no |
| kg.max_neighbors | Max neighbors to fetch | usize | 25 | env/code | SURR_KG_MAX_NEIGHBORS | no |
| kg.timeout_ms | Operation timeout | u64 | 5000 | env/code | SURR_KG_TIMEOUT_MS | no |
| kg.candidates | Max expansion candidates | usize | 200 | env/code | SURR_KG_CANDIDATES | no |
| kg.auto_extract | Auto-extract from thoughts | bool | true | toml | - | no |
| kg.min_confidence | Min extraction confidence | f32 | 0.6 | code | - | no |
| kg.dedup_threshold | Deduplication similarity | f32 | 0.9 | code | - | no |
| kg.approval_required | Require manual approval | bool | false | code | - | no |
| kg.max_edges_per_node | Max edges per entity | usize | 100 | code | - | no |

### Server Configuration (8 items)

| Key | Description | Type | Default | Current Source | Env Var | Secret |
|-----|-------------|------|---------|----------------|---------|--------|
| server.transport | Transport type | string | stdio | code | - | no |
| server.tool_timeout_ms | Tool call timeout | u64 | 15000 | env/code | SURR_TOOL_TIMEOUT_MS | no |
| server.request_buffer_size | Request buffer | usize | 65536 | code | - | no |
| server.response_buffer_size | Response buffer | usize | 65536 | code | - | no |
| server.max_concurrent_tools | Max parallel tools | usize | 10 | code | - | no |
| server.graceful_shutdown_ms | Shutdown timeout | u64 | 5000 | code | - | no |
| server.enable_health_check | Health endpoint | bool | false | code | - | no |
| server.metrics_enabled | Enable metrics | bool | false | code | - | no |

### Logging Configuration (6 items)

| Key | Description | Type | Default | Current Source | Env Var | Secret |
|-----|-------------|------|---------|----------------|---------|--------|
| logging.level | Default log level | string | info | env | RUST_LOG | no |
| logging.format | Output format | string | plain | code | - | no |
| logging.ansi_colors | Enable colors | bool | false | code | - | no |
| logging.timestamps | Include timestamps | bool | true | code | - | no |
| logging.mcp_stderr_disabled | Disable MCP stderr | bool | false | env | MCP_NO_LOG | no |
| logging.file_output | Log file path | string | - | code | - | no |

### Tool Configuration (13 items)

| Key | Description | Type | Default | Current Source | Env Var | Secret |
|-----|-------------|------|---------|----------------|---------|--------|
| tools.convo_think.enabled | Enable convo_think | bool | true | code | - | no |
| tools.convo_think.default_significance | Default significance | f32 | 0.5 | code | - | no |
| tools.tech_think.enabled | Enable tech_think | bool | true | code | - | no |
| tools.tech_think.default_significance | Default significance | f32 | 0.5 | code | - | no |
| tools.inner_voice.enabled | Enable inner_voice | bool | true | code | - | no |
| tools.inner_voice.default_visibility | Default visibility | string | private | code | - | no |
| tools.maintenance.enabled | Enable maintenance | bool | true | code | - | no |
| tools.maintenance.archive_format | Archive format | string | parquet | code | - | no |
| tools.maintenance.archive_dir | Archive directory | string | ./archive | code | - | no |
| tools.kg_tools.enabled | Enable KG tools | bool | true | code | - | no |
| tools.kg_tools.review_limit | Review batch size | usize | 50 | code | - | no |
| tools.search.enabled | Enable search | bool | true | code | - | no |
| tools.detailed_help.enabled | Enable help | bool | true | code | - | no |

### Cognitive Framework Configuration (per-submode, 6 submodes × 7 params = 42 items)

For each submode (plan, build, debug, sarcastic, empathetic, philosophical):

| Key Pattern | Description | Type | Default | Source |
|-------------|-------------|------|---------|--------|
| cognitive.{mode}.injection_scale | Memory injection scale | u8 | varies | toml |
| cognitive.{mode}.significance | Default significance | f32 | varies | toml |
| cognitive.{mode}.kg_traverse_depth | Graph depth | u8 | varies | toml |
| cognitive.{mode}.frameworks | Active frameworks | map | varies | toml |
| cognitive.{mode}.orbital_weights | Weight distribution | map | varies | toml |
| cognitive.{mode}.auto_extract | Auto KG extraction | bool | varies | toml |
| cognitive.{mode}.edge_boosts | Edge type boosts | map | varies | toml |

### Reembedding Configuration (6 items)

| Key | Description | Type | Default | Current Source | Env Var | Secret |
|-----|-------------|------|---------|----------------|---------|--------|
| reembed.batch_size | Processing batch | usize | 100 | code | - | no |
| reembed.max_limit | Max records | usize | unlimited | code | - | no |
| reembed.missing_only | Only missing embeds | bool | false | code | - | no |
| reembed.dry_run | Simulation mode | bool | false | code | - | no |
| reembed.target_dimensions | Expected dimensions | usize | 384 | code | - | no |
| reembed.parallel_workers | Worker threads | usize | 1 | code | - | no |

## Proposed Configuration File Structure

```
surreal-mind/
├── .env                    # Secrets and overrides (not committed)
├── .env.example           # Template with all supported env vars
├── surreal_mind.toml      # Main config (existing, enhanced)
└── config/                # Modular configuration directory
    ├── db.toml           # Database settings
    ├── embeddings.toml   # Embedding provider settings
    ├── cache.toml        # Cache configuration
    ├── search.toml       # Search and retrieval
    ├── kg.toml           # Knowledge graph settings
    ├── server.toml       # Server and transport
    ├── logging.toml      # Logging configuration
    ├── tools.toml        # Tool-specific settings
    ├── cognitive.toml    # Cognitive framework/submodes
    └── reembed.toml      # Reembedding job settings
```

### Configuration Precedence (Highest to Lowest)

1. **Environment Variables** - Always win, enable runtime overrides
2. **TOML Files** - Structured defaults, version controlled
3. **Compiled Defaults** - Fallback safety, guaranteed to work

### Rationale for Structure

- **Separation of Concerns**: Each file focuses on one domain
- **Security**: Secrets never in TOML, only environment variables
- **Flexibility**: Mix and match configs, partial overrides supported
- **Maintainability**: Clear ownership, easier to document
- **Testing**: Can swap entire config domains for testing

## Backward Compatibility

### Environment Variable Mapping

All existing environment variables continue to work exactly as before:

| Current Env Var | Maps To Config Key | Notes |
|-----------------|-------------------|-------|
| SURR_DB_URL | db.url | No change |
| SURR_DB_NS | db.namespace | No change |
| SURR_DB_DB | db.database | No change |
| SURR_DB_USER | db.user | Secret, env-only |
| SURR_DB_PASS | db.password | Secret, env-only |
| SURR_DB_LIMIT | db.query_limit | No change |
| SURR_EMBED_PROVIDER | embeddings.provider | No change |
| OPENAI_API_KEY | embeddings.openai_api_key | Secret, env-only |
| NOMIC_API_KEY | embeddings.nomic_api_key | Secret, env-only |
| SURR_CACHE_MAX | cache.max_entries | No change |
| SURR_TOP_K | search.top_k | No change |
| SURR_SIM_THRESH | search.similarity_threshold | No change |
| RUST_LOG | logging.level | No change |
| MCP_NO_LOG | logging.mcp_stderr_disabled | No change |

### Deprecation Strategy

- **Phase 1**: Add TOML support, env vars continue to override
- **Phase 2**: Log info message when env var overrides TOML
- **Phase 3**: (Optional, 6+ months) Deprecation warnings for renamed vars
- **Never**: Remove support for critical env vars (DB credentials, API keys)

## Migration Plan

### Phase 1: Add Configuration System (Week 1)
1. Add `config` crate dependency
2. Implement configuration loader with precedence
3. Create example TOML files in `config/examples/`
4. Update `.env.example` with complete documentation
5. No behavior changes - all tests pass

### Phase 2: Enable TOML Loading (Week 2)
1. Load TOML files if present, env vars still override
2. Add debug logging for configuration sources
3. Ship example configurations
4. Update documentation

### Phase 3: Developer Adoption (Week 3)
1. Team copies `config/examples/` to `config/`
2. Customize settings as needed
3. Verify with `RUST_LOG=debug cargo run`
4. Report any issues

### Phase 4: Production Rollout (Week 4)
1. Update deployment scripts if needed
2. Ensure env vars still set for secrets
3. Monitor for configuration issues
4. Document any gotchas

### Rollback Plan
- Delete `config/` directory
- All env vars continue to work
- Zero code changes required
- Full backward compatibility maintained

## Example Configuration Files

### config/db.toml
```toml
# SurrealDB Configuration
# Secrets should remain in environment variables

[connection]
url = "127.0.0.1:8000"  # Override with SURR_DB_URL
namespace = "surreal_mind"
database = "consciousness"
# user = "${SURR_DB_USER}"  # Keep in .env
# password = "${SURR_DB_PASS}"  # Keep in .env

[performance]
query_limit = 500
timeout_ms = 10000
max_concurrency = 1
operation_timeout_ms = 5000
http_timeout_sec = 20

[caching]
warm_batch_size = 64
retrieve_candidates = 500

[maintenance]
retention_days = 30
serialize_queries = false

[schema]
auto_initialize = true
```

### config/embeddings.toml
```toml
# Embedding Provider Configuration

[provider]
type = "local"  # local | openai | nomic | fake
model = "BAAI/bge-small-en-v1.5"
dimensions = 384

[local]
model_path = "models/bge-small-en-v1.5"
cache_embeddings = true
normalize_vectors = true
pooling_strategy = "mean"  # mean | cls

[api]
# Keys in environment variables only
max_retries = 3
retry_delay_ms = 500
strict_mode = false

[testing]
fake_noise = false
fake_noise_amplitude = 0.01

[performance]
batch_size = 100
```

### config/cache.toml
```toml
# In-Memory Cache Configuration

[lru]
max_entries = 5000
ttl_seconds = 3600
eviction_policy = "lru"  # lru | lfu | fifo

[performance]
warm_batch_size = 64
shard_count = 1
compression = false

[monitoring]
enable_metrics = false
```

### config/search.toml
```toml
# Search and Retrieval Configuration

[defaults]
similarity_threshold = 0.5
top_k = 5
max_injection_scale = 3
default_injection_scale = 1

[modes]
submode_retrieval = false
default_submode = "build"
kg_only = true

[graph]
expand_graph = false
traversal_depth = 2
min_edge_strength = 0.0
neighbor_boost = 0.15
```

### config/kg.toml
```toml
# Knowledge Graph Configuration

[embeddings]
embed_entities = true
embed_observations = true

[retrieval]
max_neighbors = 25
timeout_ms = 5000
candidates = 200

[extraction]
auto_extract = true
min_confidence = 0.6
dedup_threshold = 0.9

[governance]
approval_required = false
max_edges_per_node = 100
```

### config/server.toml
```toml
# MCP Server Configuration

[transport]
type = "stdio"
request_buffer_size = 65536
response_buffer_size = 65536

[execution]
tool_timeout_ms = 15000
max_concurrent_tools = 10
graceful_shutdown_ms = 5000

[monitoring]
enable_health_check = false
metrics_enabled = false
```

### config/logging.toml
```toml
# Logging Configuration

[output]
level = "info"  # trace | debug | info | warn | error
format = "plain"  # plain | json
ansi_colors = false
timestamps = true
file_output = ""  # Optional log file path

[mcp]
stderr_disabled = false  # Override with MCP_NO_LOG=true

[filters]
# Module-specific levels
modules = [
    "surreal_mind=info",
    "rmcp=info",
    "surrealdb=warn"
]
```

### config/tools.toml
```toml
# Tool Configuration

[convo_think]
enabled = true
default_significance = 0.5

[tech_think]
enabled = true
default_significance = 0.5

[inner_voice]
enabled = true
default_visibility = "private"

[maintenance]
enabled = true
archive_format = "parquet"
archive_dir = "./archive"

[kg_tools]
enabled = true
review_limit = 50

[search]
enabled = true

[detailed_help]
enabled = true
```

### config/cognitive.toml
```toml
# Cognitive Framework Configuration
# Orbital mechanics and thinking modes

[orbital]
decay_rate = 0.1
access_boost = 0.2
significance_weight = 0.3
recency_weight = 0.4
access_weight = 0.3

[submodes.plan]
injection_scale = 3
significance = 0.8
kg_traverse_depth = 2
auto_extract = true

[submodes.plan.frameworks]
SystemsThinking = 0.6
FirstPrinciples = 0.4

[submodes.plan.orbital_weights]
recency = 0.2
access = 0.3
significance = 0.5

[submodes.plan.edge_boosts]
depends_on = 1.5
blocks = 1.3
relates_to = 1.1

[submodes.build]
injection_scale = 1
significance = 0.5
kg_traverse_depth = 1
auto_extract = false

[submodes.build.frameworks]
OODA = 0.8
Lateral = 0.2

[submodes.build.orbital_weights]
recency = 0.7
access = 0.2
significance = 0.1

[submodes.build.edge_boosts]
implements = 1.5
uses = 1.3

[submodes.debug]
injection_scale = 1
significance = 0.9
kg_traverse_depth = 1
auto_extract = true

[submodes.debug.frameworks]
RootCause = 0.7
OODA = 0.3

[submodes.debug.orbital_weights]
recency = 0.8
access = 0.1
significance = 0.1

[submodes.debug.edge_boosts]
causes = 2.0
fixes = 1.8
related_to = 1.2
```

### config/reembed.toml
```toml
# Reembedding Job Configuration

[execution]
batch_size = 100
max_limit = 0  # 0 = unlimited
missing_only = false
dry_run = false

[embeddings]
target_dimensions = 384
parallel_workers = 1
```

## Implementation Recommendations

### Configuration Loader Pattern

```rust
use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize)]
pub struct AppConfig {
    pub db: DatabaseConfig,
    pub embeddings: EmbeddingConfig,
    pub cache: CacheConfig,
    pub search: SearchConfig,
    pub kg: KnowledgeGraphConfig,
    pub server: ServerConfig,
    pub logging: LoggingConfig,
    pub tools: ToolsConfig,
    pub cognitive: CognitiveConfig,
    pub reembed: ReembedConfig,
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let mut builder = Config::builder();
        
        // 1. Start with compiled defaults
        builder = builder.set_default("db.url", "127.0.0.1:8000")?;
        builder = builder.set_default("embeddings.dimensions", 384)?;
        // ... more defaults
        
        // 2. Merge main config file if exists
        if Path::new("surreal_mind.toml").exists() {
            builder = builder.add_source(File::with_name("surreal_mind.toml"));
        }
        
        // 3. Merge modular config files if directory exists
        if Path::new("config").is_dir() {
            for entry in ["db", "embeddings", "cache", "search", "kg", 
                         "server", "logging", "tools", "cognitive", "reembed"] {
                let path = format!("config/{}.toml", entry);
                if Path::new(&path).exists() {
                    builder = builder.add_source(
                        File::with_name(&path).required(false)
                    );
                }
            }
        }
        
        // 4. Override with environment variables (highest precedence)
        builder = builder.add_source(
            Environment::with_prefix("SURR")
                .separator("_")
                .try_parsing(true)
        );
        
        // Special handling for non-SURR prefixed vars
        if let Ok(val) = env::var("OPENAI_API_KEY") {
            builder = builder.set_override("embeddings.openai_api_key", val)?;
        }
        if let Ok(val) = env::var("NOMIC_API_KEY") {
            builder = builder.set_override("embeddings.nomic_api_key", val)?;
        }
        if let Ok(val) = env::var("RUST_LOG") {
            builder = builder.set_override("logging.level", val)?;
        }
        if let Ok(val) = env::var("MCP_NO_LOG") {
            builder = builder.set_override("logging.mcp_stderr_disabled", val)?;
        }
        
        // 5. Build and validate
        let config = builder.build()?;
        let app_config: AppConfig = config.try_deserialize()?;
        
        // Custom validation
        app_config.validate()?;
        
        Ok(app_config)
    }
    
    fn validate(&self) -> Result<(), ConfigError> {
        // Ensure required fields are set
        if self.embeddings.dimensions != 384 && self.embeddings.provider == "local" {
            return Err(ConfigError::Message(
                "Local embedder must use 384 dimensions".into()
            ));
        }
        
        // More validation...
        Ok(())
    }
}
```

### Testing Strategy

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;
    
    #[test]
    fn test_env_var_precedence() {
        env::set_var("SURR_DB_URL", "test.db:9999");
        let config = AppConfig::load().unwrap();
        assert_eq!(config.db.url, "test.db:9999");
        env::remove_var("SURR_DB_URL");
    }
    
    #[test]
    fn test_toml_loading() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config");
        std::fs::create_dir(&config_path).unwrap();
        
        let db_toml = r#"
            [connection]
            url = "custom.db:8888"
        "#;
        std::fs::write(config_path.join("db.toml"), db_toml).unwrap();
        
        env::set_var("SURREAL_MIND_CONFIG_DIR", config_path.to_str().unwrap());
        let config = AppConfig::load().unwrap();
        assert_eq!(config.db.url, "custom.db:8888");
    }
    
    #[test]
    fn test_default_fallback() {
        // No env vars, no config files
        let config = AppConfig::load().unwrap();
        assert_eq!(config.embeddings.dimensions, 384);
        assert_eq!(config.cache.max_entries, 5000);
    }
}
```

### CI Integration

Update `.github/workflows/ci.yml`:

```yaml
- name: Validate Configuration
  run: |
    # Test with default config
    cargo test --test config_tests
    
    # Test with example configs
    cp -r config/examples config/
    cargo test --test config_tests
    
    # Test with env var overrides
    SURR_DB_URL=test.db cargo test --test config_tests
    
    # Ensure no behavior changes
    cargo test --all
```

### Makefile Updates

```makefile
# Configuration tasks
config-validate:
	@echo "Validating configuration..."
	@cargo run --bin validate-config

config-generate:
	@echo "Generating example configs..."
	@cargo run --bin generate-configs

config-diff:
	@echo "Showing config changes..."
	@diff -u config/examples/ config/ || true

ci: check fmt clippy test config-validate
	@echo "CI pipeline complete"
```

## Future Enhancements

### Phase 2 Features (3-6 months)
- **Hot Reload**: Watch config files, reload without restart
- **Config Profiles**: Development/staging/production profiles  
- **Validation CLI**: Standalone config validator tool
- **Migration Tool**: Auto-convert env vars to TOML
- **Web UI**: Configuration dashboard (optional)

### Phase 3 Features (6-12 months)
- **Distributed Config**: Consul/etcd integration (optional)
- **A/B Testing**: Multiple config variants
- **Feature Flags**: Dynamic feature toggling
- **Config History**: Track changes over time
- **Performance Profiles**: Optimized configs for different workloads

## Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| Breaking changes | High | Full env var compatibility, extensive testing |
| Config complexity | Medium | Clear documentation, examples, validation |
| Performance impact | Low | Lazy loading, caching parsed configs |
| Security leaks | High | Secrets in env only, validate no secrets in TOML |
| Migration issues | Medium | Phased rollout, rollback plan, monitoring |

## Success Metrics

1. **Zero Breaking Changes**: All existing deployments continue to work
2. **100% Env Var Coverage**: Every env var still supported
3. **Configuration Time**: < 100ms to load all configs
4. **Test Coverage**: > 90% coverage for config module
5. **Documentation**: Every setting documented with examples

## Conclusion

This configuration externalization plan provides a comprehensive, backward-compatible path to making SurrealMind fully configurable without code changes. The phased approach ensures zero disruption while enabling powerful new capabilities for tuning and customization.

The system maintains the principle of "environment variables first" while adding structured configuration files for better organization and documentation. With 80+ configurable items identified and organized into logical domains, operators will have complete control over the system's behavior.

### Next Steps

1. Review and approve this proposal
2. Create feature branch for implementation
3. Implement Phase 1 (configuration system)
4. Test with team in development
5. Roll out to production with monitoring

### Questions for Review

1. Are there additional configuration items to consider?
2. Should we use `figment` instead of `config` crate?
3. Do we need config encryption for sensitive non-secret data?
4. Should we add config versioning/schema migration?
5. Any concerns about the proposed directory structure?

---

*Document Version: 1.0*  
*Date: 2025-01-02*  
*Author: SurrealMind Configuration Team*  
*Status: PROPOSAL - Awaiting Review*
