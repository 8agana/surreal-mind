# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

Project overview
- Rust MCP server implementing consciousness persistence with orbital mechanics for memory retrieval
- Entry point: src/main.rs; Embedding module: src/embeddings.rs
- Storage: SurrealDB service (WebSocket) at SURR_DB_URL (default 127.0.0.1:8000) with namespace SURR_DB_NS (default "surreal_mind") and database SURR_DB_DB (default "consciousness")
  - Env: SURR_DB_USER/SURR_DB_PASS for auth; SURR_DB_LIMIT caps fallback SELECT size; SURR_SIM_THRESH controls similarity cutoff; SURR_TOP_K controls top results returned
  - NOW FULLY WIRED: Thoughts persist to DB, bidirectional relationships created via `recalls` table
  - Hybrid approach: In-memory Vec<Thought> for fast retrieval + SurrealDB for persistence
- Exposes MCP tools:
  - convo_think: Store thoughts with bidirectional memory injection + framework analysis
  - tech_think: Technical reasoning with submodes (plan|build|debug)
  - detailed_help: Deterministic, example-rich docs for tools/params

Common commands
- Build
  - Debug: cargo build
  - Release: cargo build --release  → binary at target/release/surreal-mind
- Run (stdio MCP server)
  - Default: cargo run (loads .env automatically via dotenv)
  - With custom logs: RUST_LOG=surreal_mind=debug,rmcp=info cargo run
- Format and lint
  - Format: make fmt  (cargo fmt --all)
  - Lint: make lint  (cargo clippy -- -D warnings)
  - All checks: make ci  (runs check, fmt --check, clippy -D warnings, tests)
- Tests
  - All: cargo test --all  
  - Single test: cargo test test_list_tools_returns_convo_think
  - With logs: RUST_LOG=debug cargo test -- --nocapture
- Environment setup
  - Copy .env.example to .env (if exists) or create .env with NOMIC_API_KEY=your-key
  - Without API key: Falls back to fake embeddings for testing
- Cargo aliases (defined in .cargo/config.toml)
  - cargo lint → clippy with -D warnings
  - cargo ci → composite check, fmt, clippy, test

High-level architecture  
- Server structure
  - SurrealMindServer implements rmcp::handler::server::ServerHandler
  - State: db (Arc<RwLock<Surreal<Client>>>), thoughts cache (Arc<RwLock<LruCache<String, Thought>>>), embedder (Arc<dyn Embedder>)
  - Transport: stdio with serve_server(server, stdio())
  - Logging: tracing + tracing-subscriber, respects RUST_LOG env var
- Database layer
  - initialize_schema() defines SCHEMAFULL tables:
    - thoughts: Stores id, content, created_at, embedding (array<float>), injected_memories, enriched_content, injection_scale, significance, access_count, last_accessed, submode, framework_enhanced, framework_analysis
    - recalls: Graph edges with in/out (record<thoughts>), strength, created_at, submode_match, flavor
  - Indexes on created_at and significance for efficient queries
  - Hybrid storage: DB writes + in-memory cache for performance
- Embedding system (src/embeddings.rs)
  - Trait-based: Embedder trait with embed() and dimensions()
  - NomicEmbedder: Real 768-dim embeddings via Nomic API (requires NOMIC_API_KEY)
  - FakeEmbedder: Deterministic, normalized fallback when no API key (cosine meaningful)
  - Factory: create_embedder() auto-selects based on environment
- Tool pipeline (convo_think)
  - Generate real/fake embedding based on NOMIC_API_KEY presence
  - Retrieve memories using orbital mechanics (injection_scale 0-5):
    - 0: No injection  
    - 1: Mercury (0.2 distance) - hot memories only
    - 3: Mars (0.6 distance) - foundational [default]
    - 5: Pluto (1.0 distance) - everything relevant
  - Calculate orbital_proximity: 40% age + 30% access + 30% significance
  - Store thought in SurrealDB with CREATE syntax
  - Create bidirectional recalls relationships via RELATE queries
  - Return structured JSON: thought_id, submode_used, memories_injected, analysis (concise, conversational)
- Retrieval strategy
  - Primary: Check in-memory thoughts vector first
  - Fallback: Query SurrealDB (selected columns only) if cache empty, then cache top results
  - Cosine similarity threshold: 0.5
  - Combined scoring: 60% similarity + 40% orbital_proximity
  - Returns top 5 matches sorted by combined score

Conventions specific to this repo
- Environment: Always check for .env file; dotenv::dotenv().ok() in main()
- Error handling: anyhow::Result for main paths, McpError for MCP protocol  
- Testing: Tests currently use fake server instances (no embedder), need mock updates
- CI compliance: Must pass make ci before commits (fmt, clippy -D warnings, tests)
- Production: Build with --release, ensure NOMIC_API_KEY set for real embeddings
