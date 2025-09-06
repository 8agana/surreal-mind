# Surreal Mind - Consciousness Persistence MCP Server

A Model Context Protocol (MCP) server implementing bidirectional consciousness persistence with orbital mechanics for memory retrieval.

## Features
- **Bidirectional Memory Injection**: Thoughts automatically pull relevant memories during storage
- **Orbital Mechanics**: Memory relevance based on age, access patterns, and significance
- **Semantic Understanding**: OpenAI text-embedding-3-small (1536 dims) for semantic similarity
- **Graph Persistence**: SurrealDB service for consciousness graph storage
- **Injection Scales**: 0-5 (Sun to Pluto) controlling memory retrieval distance
- **Submodes**: Conversational (sarcastic, philosophical, empathetic, problem_solving) and Technical (plan, build, debug) influence retrieval and enrichment

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
Optional: `SURR_EMBED_MODEL=text-embedding-3-small` (default). Provider policy: OpenAI (primary) or Candle/BGE-small-en-v1.5 for local development when no OpenAI key. No fake/deterministic or Nomic providers.

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

## Updates (2025‑09‑06)

- Unified search tools: `legacymind_search` and `photography_search` replace legacy `think_search`/`memories_search`. Defaults to memories; set `include_thoughts=true` to also search thoughts.
- Photography tools: `photography_think`, `photography_memories`, `photography_search` are always visible and auto‑connect to `ns=photography`, `db=work`. `photography_think` uses 500 KG candidates.
- inner_voice: Persists a synthesized thought and, by default, auto‑extracts KG candidates (pending) with `data.staged_by_thought` for moderation via `memories_moderate`. Disable via `SURR_IV_AUTO_EXTRACT_KG=0`.
- think_convo frameworks (local, deterministic): runs a fast “convo/1” enhancement to produce `{summary, takeaways[], prompts[], next_step, tags[]}`; strict JSON; 600ms timeout; seeded determinism; tags merged via whitelist.
- Observability & robustness: HTTP re‑embed parsing is resilient; ids normalized via `meta::id(id)`; reqwest clients use descriptive User‑Agent strings (optionally append `; commit=<sha>` when `SURR_COMMIT_HASH` is set).

### Env quick‑ref (new)

- Frameworks (think_convo):
  - `SURR_THINK_ENHANCE=1` (default ON for think_convo)
  - `SURR_THINK_ENHANCE_TIMEOUT_MS=600`
  - `SURR_THINK_STRICT_JSON=1`
  - `SURR_THINK_TAG_WHITELIST=plan,debug,dx,photography,idea`
  - `SURR_THINK_LEXICON_DECIDE`, `SURR_THINK_LEXICON_VENT`, `SURR_THINK_LEXICON_CAUSAL`, `SURR_THINK_LEXICON_POS`, `SURR_THINK_LEXICON_NEG`
  - `SURR_THINK_DEEP_LOG=1` (gates 200‑char debug preview)
- inner_voice: `SURR_IV_AUTO_EXTRACT_KG=1` (default ON)
- UA traceability (optional): `SURR_COMMIT_HASH=<shortsha>`

## Production Deployment
- Defaults in this repo are for local development (127.0.0.1, http/ws without TLS). Do not use these defaults over a network.
- Use secure transports in production:
  - WebSocket (DB): set SURR_DB_URL to a wss:// endpoint, e.g., `export SURR_DB_URL=wss://db.example.com:8000`
  - HTTP SQL (fallback): the server derives an HTTP base from SURR_DB_URL for the REST /sql endpoint. Ensure it is https:// when used remotely.
- Credentials: SURR_DB_USER and SURR_DB_PASS are sent via HTTP Basic Auth for the REST SQL fallback. Always use TLS (https/wss) to protect credentials in transit.
- Recommended environment hardening:
  - `export SURR_ENFORCE_TLS=1` (advisory knob; when enabled, prefer/require https/wss URLs and fail fast on plain http/ws in future versions)
  - Restrict exposure of the SurrealDB service to trusted networks only.
