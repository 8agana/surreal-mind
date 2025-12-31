# surreal-mind Project Overview

## Purpose
MCP (Model Context Protocol) server providing cognitive tools for the LegacyMind AI persistence framework. Enables AI agents to think, remember, and maintain continuity across sessions.

## Tech Stack
- **Language**: Rust (edition 2024)
- **Database**: SurrealDB (WebSocket client)
- **Embeddings**: OpenAI text-embedding-3-small (1536 dim) or Candle BGE (local, 384 dim)
- **MCP Framework**: rmcp crate
- **HTTP**: Axum for HTTP transport
- **TUI**: Ratatui for dashboard (smtop)

## Key Dependencies
- `surrealdb` - Database client
- `rmcp` - MCP protocol implementation
- `candle-*` - Local ML inference (Metal-accelerated on macOS)
- `reqwest` - HTTP client for OpenAI API
- `tokio` - Async runtime
- `axum` - HTTP server

## Tool Surface (9 tools)
1. `legacymind_think` - Cognitive processing with memory injection
2. `legacymind_search` - Unified search across thoughts and memories
3. `memories_create` - Create knowledge graph entities/relationships
4. `maintenance_ops` - Operational commands (reembed, health checks, etc.)
5. `detailed_help` - Tool documentation
6. `delegate_gemini` - Delegate to Gemini CLI
7. `curiosity_add/get/search` - Lightweight curiosity entries

## Architecture
- Main server: `src/main.rs` (MCP over stdio/HTTP)
- Tools: `src/tools/` (one file per tool)
- Embeddings: `src/embeddings.rs` + `src/bge_embedder.rs`
- Database: `src/server/db.rs`
- Schemas: `src/schemas.rs` (tool JSON schemas)
