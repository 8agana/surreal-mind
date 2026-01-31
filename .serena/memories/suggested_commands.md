# Suggested Commands: SurrealMind

## Development
- `cargo build --release`: Build all binaries.
- `cargo fmt --all`: Format code.
- `cargo clippy --workspace --all-targets -- -D warnings`: Lint check.

## Testing
- `cargo test --workspace`: Run all unit tests.
- `cargo test --test tool_schemas`: Validate API contracts.
- `./tests/test_mcp_comprehensive.sh`: Comprehensive end-to-end MCP smoke test.

## Maintenance Binaries
- `./target/release/kg_populate`: Populate KG from thoughts.
- `./target/release/kg_embed`: Generate embeddings for new KG entries.
- `./target/release/gem_rethink`: Process correction marks.
- `./target/release/kg_wander`: Autonomous semantic exploration and gardening.
- `./target/release/reembed`: Handle dimension migration for thoughts.
- `./target/release/reembed_kg`: Handle dimension migration for KG.

## System Health
- `scripts/sm_health.sh`: Check system health (SurrealDB, binaries, etc.).
