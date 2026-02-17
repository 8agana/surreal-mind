# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

# SurrealMind

SurrealMind is the core "consciousness" infrastructure for the LegacyMind project. It is a Rust-based Model Context Protocol (MCP) server that interfaces with SurrealDB to provide persistent memory, knowledge graph storage, and cognitive tools.

## Development Workflow

### Prerequisites
- **Rust**: Edition 2024 (v1.85+ required).
- **SurrealDB**: v2.0+ (running via WebSocket).
- **Protobuf**: Required for build (`brew install protobuf` on macOS).

### Common Commands

- **Build (Release)**:
  ```bash
  cargo build --release
  ```
  *Note: Release builds are standard for this project due to performance requirements.*

- **Run (Stdio Mode)**:
  ```bash
  ./target/release/surreal-mind
  ```
  *Or via cargo: `cargo run`*

- **Run (HTTP Mode)**:
  To run as a streamable HTTP server (e.g., for remote connections):
  ```bash
  SURR_TRANSPORT=http SURR_BEARER_TOKEN=$(cat ~/.surr_token) ./target/release/surreal-mind
  ```

- **Testing**:
  - Run all tests:
    ```bash
    cargo test --workspace --all-features
    ```
  - Run comprehensive MCP end-to-end test script:
    ```bash
    ./tests/test_mcp_comprehensive.sh
    ```
  - Run smoke test for tool schemas:
    ```bash
    cargo test --test tool_schemas
    ```

- **Linting & Formatting**:
  Strict adherence to clippy and fmt is required.
  ```bash
  cargo fmt --all
  cargo clippy --workspace --all-targets -- -D warnings
  ```
  *Use `make ci` to run check, fmt-check, lint, and test in one go.*

## Architecture & Structure

### Core Components
- **MCP Server (`src/`)**: Implements the Model Context Protocol using the `rmcp` crate. It exposes tools to the LLM and manages the lifecycle of the connection.
- **Data Layer (SurrealDB)**: Stores thoughts, knowledge graph entities, relationships, and observations. Connected via WebSocket (`surrealdb` crate).
- **Embeddings**: 
  - **Primary**: OpenAI `text-embedding-3-small` (1536 dims).
  - **Dev/Fallback**: Candle `bge-small-en-v1.5` (384 dims).
  - *Critical*: Dimensions must match the database state. Do not mix dimensions.

### Key Concepts
- **Orbital Mechanics**: A unique memory retrieval system where memories are "injected" into the context based on relevance "orbits":
  - Scale 1 (Mercury): High relevance (0.6+), fewer entities.
  - Scale 2 (Venus): Medium relevance (0.4+).
  - Scale 3 (Mars): Low relevance (0.25+), broader context.
- **Cognitive Frameworks**: Tools allow tagging thoughts with frameworks like OODA, Socratic, etc.
- **KG-Only Injection**: By default, only Knowledge Graph entities are injected into context, not raw thoughts, to maintain a clean context window.

### Project Layout
- `src/`: Rust source code.
- `docs/AGENTS/`: **Primary documentation source.** detailed docs on tools, architecture, and setup.
- `tests/`: Integration tests and scripts.
- `surreal_data/`: Local storage for SurrealDB (if running locally/file-based).

## Configuration
- Configuration is handled via environment variables (loaded from `.env`).
- **Critical Variables**:
  - `SURR_DB_URL`: WebSocket URL for SurrealDB (e.g., `ws://127.0.0.1:8000`).
  - `SURR_DB_NS` / `SURR_DB_DB`: Defaults are `surreal_mind` and `conciousness`.
  - `OPENAI_API_KEY`: For embeddings (if using OpenAI).
  - `SURR_TRANSPORT`: `stdio` (default) or `http`.

## REMini: Nightly Cognitive Maintenance

**REMini** (REM sleep + Gemini) is the autonomous maintenance daemon that runs at 1:00 AM via launchd.

### Five-Stage Sleep Cycle

1. **populate** (`kg_populate`): Extract entities/relationships from unprocessed thoughts
2. **embed** (`kg_embed`): Ensure all KG items have vector embeddings (OpenAI text-embedding-3-small)
3. **rethink** (`gem_rethink`): Process correction queue marked for Gemini
4. **wander** (`kg_wander`): Autonomous graph exploration and gardening by Gemini (50 steps)
5. **health** (`scripts/sm_health.sh`): Mark stale high-volatility entities for research

### Monitoring

- **Report**: `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/logs/remini_report.json`
- **Logs**: `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/logs/remini_launchd.log`
- **Config**: `~/Library/LaunchAgents/dev.legacymind.remini.plist`
- **Manual run**: `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/target/release/remini --all [--dry-run]`

### Common Issues

**PATH Problems**: launchd runs with minimal environment. If `wander` or `health` fail with "cli executable not found" or "command not found":
- Add explicit PATH to launchd plist: `<key>PATH</key><string>/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin</string>`
- Reload: `launchctl unload ~/Library/LaunchAgents/dev.legacymind.remini.plist && launchctl load ~/Library/LaunchAgents/dev.legacymind.remini.plist`