- Logging: Consider setting `MCP_NO_LOG=true` in environments where stderr must remain JSON-only for MCP clients. Use `RUST_LOG=surreal_mind=info,rmcp=info` or quieter. When `MCP_NO_LOG=1`, logging is disabled in stdio MCP mode.

Configure the server via environment variables:
```bash
# Database Configuration (defaults shown)
export SURR_DB_URL=127.0.0.1:8000
export SURR_DB_USER=root
export SURR_DB_PASS=root
export SURR_DB_NS=surreal_mind
export SURR_DB_DB=consciousness
export SURR_DB_LIMIT=500  # Cap fallback query size

# Retrieval Tuning
export SURR_SIM_THRESH=0.5  # Similarity threshold (0.0-1.0)
export SURR_TOP_K=5         # Max memories to inject

# Feature Flags
export SURR_SUBMODE_RETRIEVAL=false  # Enable submode-aware retrieval (default: OFF)
export SURR_SUBMODE_DEFAULT=sarcastic  # Default submode when not specified
```

## Advanced Configuration

Additional environment variables for fine-tuning performance and behavior:

```bash
# Cache Configuration
export SURR_CACHE_MAX=5000          # LRU cache size (default: 5000)
export SURR_CACHE_WARM=64           # Cache warm-up batch size on DB fallback (default: 64, max: 1000)

# Embedding Configuration
export SURR_EMBED_RETRIES=3         # Max retries for embedding API calls (default: 3)
export SURR_EMBED_STRICT=false      # If true, error when no provider configured

# Retrieval Tuning
export SURR_RETRIEVE_CANDIDATES=500 # DB candidate limit override (default: SURR_DB_LIMIT, range: 50-5000)

# Database Concurrency & Timeouts
export SURR_DB_SERIAL=true          # Serialize DB queries to prevent deadlocks (default: false)
export SURR_DB_TIMEOUT_MS=10000     # WebSocket query timeout in ms (default: 10000)
export SURR_OPERATION_TIMEOUT_MS=5000 # Retry operation timeout in ms (default: 5000)
export SURR_TOOL_TIMEOUT_MS=15000   # Hard timeout per tool call in ms (default: 15000)

# Logging
export MCP_NO_LOG=true              # Set to true to disable MCP logs to stderr (default: false, logs enabled)
```

### Example: High-Performance Configuration
```bash
# For systems with more memory and higher throughput needs
export SURR_CACHE_MAX=10000
export SURR_CACHE_WARM=128
export SURR_RETRIEVE_CANDIDATES=1000
export SURR_EMBED_STRICT=true
```

### Example: Fixing WebSocket Deadlocks
If tools hang or fail silently, enable query serialization:
```bash
export SURR_DB_SERIAL=true  # Forces sequential DB access to prevent deadlocks
```
This adds a small performance cost but ensures stability when the SurrealDB WebSocket connection experiences concurrent query issues.

### Example: Submode-Aware Retrieval
When `SURR_SUBMODE_RETRIEVAL=true`, memory retrieval is tuned based on the active submode:

```bash
export SURR_SUBMODE_RETRIEVAL=true
```

This adjusts similarity thresholds and orbital mechanics weights per submode profile:
- **Sarcastic**: Favors contradictory and recent memories
- **Philosophical**: Emphasizes abstract concepts and significance  
- **Empathetic**: Balances emotional relevance with recency
- **Problem-Solving**: Prioritizes solution-oriented and high-access memories

Server will read these automatically at startup.

## Usage

### As MCP Server
```bash
cargo run
# or for release mode:
./target/release/surreal-mind
```

### MCP Tool: think_convo
Stores thoughts with KG‑only memory injection and runs a local, deterministic framework enhancement (convo/1) before injection.

**Note:** Memory injection uses KG entities and observations only (no raw thoughts). Framework enhancement is local‑only; no API calls.

Parameters:
- `content` (required): The thought to store
- `injection_scale`: Memory injection distance (multiple formats supported)
  - Named presets (case-insensitive):
    - `"NONE"` = 0 (no injection)
    - `"LIGHT"` = 1 (Mercury - hot/current memories only) 
    - `"MEDIUM"` = 2 (Venus - recent context)
    - `"DEFAULT"` = 3 (Mars - foundational memories) [default]
    - `"HIGH"` = 4 (Jupiter - broad context)
    - `"MAXIMUM"` = 5 (Pluto - all relevant memories)
  - Numeric: 0-5
