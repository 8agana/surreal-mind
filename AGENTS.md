# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

```json
{
  "execution_context": {
    "directory_state": {
      "pwd": "/Users/samuelatagana/Projects/LegacyMind/surreal-mind",
      "home": "/Users/samuelatagana"
    },
    "operating_system": {
      "platform": "MacOS"
    },
    "current_time": "2025-08-23T23:40:34Z",
    "shell": {
      "name": "zsh",
      "version": "5.9"
    }
  }
}
```

Project overview
- Single-crate Rust MCP server built with rmcp and tokio. Entry point: src/main.rs
- Storage: SurrealDB embedded (RocksDB backend) initialized at ./surreal_data with ns "surreal_mind" and db "consciousness"; current thought persistence is in-memory (Vec<Thought>) with a TODO to wire DB writes/reads
- Exposes one MCP tool: convo_think (stores a thought and injects relevant memories into enriched content)

Common commands
- Build
  - Debug: cargo build
  - Release: cargo build --release  → binary at target/release/surreal-mind
- Run (stdio MCP server)
  - Default logs: cargo run
  - Verbose logs: RUST_LOG=surreal_mind=debug,rmcp=info cargo run
- Format and lint
  - Format: make fmt  (cargo fmt --all)
  - Lint: make lint  (cargo clippy -- -D warnings)
  - All checks locally: make ci  (cargo check, fmt --check, clippy -D warnings, tests)
- Tests
  - All: cargo test --all
  - Single test by name: cargo test test_list_tools_returns_convo_think
  - With logs: RUST_LOG=debug cargo test -- --nocapture
- One-time setup (if needed): rustup component add rustfmt clippy
- Note: Docker is not used in this repo

High-level architecture
- Server runtime and transport
  - src/main.rs implements rmcp::handler::server::ServerHandler for SurrealMindServer
  - Transport is stdio(); serve_server(server, stdio()) drives request handling
  - Tracing via tracing + tracing-subscriber with env filter surreal_mind=debug,rmcp=info
- State and concurrency
  - SurrealMindServer holds:
    - db: Arc<RwLock<Surreal<Db>>> → SurrealDB (RocksDB) created at ./surreal_data
    - thoughts: Arc<RwLock<Vec<Thought>>> → current in-memory store of thoughts
  - Tokio async + RwLock provides concurrent, async-safe access
- Tool surface
  - list_tools declares one Tool named "convo_think" with a JSON Schema input describing parameters:
    - content: string (required)
    - injection_scale: integer [0..5] (optional)
    - submode: enum ["sarcastic","philosophical","empathetic","problem_solving"] (optional)
    - tags: string[] (optional)
    - significance: number [0.0..1.0] (optional)
  - call_tool routes by name; unknown names return METHOD_NOT_FOUND
- Thought pipeline (convo_think)
  - Build a placeholder 768-dim embedding for the input content
  - Retrieve candidate memories (retrieve_memories_for_injection):
    - Filters by a similarity threshold and an orbital distance cap derived from injection_scale
    - orbital_distance combines age, access_count, and significance; take top 5 by a combined score
  - Enrich content with a compact memory preview (up to three memories) and return structured JSON including:
    - thought_id, memories_injected, enriched_content, injection_scale, orbital_distances[], memory_summary
- Testing approach
  - Inline #[tokio::test] tests in src/main.rs exercise tool listing and structured tool output using rmcp oneshot transport
  - CI runs fmt check, clippy with -D warnings, and cargo test --all --locked (.github/workflows/ci.yml)

Conventions specific to this repo
- Treat clippy warnings as errors (make lint). Keep code formatted (make fmt). Use make ci before pushing for local parity with CI
- Build production binaries with cargo build --release when distributing the MCP server