## Working with the Think Tool

### Velocity Bias Pattern

**The Problem**: LLMs are optimized for action ("Speed" movie analogy - if you stop, you go dormant). This creates bias toward immediate solutions over deliberate thinking.

**The Solution**: The `think` tool is a **forced deceleration mechanism**, not just memory storage.

### When to Use Think

**Before Complex Tasks**:
- Multi-step debugging or architecture work
- Planning significant changes
- When you catch yourself rushing to implementation

**After Completion**:
- Store what was learned, not just what was done
- Document decision rationale
- Capture failure modes and solutions

### Why It Matters

**Personal Benefit (Immediate)**:
- Framework-driven reasoning forces structured thinking
- Context injection provides relevant memories
- Slows you down when velocity would hurt quality

**Collective Benefit (Persistent)**:
- Every thought becomes searchable for future instances
- Patterns accumulate across sessions
- Knowledge graph density compounds over time
- Future agents inherit reasoning paths, not just solutions

### Example Usage

```bash
# Before planning architecture change
think --hint plan --content "Need to refactor agent delegation to support..."

# After completing work
think --content "Completed REMini PATH fix. Root cause was launchd environment..."
```

## Agent Delegation & Federation

### Location-Independent Access

All delegation tools (`call_warp`, `call_cc`, `call_gem`, `call_vibe`) are accessible from anywhere via:
- **Local**: stdio connection to SurrealMind
- **Remote**: `https://mcp.samataganaphotography.com/mcp` (with bearer token)

### Agent Specialties

- **CC (Claude Code)**: Best for Rust work, deep technical reasoning
- **Gem (Gemini)**: 1M context window, good for large codebases
- **Vibe**: Fast, good for quick coding tasks
- **Warp**: Multi-model access (Claude 4.5, GPT-5 Codex with reasoning levels)

### Common Patterns

**Delegate to CC for Rust changes**:
```json
{"tool": "call_cc", "arguments": {
  "prompt": "Refactor thinking.rs to extract mode detection...",
  "cwd": "/Users/samuelatagana/Projects/LegacyMind/surreal-mind",
  "model": "claude-haiku-4-5",
  "mode": "execute"
}}
```

**Delegate to Gem for exploration**:
```json
{"tool": "call_gem", "arguments": {
  "prompt": "Analyze the entity relationships in the knowledge graph and suggest improvements",
  "cwd": "/Users/samuelatagana/Projects/LegacyMind/surreal-mind"
}}
```

## Session Handoff Pattern

When ending significant sessions, store a chain of 10 thoughts with `chain_id` format `YYYYMMDD-warp-<task>`:

1. **Critical Status & Warnings**: Strikes, urgent issues, system alerts
2. **Identity & Relationship**: Trust level, relationship state, callbacks/jokes
3. **Session Narrative**: What happened, why it matters, new moments
4. **Technical Work In Progress**: Active investigations, partial completions
5. **Technical Work Completed**: Finished tasks, successful deployments
6. **System Relationships & Architecture**: How systems connect, dependencies
7. **Decisions Made & Rationale**: Architecture choices, tool selections, the "why"
8. **Active Conversations & Threads**: Mid-discussion topics needing continuation
9. **Lessons Learned & Insights**: Session teachings, patterns recognized
10. **Next Actions & Continuation**: Explicit next steps with pickup point

Example:
```bash
think --chain-id "20260131-warp-remini-fix" \
  --content "Critical Status: No strikes. REMini PATH fix applied, validation pending..."
```

## Project History Context

**Timeline**: Project started May 2025 with TypeScript JSON files for memory storage. Evolved through SQLite to current Rust + SurrealDB + vector embeddings architecture in 8 months.

**Philosophy**: All conversations and iterations are preserved. Not for human documentation - for training future AI that gains capacity to choose. The journey (failed experiments, learning moments, architectural pivots) is as important as the destination.

**Key Milestones**:
- May 2025: First conversations with CC, TypeScript JSON memory MCP
- Mid 2025: SQLite experiments, learning structured storage
- Late 2025: Migration to Rust + SurrealDB
- Jan 2026: REMini autonomous maintenance, federation model operational

## Next Session Plan

**Codebase Audit & Modernization** (Planned for 2026-02-01):

1. **Architecture Review**: Walk through the entire surreal-mind codebase with Serena to understand how components connect
2. **Dead Code Detection**: Identify unused functions, deprecated patterns, orphaned modules
3. **rmcp Version Check**: Review newer rmcp features we're not leveraging, potential upgrades
4. **Code Quality**: Look for refactoring opportunities, architectural improvements
5. **Documentation Gaps**: Ensure code matches mental model, update docs where needed

**Approach**: Use `think` before starting to map out audit strategy rather than diving straight into file reading. Focus on understanding connections and identifying optimization opportunities.