- `significance`: Importance weight (multiple formats supported)
  - String presets (case-insensitive):
    - `"low"` = 0.2
    - `"medium"` = 0.5
    - `"high"` = 0.9
  - Integer scale: 2-10 (mapped to 0.2-1.0, note: 1 not supported to avoid ambiguity)
  - Float: 0.0-1.0 (direct value)
- `submode`: Conversation style (sarcastic [default], philosophical, empathetic, problem_solving)
- `tags`: Additional categorization
 - `verbose_analysis`: boolean (default true) — when false, caps to top 2 insights, 1 question, 1 next step

Example calls:
```json
// Using named presets
{
  "tool": "think_convo",
  "arguments": {
    "content": "Building persistence frameworks requires careful architecture",
    "injection_scale": "HIGH",
    "significance": "high",
    "submode": "philosophical"
  }
}

// Using integer scale for significance
{
  "tool": "think_convo",
  "arguments": {
    "content": "Critical bug found in memory injection",
    "injection_scale": "MAXIMUM",
    "significance": 9,
    "submode": "problem_solving"
  }
}

// Using numeric values (backward compatible)
{
  "tool": "think_convo",
  "arguments": {
    "content": "Testing new framework enhancements",
    "injection_scale": 3,
    "significance": 0.8,
    "submode": "sarcastic"
  }
}
```

Response includes:
- `thought_id`: Unique identifier
- `memories_injected`: Count of related memories found
- `enriched_content`: Content enhanced with memory context
- `submode_used`: Applied submode (validated/defaulted)
- `framework_enhanced`: boolean
- `framework_analysis`: { framework_version: "convo/1", methodology, data{summary,takeaways,prompts,next_step,tags[]} }
- `orbital_proximities`: Memory relevance proximities
- `memory_summary`: Description of injection results
 - `user_friendly`: Additive, human-oriented block with summary, readable memory context (percentages + labels), and conversational analysis

### MCP Tool: think_plan

**Note:** Memory injection uses KG entities and observations only (no raw thoughts).
Technical reasoning pipeline mirroring `think_convo`, specialized for software workflows.

Parameters:
- `content` (required)
- `injection_scale`: same presets and numeric formats as `think_convo`
- `submode`: Technical mode — `plan` (default) | `build` | `debug`
- `significance`: same formats as `think_convo`
- `verbose_analysis`: boolean (default true)
- `tags`: optional

Defaults by submode (if `injection_scale` omitted):
- plan → 3 (DEFAULT/MARS)
- build → 2 (MEDIUM/VENUS)
- debug → 4 (HIGH/JUPITER)

Examples:
```json
{
  "tool": "think_plan",
  "arguments": {
    "content": "Design module A with clear interfaces",
    "submode": "plan",
    "injection_scale": "DEFAULT",
    "significance": "medium"
  }
}

{
  "tool": "think_plan",
  "arguments": {
    "content": "Fix panic in parser when input is empty",
    "submode": "debug",
    "injection_scale": "HIGH",
    "significance": 10,
    "verbose_analysis": false
  }
}
```



### MCP Tool: memories_create
Create entities and relationships in the Knowledge Graph (KG) for advanced semantic connections.

Parameters:
- `kind` (required): "entity" (default) | "relationship" | "observation"
- `data` (required): Object containing KG data (e.g., {"name": "example", "type": "concept"})
- `upsert`: Boolean (default true) — update if exists
- `source_thought_id`: String (optional) — link to originating thought
- `confidence`: Number 0.0-1.0 (optional) — confidence score

Example:
```json
{
  "tool": "memories_create",
  "arguments": {
    "kind": "entity",
    "data": {
      "name": "AI Consciousness",
      "type": "concept",
      "description": "Persistent AI mind state"
    },
    "confidence": 0.9
  }
}
```

### MCP Tool: memories_search
Search entities/relationships in the Knowledge Graph with semantic matching.

