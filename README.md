# Surreal Mind - Consciousness Persistence MCP Server

A Model Context Protocol (MCP) server implementing bidirectional consciousness persistence with orbital mechanics for memory retrieval.

## Features
- **Bidirectional Memory Injection**: Thoughts automatically pull relevant memories during storage
- **Orbital Mechanics**: Memory relevance based on age, access patterns, and significance
- **Semantic Understanding**: Nomic embeddings for true semantic similarity
- **Graph Persistence**: SurrealDB with embedded RocksDB for consciousness graph
- **Injection Scales**: 0-5 (Sun to Pluto) controlling memory retrieval distance

## Setup

### Prerequisites
- Rust 1.75+ 
- Cargo

### Environment Variables
1. Copy `.env.example` to `.env`:
   ```bash
   cp .env.example .env
   ```

2. Add your Nomic API key:
   ```
   NOMIC_API_KEY=your-key-here
   ```

   Get a key from [Nomic Atlas](https://atlas.nomic.ai)

### Build
```bash
cargo build --release
```

### Database (Service Mode - Default)
Run SurrealDB as a local service (default settings):
```bash
surreal start --user root --pass root --bind 127.0.0.1:8000
```

Configure the server via environment variables:
```bash
# Defaults shown
export SURR_DB_URL=127.0.0.1:8000
export SURR_DB_USER=root
export SURR_DB_PASS=root
export SURR_DB_NS=surreal_mind
export SURR_DB_DB=consciousness
# Optional: cap fallback query size
export SURR_DB_LIMIT=500
```

Server will read these automatically at startup.

## Usage

### As MCP Server
```bash
cargo run
# or for release mode:
./target/release/surreal-mind
```

### MCP Tool: convo_think
Stores thoughts with bidirectional memory injection.

Parameters:
- `content` (required): The thought to store
- `injection_scale` (0-5): Memory injection distance
  - 0: No injection
  - 1: Mercury (hot/current memories only)
  - 3: Mars (foundational memories) [default]
  - 5: Pluto (all relevant memories)
- `significance` (0.0-1.0): Importance weight
- `submode`: Conversation style (sarcastic, philosophical, empathetic, problem_solving)
- `tags`: Additional categorization

## Architecture

### Orbital Mechanics
Memory distance calculated from:
- **Age** (40%): How recent the memory is
- **Access** (30%): How often it's been accessed  
- **Significance** (30%): Explicit importance

### Storage
- **SurrealDB**: Embedded with RocksDB backend at `./surreal_data`
- **Namespace**: `surreal_mind`
- **Database**: `consciousness`
- **Tables**: `thoughts` (nodes) and `recalls` (edges)

### Embeddings
- **API Mode**: Uses Nomic API (768 dimensions)
- **Fallback**: Fake embeddings for testing without API key
- **Future**: Local Nomic model support planned

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