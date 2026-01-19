# surreal-mind Code Structure

## Directory Layout
```
src/
├── main.rs              # Entry point (MCP server)
├── lib.rs               # Library root, re-exports
├── http.rs              # HTTP transport layer
├── config.rs            # Configuration loading
├── error.rs             # Error types (SurrealMindError)
├── schemas.rs           # JSON schemas for tools
├── embeddings.rs        # OpenAI embedding client

├── indexes.rs           # Vector index management
├── registry.rs          # Agent job registry
├── serializers.rs       # Custom serialization
├── deserializers.rs     # Custom deserialization
├── bin/                 # Utility binaries
│   ├── smtop.rs         # TUI dashboard
│   ├── remini.rs        # Maintenance orchestrator
│   ├── reembed.rs       # Re-embed thoughts
│   ├── reembed_kg.rs    # Re-embed KG
│   ├── kg_populate.rs   # Populate KG
│   ├── kg_embed.rs      # Embed KG entries
│   ├── gem_rethink.rs   # Process rethink marks
│   └── ...
├── server/              # Server implementation
│   ├── mod.rs
│   ├── db.rs            # Database operations
│   ├── router.rs        # MCP tool routing
│   └── schema.rs        # DB schema definitions
├── tools/               # Tool implementations
│   ├── thinking.rs      # think tool
│   ├── unified_search.rs # search tool
│   ├── knowledge_graph.rs # remember tool, KG operations
│   ├── maintenance.rs   # maintain tool
│   ├── rethink.rs       # rethink tool (mark for correction)
│   ├── corrections.rs   # corrections tool (list events)
│   ├── howto.rs         # howto tool
│   ├── call_gem.rs      # call_gem tool
│   ├── agent_job_status.rs # call_status tool
│   ├── list_agent_jobs.rs # call_jobs tool
│   └── cancel_agent_job.rs # call_cancel tool
├── clients/             # External CLI clients
│   ├── gemini.rs        # Gemini CLI wrapper
│   ├── persisted.rs     # Session persistence wrapper
│   └── traits.rs        # CognitiveAgent trait
├── cognitive/           # Cognitive processing
│   ├── mod.rs
│   └── ...              # Thinking modes, profiles, memory injection
└── utils/               # Utilities
```

## Key Patterns

### Tool Implementation
Each tool in `src/tools/` follows the pattern:
1. Define `*Params` struct with serde
2. Implement `handle_*` method on `SurrealMindServer`
3. Add schema function in `src/schemas.rs`
4. Register in `src/server/router.rs`

### Client Wrapper Pattern
`src/clients/` wraps CLI tools (like Gemini) with:
- `CognitiveAgent` trait for uniform interface
- `PersistedAgent` wrapper for session tracking in DB

### Error Handling
- Custom `SurrealMindError` enum in `src/error.rs`
- Variants: Mcp, InvalidParams, Database, Embedding, Serialization, Internal, Timeout
- Use `Result<T>` alias from error.rs
