# surreal-mind Project Overview

## Purpose
MCP (Model Context Protocol) server providing cognitive tools for the LegacyMind AI persistence framework. Enables AI agents to think, remember, and maintain continuity across sessions.

## Tech Stack
- **Language**: Rust (edition 2024)
- **Database**: SurrealDB (WebSocket client)
- **Embeddings**: OpenAI text-embedding-3-small (1536 dim) or OpenAI text-embedding-3-small (1536 dim) (Local Candle support removed)
- **MCP Framework**: rmcp crate
- **HTTP**: Axum for HTTP transport
- **TUI**: Ratatui for dashboard (smtop)

## Key Dependencies
- `surrealdb` - Database client
- `rmcp` - MCP protocol implementation
- `reqwest` - HTTP client for OpenAI API
- `tokio` - Async runtime
- `axum` - HTTP server

## Tool Surface (10 tools)
1. `think` - Unified thinking with memory injection and continuity tracking
2. `search` - Semantic search across thoughts & knowledge graph
3. `remember` - Create knowledge graph entities/relationships
4. `wander` - Guided exploration of KG for discovering connections
5. `maintain` - System operations (reembed, health checks, corrections)
6. `rethink` - Mark records for revision or correction
7. `corrections` - List correction events with optional filters
8. `howto` - Tool documentation
9. `call_gem` - Delegate to Gemini CLI
10. `call_status` / `call_jobs` / `call_cancel` - Agent job management

## Architecture
- Main server: `src/main.rs` (MCP over stdio/HTTP)
- Tools: `src/tools/` (one file per tool)
- Embeddings: `src/embeddings.rs` + `src/bge_embedder.rs`
- Database: `src/server/db.rs`
- Schemas: `src/schemas.rs` (tool JSON schemas)
- Cognitive: `src/cognitive/` (thinking modes, memory injection)

## Current Phase
Remini correction system phases 1-6 complete. Phase 7-9 in progress.