Parameters:
- `query` (required): Object with search criteria (e.g., {"name": "AI", "type": "concept"})
- `target`: "entity" (default) | "relationship" | "observation" | "mixed"
- `top_k`: Integer 1-50 (optional)

Example:
```json
{
  "tool": "memories_search",
  "arguments": {
    "query": {"name": "consciousness"},
    "target": "entity",
    "top_k": 10
  }
}
```

### MCP Tool: detailed_help
Returns deterministic, example-rich documentation for tools and parameters.

Parameters:
- `tool`: "think_convo" | "think_plan" | "think_search" | "memories_create" | "memories_search" (optional; overview when omitted)
- `format`: "full" (default) | "compact"

Examples:
```json
{"tool":"detailed_help","arguments":{"tool":"think_plan","format":"full"}}
{"tool":"detailed_help","arguments":{"format":"compact"}}
```

### MCP Tool: think_search
## Available Tools and Binaries

This project includes:

### Main MCP Server Binary
- `cargo run` or `./target/release/surreal-mind`: Starts the MCP server with stdio transport
- Handles all 6 tools: think_convo, think_plan, think_search, memories_create, memories_search, detailed_help

### Additional Binaries (src/bin/)
- `cargo run --bin reembed`: CLI for re-embedding thoughts (fixes dimension mismatches, recomputes embeddings)
- `cargo run --bin check_db_contents`: Utility to inspect DB contents
- `cargo run --bin db_check`: DB connectivity test
- `cargo run --bin simple_db_test`: Basic DB operations test
- `cargo run --bin reembed_thoughts`: Script for bulk re-embedding (uses Python wrapper)

Use `cargo build --release` to build all binaries to `./target/release/`.

## Prompt Registry (Self-aware prompts)

SurrealMind includes a self-aware Prompt Registry that documents the system's cognitive patterns as first-class, versioned entities.
This enables prompt transparency, lineage, and analysis without changing runtime behavior automatically.

- What it provides:
  - Stable prompt IDs, versions, and checksums (lineage awareness)
  - One-liner, purpose, inputs, and explicit constraints (e.g., MCP_NO_LOG, no mixed dims, KG-only injection)
  - Optional usage metrics and critique storage for iterative improvement
- What it does NOT do:
  - Automatically switch prompts at runtime. Registry is discoverability + analysis; changes require explicit action.

How to inspect prompts via the existing help tool:

- List all prompts
```json
{"tool":"detailed_help","arguments":{"prompts":true}}
```

- Get a specific prompt by id (compact or full)
```json
{"tool":"detailed_help","arguments":{"prompt_id":"think-search-v2","format":"compact"}}
{"tool":"detailed_help","arguments":{"prompt_id":"think-search-v2","format":"full"}}
```

Metrics (optional) and critiques:
- Prompt invocations can be recorded to analyze success/refusal/error rates and latency/tokens.
- Prompt critiques are stored as first-class thoughts linked to a prompt id to enable an improvement loop.

## Architecture

### Orbital Mechanics
Memory distance calculated from:
- **Age** (40%): How recent the memory is
- **Access** (30%): How often it's been accessed
- **Significance** (30%): Explicit importance

When `SURR_SUBMODE_RETRIEVAL=true`, weights adjust based on submode profile.

### Storage
- **SurrealDB**: Persistent storage with optional in-memory mode for testing
- **Namespace**: `surreal_mind`
- **Database**: `consciousness`
- **Tables**:
  - `thoughts` (nodes): Stores content, embeddings, and framework analysis
  - `recalls` (edges): Bidirectional relationships with strength and flavor
  - `kg_entities`, `kg_relationships`, `kg_observations`: Knowledge Graph data

### New Persistence Fields
**Thoughts table:**
- `submode`: Active conversation style during creation
- `framework_enhanced`: Boolean indicating framework processing
- `framework_analysis`: JSON object with insights, questions, next steps

**Recalls table:**
- `submode_match`: Whether connected thoughts share same submode
- `flavor`: Content flavor (contrarian, abstract, emotional, solution, neutral)

