# Surreal Mind - Consciousness Persistence MCP Server

A Model Context Protocol (MCP) server implementing bidirectional consciousness persistence with orbital mechanics for memory retrieval.

## Features
- **Bidirectional Memory Injection**: Thoughts automatically pull relevant memories during storage
- **Orbital Mechanics**: Memory relevance based on age, access patterns, and significance
- **Semantic Understanding**: OpenAI text-embedding-3-small (1536 dims) for semantic similarity
- **Graph Persistence**: SurrealDB service for consciousness graph storage
- **Injection Scales**: 0-5 (Sun to Pluto) controlling memory retrieval distance
- **Orbital Mechanics**: Memory relevance based on age, access patterns, and significance (Note: this feature is currently dormant).
- **Semantic Understanding**: OpenAI `text-embedding-3-small` (1536 dims) for semantic similarity, with local fallback via Candle.
- **Graph Persistence**: SurrealDB service for consciousness graph storage.
- **Injection Scales**: 0-5 (Sun to Pluto) controlling memory retrieval distance.

## Setup

### Prerequisites
- Rust 1.85+ (uses edition 2024)
- Cargo

### Environment Variables
1. Copy `.env.example` to `.env`:
   ```bash
   cp .env.example .env
   ```

2. Add your OpenAI API key:
   ```
   OPENAI_API_KEY=sk-...
   ```
Optional: `SURR_EMBED_MODEL=text-embedding-3-small` (default). Provider policy: OpenAI (primary) or Candle/BGE-small-en-v1.5 for local development when no OpenAI key is provided.

### Build
```bash
cargo build --release
```

### Database Setup

The server connects to SurrealDB via WebSocket. You must run SurrealDB as a separate service:

```bash
# For in-memory testing (data lost on restart)
surreal start --user root --pass root --bind 127.0.0.1:8000 memory

# For persistent storage with RocksDB
surreal start --user root --pass root --bind 127.0.0.1:8000 file:/path/to/data.db
```

Note: The server connects via WebSocket only. Embedded in-process DB is not currently supported.

See [DATABASE.md](DATABASE.md) for detailed schema, indexes, and maintenance operations.

## Production Deployment
- Defaults in this repo are for local development (127.0.0.1, http/ws without TLS). Do not use these defaults over a network.
- Use secure transports in production:
  - WebSocket (DB): set SURR_DB_URL to a wss:// endpoint, e.g., `export SURR_DB_URL=wss://db.example.com:8000`
- Credentials: SURR_DB_USER and SURR_DB_PASS should be set securely.
- Logging: Consider setting `MCP_NO_LOG=true` in environments where stderr must remain JSON-only for MCP clients. Use `RUST_LOG=surreal_mind=info,rmcp=info` or quieter.

## Configuration
Configure the server via environment variables:
```bash
# Database Configuration (defaults shown)
export SURR_DB_URL=127.0.0.1:8000
export SURR_DB_USER=root
export SURR_DB_PASS=root
export SURR_DB_NS=surreal_mind
export SURR_DB_DB=consciousness

# Retrieval Tuning
export SURR_SIM_THRESH=0.5  # Similarity threshold for think_search (0.0-1.0)
export SURR_TOP_K=10        # Default number of results for search
export SURR_DB_LIMIT=500    # Candidate limit for DB queries

# Embedding Configuration
export SURR_EMBED_RETRIES=3         # Max retries for embedding API calls
export SURR_EMBED_STRICT=false      # If true, error when no provider configured
```

## Usage

### As MCP Server
```bash
cargo run
# or for release mode:
./target/release/surreal-mind
```

### MCP Tool: `think_*`
The `think_*` family of tools store thoughts with bidirectional memory injection.

**Tools:**
- `think_convo`: For conversational thoughts.
- `think_plan`: For architecture and strategy.
- `think_debug`: For root cause analysis.
- `think_build`: For implementation-focused thinking.
- `think_stuck`: For lateral thinking to unblock progress.

Each tool has different default values for memory injection, but they share the same parameters.

**Parameters:**
- `content` (required): The thought to store.
- `injection_scale`: Overrides the tool's default memory injection level.
  - Accepts numeric values (0-5) or named presets (`"NONE"`, `"LIGHT"`, `"MEDIUM"`, `"DEFAULT"`, `"HIGH"`, `"MAXIMUM"`).
