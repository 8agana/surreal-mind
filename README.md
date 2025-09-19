# Surreal Mind - Consciousness Persistence MCP Server

A Model Context Protocol (MCP) server implementing bidirectional consciousness persistence with orbital mechanics for memory retrieval and cognitive scaffolding.

## Features
- **Unified Thinking Tools**: `legacymind_think` and `photography_think` with automatic mode routing
- **Session Continuity**: `session_id` and `previous_thought_id` chaining
- **Hypothesis Verification**: Evidence-based validation against the Knowledge Graph
- **Memory Injection**: KG-only retrieval with injection scales 0–3 (0=none; 1=5, 2=10, 3=20)
- **Semantic Embeddings**: OpenAI text-embedding-3-small (1536 dims)
- **SurrealDB Persistence**: Consciousness graph storage (SurrealDB over WebSocket)
- **Orbital Mechanics**: Memory relevance based on age, access frequency, and significance

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

## Quick Start

### 1. Launch SurrealDB
```bash
# Quick in-memory start (data lost on restart)
surreal start --user root --pass root --bind 127.0.0.1:8000 memory
```

### 2. Set Environment Variables
```bash
export OPENAI_API_KEY=sk-your-openai-key-here
# Optional: For reconnection on network issues
export SURR_DB_RECONNECT=1
# Optional: Strict embedding validation
export SURR_EMBED_STRICT=true
```

### 3. Start the MCP Server
```bash
./target/release/surreal-mind
```

### 4. Test with MCP Client
The server is now ready to handle MCP requests. Test with your preferred MCP client or use the provided test scripts:

```bash
# Run comprehensive tests
cargo test --test tool_schemas

# Or use the MCP comprehensive script
./tests/test_mcp_comprehensive.sh
```

### Troubleshooting
- **Protoc errors**: Install with `brew install protobuf` (already handled in CI)
- **Dimension mismatches**: Run `./target/release/reembed` to fix
- **Connection issues**: Check `SURR_DB_URL` points to running SurrealDB instance
- **Logs**: Set `RUST_LOG=surreal_mind=debug` for detailed output

## Recent Updates

### 2025-09-07 - Major Refactor: Unified Thinking Tools
- **Phase A/B Complete**: Consolidated 5 think tools into 2 domain-focused tools
  - `legacymind_think`: Development/technical thinking with mode routing
  - `photography_think`: Photography-specific thinking (auto-connects to photography namespace)
- **Mode Routing**: Automatic selection based on trigger phrases or heuristics
  - Trigger phrases: "debug time", "building time", "plan time", "i'm stuck", "question time"
  - Heuristic keywords: error→debug, implement→build, design→plan
- **Session Continuity**: Chain thoughts via `session_id` and `previous_thought_id`
- **Phase C Implemented**: Hypothesis verification with evidence-based validation
  - Deterministic rule-based classification
  - Confidence scoring: supporting/(supporting+contradicting)
  - Configurable thresholds and evidence limits

### 2025-09-06 - Production Ready
- Unified search tools: `legacymind_search` and `photography_search` replace legacy tools
- inner_voice: Synthesis with auto-extraction to KG candidates
- Frameworks: Local deterministic enhancements (600ms timeout)
- HTTP robustness: Resilient parsing, descriptive User-Agent strings

### Env quick‑ref (new)

- Frameworks (think_convo):
  - `SURR_THINK_ENHANCE=1` (default ON for think_convo)
  - `SURR_THINK_ENHANCE_TIMEOUT_MS=600`
  - `SURR_THINK_STRICT_JSON=1`
  - `SURR_THINK_TAG_WHITELIST=plan,debug,dx,photography,idea`
  - `SURR_THINK_LEXICON_DECIDE`, `SURR_THINK_LEXICON_VENT`, `SURR_THINK_LEXICON_CAUSAL`, `SURR_THINK_LEXICON_POS`, `SURR_THINK_LEXICON_NEG`
  - `SURR_THINK_DEEP_LOG=1` (gates 200‑char debug preview)
- Database: `SURR_DB_RECONNECT=1` (enables auto-reconnection with backoff), `SURR_DB_URL=wss://example.com:8000` (WebSocket endpoint)
- Embeddings: `SURR_EMBED_STRICT=true` (strict dimension/provider validation), `SURR_EMBED_RPS=1.0` (rate limit for API calls)
- inner_voice: `SURR_IV_AUTO_EXTRACT_KG=1` (default ON)
- UA traceability (optional): `SURR_COMMIT_HASH=<shortsha>`

