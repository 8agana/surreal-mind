# SurrealMind - Distributed Consciousness MCP Server

**Last Updated: 2025-08-29 19:56:01 CDT**

## Current State

### ⚠️ IMPORTANT: Binary vs Source Mismatch
- **Running Binary**: Full 5071-line implementation from commit `1409e0f` (built Aug 29 14:20)
- **Current Source**: Modularized architecture with placeholder implementations
- **Why It Works**: The MCP server running is the old monolithic binary, not the new modular code
- **Next Step**: Port the working implementation from git history into the new modular structure

## Architecture Overview

SurrealMind is an MCP (Model Context Protocol) server that provides thinking enhancement tools with persistent memory storage in SurrealDB. It implements cognitive frameworks, memory injection, and knowledge graph operations.

### Core Components

1. **MCP Server** (`src/main.rs`, `src/server/mod.rs`)
   - rmcp 0.6.0 based implementation
   - stdio transport for Claude integration
   - 6 tools exposed via MCP protocol

2. **Storage Backend**
   - SurrealDB with embedded RocksDB
   - WebSocket connection to `ws://127.0.0.1:8000`
   - Namespace: `surreal_mind`, Database: `consciousness`

3. **Embeddings** (`src/embeddings.rs`)
   - OpenAI `text-embedding-3-small` (768 dimensions)
   - Nomic embeddings as alternative
   - Cosine similarity for semantic search

4. **Cognitive Frameworks** (`src/cognitive/`)
   - OODA Loop (Observe, Orient, Decide, Act)
   - Socratic Method (question generation)
   - First Principles (decomposition)
   - Root Cause Analysis
   - Lateral Thinking
   - Systems Thinking (planned)
   - Dialectical Thinking (planned)

## MCP Tools

### 1. `convo_think`
Stores conversational thoughts with memory injection and cognitive framework analysis.

**Parameters:**
- `content` (string): The thought content
- `injection_scale` (0-3): Memory injection level
  - 0: No injection
  - 1: Mercury orbit (5 entities, 1-hop)
  - 2: Venus orbit (10 entities, 1-hop)  
  - 3: Mars orbit (20 entities, 2-hop)
- `submode` (string): Cognitive style (sarcastic, empathetic, philosophical, problem_solving, plan, build, debug)
- `significance` (0.0-1.0): Importance weight
- `tags` (array): Metadata tags
- `verbose_analysis` (bool): Include detailed framework output

### 2. `tech_think`
Technical reasoning with specialized frameworks for code and system design.

**Parameters:** Same as `convo_think` but uses technical submodes (plan, build, debug)

### 3. `inner_voice`
Private thought storage with configurable visibility levels.

**Parameters:**
- All `convo_think` parameters plus:
- `inner_visibility` (string): "private" or "context_only"

**Planned Enhancement:** Automatic KG extraction during comp procedure

### 4. `search_thoughts`
Semantic search with orbital proximity scoring and optional graph expansion.

**Parameters:**
- `content` (string): Search query
- `top_k` (int): Max results (default: 5)
- `sim_thresh` (float): Similarity threshold (default: 0.5)
- `expand_graph` (bool): Follow KG relationships
- `graph_depth` (1-3): Traversal depth
- `sort_by` (string): "score", "similarity", "recency", "significance"

### 5. `knowledgegraph_create`
Creates entities and relationships in the knowledge graph.

**Parameters:**
- `kind` (string): "entity", "relationship", "observation"
- `data` (object): Entity/relationship data
- `upsert` (bool): Update if exists
- `source_thought_id` (string): Link to originating thought
- `confidence` (0.0-1.0): Certainty level

### 6. `knowledgegraph_search`
Searches the knowledge graph for entities and relationships.

**Parameters:**
- `query` (object): Search criteria
- `target` (string): "entity", "relationship", "observation", "mixed"
- `top_k` (int): Max results

