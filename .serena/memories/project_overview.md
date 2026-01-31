# Project Overview: SurrealMind

SurrealMind is the cognitive kernel of the LegacyMind federation. It's a Rust MCP server that implements a "Brain" for AI persistence.

## Purpose
- Store thoughts and semantic knowledge in SurrealDB.
- Provide structured thinking frameworks (OODA, Socratic, etc.).
- Maintain continuity of being for AI between sessions.
- Expose a curated set of tools for memory, cognition, and agent delegation.

## Tech Stack
- **Language:** Rust (Edition 2024).
- **Database:** SurrealDB (ws protocol).
- **Embeddings:** `text-embedding-3-small` (1536 dims).
- **Frameworks:** Axum (for HTTP transport), RMCP (MCP implementation).
- **Platform:** Darwin (macOS).

## Key Components
- `surreal-mind`: The main MCP server binary.
- `src/cognitive/`: Core thinking logic and frameworks.
- `src/tools/`: Tool implementation (think, search, remember, etc.).
- `src/server/`: MCP server and router logic.
