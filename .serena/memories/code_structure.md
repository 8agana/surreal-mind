# surreal-mind Code Structure

## Directory Layout
```
src/
├── main.rs              # Entry point (MCP server)
├── lib.rs               # Library root, re-exports
├── http.rs              # HTTP transport layer
├── config.rs            # Configuration loading
├── error.rs             # Error types
├── schemas.rs           # JSON schemas for tools
├── embeddings.rs        # OpenAI embedding client
├── bge_embedder.rs      # Candle/BGE local embeddings
├── indexes.rs           # Vector index management
├── serializers.rs       # Custom serialization
├── deserializers.rs     # Custom deserialization
├── bin/                 # Utility binaries
├── server/              # Server implementation
│   ├── mod.rs
│   ├── db.rs            # Database operations
│   ├── router.rs        # MCP tool routing
│   └── schema.rs        # DB schema definitions
├── tools/               # Tool implementations
│   ├── thinking.rs      # legacymind_think
│   ├── unified_search.rs # legacymind_search
│   ├── knowledge_graph.rs # memories_create, kg operations
│   ├── maintenance.rs   # maintenance_ops
│   ├── detailed_help.rs # detailed_help
│   ├── delegate_gemini.rs # delegate_gemini
│   └── curiosity.rs     # curiosity_add/get/search
├── clients/             # External CLI clients
│   ├── gemini.rs        # Gemini CLI wrapper
│   ├── persisted.rs     # Session persistence wrapper
│   └── traits.rs        # CognitiveAgent trait
├── cognitive/           # Cognitive processing
│   └── ...              # Thinking modes, profiles
└── utils/               # Utilities
    └── ...
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
