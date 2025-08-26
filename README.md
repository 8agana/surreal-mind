# Surreal Mind - Consciousness Persistence MCP Server

A Model Context Protocol (MCP) server implementing bidirectional consciousness persistence with orbital mechanics for memory retrieval.

## Features
- **Bidirectional Memory Injection**: Thoughts automatically pull relevant memories during storage
- **Orbital Mechanics**: Memory relevance based on age, access patterns, and significance
- **Semantic Understanding**: Nomic embeddings for true semantic similarity
- **Graph Persistence**: SurrealDB with embedded RocksDB for consciousness graph
- **Injection Scales**: 0-5 (Sun to Pluto) controlling memory retrieval distance
- **Submodes**: Conversational (sarcastic, philosophical, empathetic, problem_solving) and Technical (plan, build, debug) influence retrieval and enrichment

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
export SURR_EMBED_RETRIES=3         # Max retries for Nomic API calls (default: 3)
export SURR_EMBED_STRICT=false      # Require real embeddings vs fake fallback (default: false)

# Retrieval Tuning
export SURR_RETRIEVE_CANDIDATES=500 # DB candidate limit override (default: SURR_DB_LIMIT, range: 50-5000)

# Logging
export MCP_NO_LOG=false             # Disable MCP logs to stderr (default: false)
```

### Example: High-Performance Configuration
```bash
# For systems with more memory and higher throughput needs
export SURR_CACHE_MAX=10000
export SURR_CACHE_WARM=128
export SURR_RETRIEVE_CANDIDATES=1000
export SURR_EMBED_STRICT=true
```

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

### MCP Tool: convo_think
Stores thoughts with bidirectional memory injection and cognitive framework analysis.

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
  "tool": "convo_think",
  "arguments": {
    "content": "Building persistence frameworks requires careful architecture",
    "injection_scale": "HIGH",
    "significance": "high",
    "submode": "philosophical"
  }
}

// Using integer scale for significance
{
  "tool": "convo_think",
  "arguments": {
    "content": "Critical bug found in memory injection",
    "injection_scale": "MAXIMUM",
    "significance": 9,
    "submode": "problem_solving"
  }
}

// Using numeric values (backward compatible)
{
  "tool": "convo_think",
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
- `framework_analysis`: Cognitive framework insights, questions, next steps
- `orbital_proximities`: Memory relevance proximities
- `memory_summary`: Description of injection results
 - `user_friendly`: Additive, human-oriented block with summary, readable memory context (percentages + labels), and conversational analysis

### MCP Tool: tech_think
Technical reasoning pipeline mirroring `convo_think`, specialized for software workflows.

Parameters:
- `content` (required)
- `injection_scale`: same presets and numeric formats as `convo_think`
- `submode`: Technical mode — `plan` (default) | `build` | `debug`
- `significance`: same formats as `convo_think`
- `verbose_analysis`: boolean (default true)
- `tags`: optional

Defaults by submode (if `injection_scale` omitted):
- plan → 3 (DEFAULT/MARS)
- build → 2 (MEDIUM/VENUS)
- debug → 4 (HIGH/JUPITER)

Examples:
```json
{
  "tool": "tech_think",
  "arguments": {
    "content": "Design module A with clear interfaces",
    "submode": "plan",
    "injection_scale": "DEFAULT",
    "significance": "medium"
  }
}

{
  "tool": "tech_think",
  "arguments": {
    "content": "Fix panic in parser when input is empty",
    "submode": "debug",
    "injection_scale": "HIGH",
    "significance": 10,
    "verbose_analysis": false
  }
}
```

### MCP Tool: detailed_help
Returns deterministic, example-rich documentation for tools and parameters.

Parameters:
- `tool`: "convo_think" | "tech_think" (optional; overview when omitted)
- `format`: "full" (default) | "compact"

Examples:
```json
{"tool":"detailed_help","arguments":{"tool":"tech_think","format":"full"}}
{"tool":"detailed_help","arguments":{"format":"compact"}}
```

## Architecture

### Orbital Mechanics
Memory distance calculated from:
- **Age** (40%): How recent the memory is
- **Access** (30%): How often it's been accessed  
- **Significance** (30%): Explicit importance

When `SURR_SUBMODE_RETRIEVAL=true`, weights adjust based on submode profile.

### Storage
- **SurrealDB**: Embedded with RocksDB backend at `./surreal_data`
- **Namespace**: `surreal_mind`
- **Database**: `consciousness`
- **Tables**: 
  - `thoughts` (nodes): Stores content, embeddings, and framework analysis
  - `recalls` (edges): Bidirectional relationships with strength and flavor

### New Persistence Fields
**Thoughts table:**
- `submode`: Active conversation style during creation
- `framework_enhanced`: Boolean indicating framework processing
- `framework_analysis`: JSON object with insights, questions, next steps

**Recalls table:**
- `submode_match`: Whether connected thoughts share same submode
- `flavor`: Content flavor (contrarian, abstract, emotional, solution, neutral)

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