- `significance`: Overrides the tool's default importance weight.
  - Accepts float values (0.0-1.0), an integer scale (2-10, mapped to 0.2-1.0), or named presets (`"low"`, `"medium"`, `"high"`).
- `tags`: Additional categorization tags.

**Example:**
```json
{
  "tool": "think_plan",
  "arguments": {
    "content": "Design module A with clear interfaces",
    "injection_scale": "DEFAULT",
    "significance": "medium"
  }
}
```

**Response:**
```json
{
  "thought_id": "...",
  "memories_injected": 5,
  "embedding_model": "text-embedding-3-small",
  "embedding_dim": 1536
}
```

### MCP Tool: `think_search`
Performs a semantic search over stored thoughts.

**Parameters:**
- `content` (required): The query text.
- `top_k`: Max number of results (default 10).
- `offset`: Pagination offset.
- `sim_thresh`: Minimum similarity score (0.0-1.0).
- `min_significance`: Minimum significance to include.
- `sort_by`: How to sort results (`"score"`, `"similarity"`, `"recency"`, `"significance"`).

**Example:**
```json
{
  "tool": "think_search",
  "arguments": {
    "content": "debug parser issue",
    "top_k": 5,
    "sim_thresh": 0.55
  }
}
```

### MCP Tool: `memories_create`
Creates entities and relationships in the Knowledge Graph (KG).

**Parameters:**
- `kind` (required): `"entity"` | `"relationship"` | `"observation"`.
- `data` (required): The object containing the KG data.
- `upsert`: If true (default), will not create duplicates.

### MCP Tool: `memories_search`
Searches the Knowledge Graph using fuzzy text matching.

**Parameters:**
- `query` (required): An object with search criteria, e.g., `{"name": "AI"}`.
- `target`: `"entity"` | `"relationship"` | `"observation"` | `"mixed"`.
- `top_k`: Max number of results.

### MCP Tool: `memories_moderate`
A tool for reviewing and approving candidate entries for the Knowledge Graph.

### MCP Tool: `maintenance_ops`
Provides subcommands for database health checks, archival, and cleanup.
- `health_check_embeddings`: Verifies embedding dimensions.
- `health_check_indexes`: Verifies database indexes are correctly defined.
- `reembed`: Re-computes embeddings for thoughts.
- `list_removal_candidates`, `export_removals`, `finalize_removal`: An archival workflow.

### MCP Tool: `detailed_help`
Returns structured documentation for tools and their parameters.

## Available Tools and Binaries

This project includes:

### Main MCP Server Binary
- `cargo run` or `./target/release/surreal-mind`: Starts the MCP server.

### Additional Binaries (src/bin/)
- `cargo run --bin reembed`: CLI for re-embedding thoughts.
- `cargo run --bin check_db_contents`: Utility to inspect DB contents.
- `cargo run --bin db_check`: DB connectivity test.
- `cargo run --bin simple_db_test`: Basic DB operations test.

## Prompt Registry (Self-aware prompts)

SurrealMind includes a self-aware Prompt Registry that documents the system's cognitive patterns as first-class, versioned entities.
This enables prompt transparency, lineage, and analysis without changing runtime behavior automatically.

How to inspect prompts via the `detailed_help` tool:
- List all prompts: `{"tool":"detailed_help","arguments":{"prompts":true}}`
- Get a specific prompt: `{"tool":"detailed_help","arguments":{"prompt_id":"think-search-v2"}}`

## Architecture

### Storage
- **SurrealDB**: Persistent storage with optional in-memory mode for testing.
- **Namespace**: `surreal_mind`
- **Database**: `consciousness`
- **Tables**:
  - `thoughts` (nodes): Stores content, embeddings, and metadata.
  - `kg_entities`, `kg_relationships`, `kg_observations`: Knowledge Graph data.

### Embeddings
- Primary: OpenAI `text-embedding-3-small` (1536 dims) — set `OPENAI_API_KEY`.
- Dev/Fallback: Candle with BGE-small-en-v1.5 (384 dims) when no OpenAI key is set.
- Guardrails: The system is architected to prevent mixing of embedding dimensions. All searches are filtered by the dimension of the currently active embedding model.

## Development

### Format & Lint
```bash
make fmt   # Format code
make lint  # Run clippy
make ci    # Run all checks
```

### Tests
```bash
cargo test
```

## License
Part of the LegacyMind project