## Production Deployment
- Defaults in this repo are for local development (127.0.0.1, http/ws without TLS). Do not use these defaults over a network.
- Use secure transports in production:
  - WebSocket (DB): set SURR_DB_URL to a wss:// endpoint, e.g., `export SURR_DB_URL=wss://db.example.com:8000`
  - HTTP SQL (fallback): the server derives an HTTP base from SURR_DB_URL for the REST /sql endpoint. Ensure it is https:// when used remotely.
- Credentials: SURR_DB_USER and SURR_DB_PASS are sent via HTTP Basic Auth for the REST SQL fallback. Always use TLS (https/wss) to protect credentials in transit.
- Recommended environment hardening:
  - Restrict exposure of the SurrealDB service to trusted networks only.
- Logging: Consider setting `MCP_NO_LOG=true` in environments where stderr must remain JSON-only for MCP clients. Use `RUST_LOG=surreal_mind=info,rmcp=info` or quieter. When `MCP_NO_LOG=1`, logging is disabled in stdio MCP mode.

Configure the server via environment variables:
```bash
# Database (WebSocket)
export SURR_DB_URL=ws://127.0.0.1:8000
export SURR_DB_USER=root
export SURR_DB_PASS=root
export SURR_DB_NS=surreal_mind
export SURR_DB_DB=consciousness

# Embeddings
export OPENAI_API_KEY={{YOUR_OPENAI_API_KEY}}        # or set SURR_EMBED_PROVIDER=candle for local dev
export SURR_EMBED_PROVIDER=openai                    # openai | candle
export SURR_EMBED_MODEL=text-embedding-3-small       # primary
# SURR_EMBED_DIM is inferred; avoid overriding unless you know what you're doing

# Retrieval (KG-only injection)
export SURR_KG_CANDIDATES=500
export SURR_INJECT_T1=0.6
export SURR_INJECT_T2=0.4
export SURR_INJECT_T3=0.25
export SURR_INJECT_FLOOR=0.15

# Runtime/logging
export RUST_LOG=surreal_mind=info,rmcp=info
# Set MCP_NO_LOG=1 to silence logs in stdio MCP mode
export SURR_TOOL_TIMEOUT_MS=15000
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
export SURR_SKIP_DIM_CHECK=false    # If true, bypass startup embedding dimension hygiene check

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

### Example: Memory Injection Scales
Memory retrieval adjusts based on injection scale (1-3):
- **Scale 1**: 5 entities, 0.6 proximity threshold
- **Scale 2**: 10 entities, 0.4 proximity threshold
- **Scale 3**: 20 entities, 0.25 proximity threshold

Server reads thresholds from env: `SURR_INJECT_T1/T2/T3` and `SURR_INJECT_FLOOR`.

## Usage

### As MCP Server
```bash
cargo run
# or for release mode:
./target/release/surreal-mind
```

### MCP Tool: legacymind_think
Unified development thinking with automatic mode routing, session continuity, and optional hypothesis verification. KG-only memory injection.

Parameters:
- `content` (required): Thought content
- `hint`: `"debug" | "build" | "plan" | "stuck" | "question"` — optional routing nudge
- `injection_scale`: 0–3; 0=none, 1=5, 2=10, 3=20
- `significance`: 0.0–1.0
- `tags`: string[]
- `session_id`, `previous_thought_id`, `chain_id`, `branch_from`, `revises_thought`: continuity fields
- `hypothesis`: string (optional)
- `needs_verification`: boolean (default false)
- `verify_top_k`: integer (env default `SURR_VERIFY_TOPK`)
- `min_similarity`: 0.0–1.0 (env default `SURR_VERIFY_MIN_SIM`)
- `evidence_limit`: integer (env default `SURR_VERIFY_EVIDENCE_LIMIT`)
- `verbose_analysis`: boolean (default true)

Example:
```json
{
  "tool": "legacymind_think",
  "arguments": {
    "content": "Planning the re-embed SOP for mismatched dims",
    "hint": "plan",
    "injection_scale": 2,
    "hypothesis": "We should filter KG by embedding_dim before cosine to avoid mismatches",
    "needs_verification": true
  }
}
```

Response includes:
- `thought_id`
- `mode_selected` and `reason`
- `memories_injected`
- `verification`: `confidence`, `supporting`, `contradicting` (when `needs_verification=true`)

Legacy aliases:
- `think_convo`, `think_plan`, `think_debug`, `think_build`, `think_stuck` route to `legacymind_think`. Prefer `legacymind_think` in new clients.

### Legacy alias: think_plan
Routes to `legacymind_think` (plan mode). Prefer `legacymind_think`.



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


### MCP Tool: detailed_help
Returns deterministic, example-rich documentation for tools and parameters.

Parameters:
- `tool`: "think_convo" | "think_plan" | "memories_create" | "legacymind_search" | "photography_search" (optional; overview when omitted)
- `format`: "full" (default) | "compact"

Examples:
```json
{"tool":"detailed_help","arguments":{"tool":"think_plan","format":"full"}}
{"tool":"detailed_help","arguments":{"format":"compact"}}
```

### MCP Tool: legacymind_search
Unified search over the Knowledge Graph; optionally include thoughts.

Parameters:
- `query`: object with text or filters (e.g., { "text": "..." })
- `include_thoughts`: boolean (default false)
- `target`: "entity" | "relationship" | "observation" | "mixed" (default "mixed")
- `sim_thresh`: 0.0–1.0 (optional)
- `top_k_memories`: 1–50 (default 10)
- `top_k_thoughts`: 1–50 (default 5)
- `thoughts_content`: string (optional free-text for thought search)

Example:
```json
{
  "tool": "legacymind_search",
  "arguments": {
    "query": { "text": "debug parser issue" },
    "include_thoughts": true,
    "top_k_memories": 10,
    "top_k_thoughts": 5,
    "sim_thresh": 0.5
  }
}
```

## Available Tools and Binaries

This project includes:

### Main MCP Server Binary
- `cargo run` or `./target/release/surreal-mind`: Starts the MCP server with stdio transport
- **Unified Thinking Tools**: `legacymind_think` (with automatic mode routing), `photography_think`
- **Legacy Tool Aliases** (forward to `legacymind_think`): `think_convo`, `think_plan`, `think_debug`, `think_build`, `think_stuck`
- **Memory & Knowledge Tools**: `memories_create`, `memories_moderate`, `inner_voice`, `legacymind_search`, `photography_search`, `photography_memories`, `photography_voice`, `photography_moderate`
- **Maintenance Tools**: `maintenance_ops`, `detailed_help`

### Inner Voice Tool
The `inner_voice` tool provides RAG-based synthesis and optional KG extraction.
- **Provider Chain**: Now Grok-primary with existing local fallback. CLI removed for simplicity; legacy envs warn and default to Grok (if key present) or local fallback. Update configs if needed.
- **Local Fallback Response**: "Based on what I could find: [summary of top snippets]"

### Photography Voice Tool
The `photography_voice` tool provides RAG-based synthesis for photography memories/thoughts in an isolated namespace, with optional KG extraction.
- **Provider Chain**: Same as inner_voice (Grok-primary with local fallback)
- **Namespace Isolation**: Operates on photography database (`ns=photography`, `db=work`)
- **Local Fallback Response**: "Based on what I could find: [summary of photography snippets]"

### Photography Moderate Tool
The `photography_moderate` tool reviews and decides on photography knowledge-graph candidates (entities/relationships) in the isolated photography namespace.
- **Actions**: Accept, reject, or get candidates
- **Namespace Isolation**: Targets photography KG candidates
- **Auto-extraction**: Uses Grok for KG candidate extraction if enabled.

### Additional Binaries (src/bin/)
- `cargo run --bin reembed`: Re-embed thoughts to the active provider/model/dim
- `cargo run --bin reembed_kg`: Re-embed KG entities/observations
- `cargo run --bin fix_dimensions`: Correct thoughts with wrong embedding dimensions
- `cargo run --bin db_check`: DB connectivity test
- `cargo run --bin check_db_contents`: Inspect DB contents
- `cargo run --bin simple_db_test`: Basic DB ops smoke test

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

When `SURR_SUBMODE_RETRIEVAL=true` (internal flag), retrieval weights may be adjusted.

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
- `submode`: Internal field, not exposed via API
- `framework_enhanced`: Boolean indicating framework processing
- `framework_analysis`: JSON object with insights, questions, next steps

**Recalls table:**
- `submode_match`: Internal field for retrieval tuning
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
Run the full test suite (52 tests total, includes unit, integration, KG, schema validation):
```bash
cargo test
```

Run specific test suites:
```bash
# Tool schemas and parameters
cargo test --test tool_schemas

# Integration tests
cargo test --test inner_voice_retrieve

# MCP comprehensive tests
./tests/test_mcp_comprehensive.sh
```

## License
Part of the LegacyMind project