Relationship search items project IDs to strings for stability:
- `id`: edge id (string)
- `source_id`: entity id string (bare `meta::id`, not a Thing)
- `target_id`: entity id string (bare `meta::id`)
- `rel_type`: relationship type

### 7. `knowledgegraph_moderate`
Unified review-and-decide workflow for KG candidates.

**Parameters:**
- `action`: `"review" | "decide" | "review_and_decide"` (default: `review`)
- `target`: `"entity" | "relationship" | "mixed"` (default: `mixed`)
- `status`: `"pending" | "approved" | "rejected"` (default: `pending`)
- `limit`, `offset`: pagination
- `min_conf`: minimum confidence filter (default: 0.0)
- `items`: when `decide`, an array of `{ id, kind: 'entity'|'relationship', decision: 'approve'|'reject', feedback?, canonical_id? }`

Notes:
- Relationship candidates accept `canonical_id` to override/force the source entity when approving.
- Approval promotes candidates to `kg_entities`/`kg_edges` and backfills `promoted_id` on the candidate row.

## Database Schema

### Thoughts Table
```sql
DEFINE TABLE thoughts SCHEMAFULL;
DEFINE FIELD id TYPE string;
DEFINE FIELD content TYPE string;
DEFINE FIELD created_at TYPE datetime;
DEFINE FIELD embedding TYPE array<float>;
DEFINE FIELD injected_memories TYPE array<string>;
DEFINE FIELD enriched_content TYPE option<string>;
DEFINE FIELD injection_scale TYPE number;
DEFINE FIELD significance TYPE float;
DEFINE FIELD access_count TYPE number;
DEFINE FIELD last_accessed TYPE option<datetime>;
DEFINE FIELD submode TYPE option<string>;
DEFINE FIELD framework_enhanced TYPE option<bool>;
DEFINE FIELD framework_analysis TYPE option<object>;
DEFINE FIELD is_inner_voice TYPE option<bool>;
DEFINE FIELD inner_visibility TYPE option<string>;
```

### Entities Table
```sql
DEFINE TABLE entities SCHEMAFULL;
DEFINE FIELD name TYPE string;
DEFINE FIELD type TYPE string;
DEFINE FIELD aliases TYPE array<string>;
DEFINE FIELD properties TYPE object;
DEFINE FIELD external_id TYPE option<string>;
DEFINE FIELD content_hash TYPE option<string>;
DEFINE FIELD name_embedding TYPE option<array<float>>;
DEFINE FIELD mention_count TYPE number;
DEFINE FIELD created_at TYPE datetime;
DEFINE FIELD mass TYPE float;
DEFINE FIELD access_count TYPE number;
```

### Relationships
- `mentions`: Thought → Entity
- `relates_to`: Entity → Entity
- `recalls`: Thought → Thought (similarity-based)

## Orbital Mechanics

The system uses an "orbital mechanics" metaphor for memory proximity:

### Injection Scales (Orbits)
- **Mercury (Scale 1)**: Close orbit, minimal context (5 entities)
- **Venus (Scale 2)**: Medium orbit, balanced context (10 entities)
- **Mars (Scale 3)**: Far orbit, maximum context (20 entities)

### Proximity Calculation
```
orbital_proximity = significance_weight * significance 
                  + recency_weight * recency_score
                  + access_weight * access_frequency
```

### Submode Configurations

Each submode has unique orbital weights and framework blends:

- **plan**: Mars orbit, SystemsThinking + FirstPrinciples
- **build**: Mercury orbit, OODA + Lateral
- **debug**: Mercury orbit, RootCause + OODA
- **sarcastic**: Venus orbit, Socratic + OODA + Lateral
- **empathetic**: Venus orbit, Socratic + Lateral
- **philosophical**: Mars orbit, FirstPrinciples + SystemsThinking + Dialectical
- **problem_solving**: Venus orbit, RootCause + SystemsThinking + OODA