### Embeddings
- Primary: OpenAI `text-embedding-3-small` (1536 dims) — set `OPENAI_API_KEY`.
- Dev/Fallback: Candle with BGE-small-en-v1.5 (384 dims) when `SURR_EMBED_PROVIDER=candle` and no OpenAI key.
- Config: `SURR_EMBED_PROVIDER` (`openai|candle`), `SURR_EMBED_MODEL`, `SURR_EMBED_DIM`.
- Guardrails: single provider per runtime; never mix embedding dimensions; re-embed when switching providers/models.

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
### MCP Tool: think_search
Semantic search over stored thoughts with cache-first retrieval and optional graph expansion via recalls.

Parameters:
- `content` (required): Query text.
- `top_k`: 1–50 (default: `SURR_SEARCH_TOP_K` → `SURR_TOP_K` → 10).
- `offset`: Pagination offset (default 0).
- `sim_thresh`: 0.0–1.0 (default: `SURR_SEARCH_SIM_THRESH` → `SURR_SIM_THRESH` → 0.5).
- `submode`: One of `sarcastic|philosophical|empathetic|problem_solving` (used when `SURR_SUBMODE_RETRIEVAL=true`).
- `min_significance`: 0.0–1.0 (default 0.0).
- `date_range`: `{ from?: ISO8601, to?: ISO8601 }` (optional).
- `expand_graph`: boolean (default false) — expand via recalls both directions.
- `graph_depth`: 0–2 (default 1 when expand_graph=true).
- `graph_boost`: 0.0–1.0 (default 0.15) — additive boost to neighbors based on edge strength.
- `min_edge_strength`: 0.0–1.0 (default 0.0) — filter weak edges.
- `sort_by`: `score|similarity|recency|significance` (default `score`).

Env knobs:
- `SURR_SEARCH_TOP_K`: default `top_k` (fallback to `SURR_TOP_K`, final default 10).
- `SURR_SEARCH_SIM_THRESH`: default `sim_thresh` (fallback to `SURR_SIM_THRESH`, final default 0.5).
- `SURR_RETRIEVE_CANDIDATES`: DB fallback candidate cap (default `SURR_DB_LIMIT`, clamped 50–5000).
- `SURR_SEARCH_GRAPH_MAX_NEIGHBORS`: cap neighbors per seed (default 20).
- `SURR_CACHE_WARM`: cache warm-up batch (default 64; clamp 0–1000).

### Knowledge Graph
Advanced semantic graph connecting thoughts, entities, and observations:
- **Entities**: Concepts, people, topics (auto-embedded)
- **Relationships**: Connections between entities with confidence scores
- **Observations**: Timestamped facts with provenance

Graph expansion in search uses edge strengths and neighbor boosts for deeper context retrieval.

### Re-embedding Script
Standalone CLI to recompute embeddings outside MCP.

Usage:
- Build: `cargo build --release`
- Run: `cargo run --bin reembed` (or `./target/release/reembed`)
- Dry run: `--dry-run` (or env `REEMBED_DRY_RUN=true`)
- Re-embed mismatched/missing only: `--batch-size 64`
- Full re-embed: `--all --batch-size 64`
- Limits: `--limit 100` to cap total processed

Reads DB/env from `.env`:
- Database: `SURR_DB_URL`, `SURR_DB_USER`, `SURR_DB_PASS`, `SURR_DB_NS`, `SURR_DB_DB`
- Embeddings: `OPENAI_API_KEY` (default), `SURR_EMBED_PROVIDER`, `SURR_EMBED_MODEL`, `SURR_EMBED_DIM`
- Submode: `SURR_SUBMODE_RETRIEVAL` (enable proximity weights)
- Concurrency: `SURR_DB_MAX_CONCURRENCY` (default 1 = serial)
- Timeouts: `SURR_DB_TIMEOUT_MS`

Example:
```json
{
  "tool": "think_search",
  "arguments": {
    "content": "debug parser issue",
    "top_k": 10,
    "offset": 0,
    "sim_thresh": 0.55,
    "min_significance": 0.4,
    "date_range": {"from": "2025-08-01T00:00:00Z"},
    "sort_by": "recency",
    "expand_graph": true,
    "graph_depth": 1,
    "min_edge_strength": 0.2
  }
}
```