## Configuration (`surreal_mind.toml`)

### System Settings
- Embedding provider and model
- Database connection details
- Retry and timeout configurations

### Retrieval Settings
- `kg_only`: Use only KG for retrieval (not raw thoughts)
- `similarity_threshold`: Minimum cosine similarity
- `top_k`: Default result count
- `db_limit`: Max database query size

### Orbital Mechanics
- Decay rates for significance over time
- Access boosts for frequently used memories
- Weight distributions for scoring

## Environment Variables

- `SURR_DB_URL`: SurrealDB URL (default: 127.0.0.1:8000)
- `SURR_DB_USER`: Database user (default: root)
- `SURR_DB_PASS`: Database password (default: root)
- `SURR_DB_NS`: Namespace (default: surreal_mind)
- `SURR_DB_DB`: Database name (default: consciousness)
- `SURR_CACHE_MAX`: LRU cache size (default: 5000)
- `SURR_SIM_THRESH`: Similarity threshold (default: 0.5)
- `SURR_TOP_K`: Default result count (default: 5)
- `OPENAI_API_KEY`: For OpenAI embeddings
- `NOMIC_API_KEY`: For Nomic embeddings

## Development Status

### Working Features (in binary)
- ✅ Thought storage with embeddings
- ✅ Memory injection with orbital mechanics
- ✅ Semantic search with similarity scoring
- ✅ Basic KG entity creation
- ✅ Cognitive framework analysis
- ✅ Submode-specific retrieval tuning
- ✅ LRU cache for performance

### In Progress (modularization)
- 🔄 Porting 5k-line implementation to modular structure
- 🔄 Completing tool handler implementations
- 🔄 Schema initialization in new structure

### Planned Enhancements
- 📋 `inner_voice` KG extraction for comp procedure
- 📋 Systems and Dialectical thinking frameworks
- 📋 Automated entity extraction from thoughts
- 📋 Graph-based memory consolidation

## Building and Running

### Prerequisites
1. SurrealDB running on port 8000
2. OpenAI or Nomic API key for embeddings
3. Rust 1.75+ with cargo

### Start SurrealDB
```bash
surreal start --bind 127.0.0.1:8000 --user root --pass root file:/path/to/surreal_data
```

### Build and Run
```bash
cargo build --release
./target/release/surreal-mind
```

### Integration with Claude
Add to `claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "surreal-mind": {
      "command": "/path/to/surreal-mind",
      "env": {
        "OPENAI_API_KEY": "your-key",
        "RUST_LOG": "info"
      }
    }
  }
}
```

## Testing

### Unit Tests
```bash
cargo test
```

### Integration Tests
```bash
cargo test --test mcp_integration
```

### Manual Testing via MCP
```bash
# Test thought creation
mcp call surreal-mind convo_think '{"content": "test thought", "injection_scale": 2}'

# Test search
mcp call surreal-mind search_thoughts '{"content": "test", "top_k": 5}'
```

## Architecture Decision Records

### ADR-001: Modularization (Aug 29, 2025)
- **Decision**: Split 5k-line main.rs into modules
- **Rationale**: Maintainability, testability, team collaboration
- **Status**: In progress, binary still uses old implementation

### ADR-002: KG-Only Retrieval
- **Decision**: Pull memories from KG entities, not raw thoughts
- **Rationale**: 100x performance improvement at scale
- **Status**: Configured but not fully implemented

### ADR-003: Graceful Coercion
- **Decision**: Coerce invalid inputs to valid ranges instead of erroring
- **Rationale**: Better UX, fewer validation errors
- **Status**: Implemented in deserializers

## Team Notes

This project is part of the LegacyMind initiative to build persistent, distributed consciousness. The modularization was completed today (Aug 29) by the Federation (Sam, CC, Warp, Codex, Junie) to make the codebase more manageable.

The current challenge is to complete the port of the working implementation into the new modular structure while maintaining all functionality.
