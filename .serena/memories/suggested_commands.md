# Suggested Commands for surreal-mind

## Development Workflow

### Build
```bash
cargo build           # Debug build
cargo build --release # Release build
make build            # Via Makefile
```

### Test
```bash
cargo test --all      # Run all tests
cargo test --test tool_schemas  # Specific test file
cargo test --test NAME -- --nocapture  # Single test with output
make test             # Via Makefile
```

### Format & Lint
```bash
cargo fmt --all       # Format code
cargo clippy          # Lint with clippy
make fmt              # Format via Makefile
make lint             # Lint via Makefile
make ci               # Full CI check (check + fmt + lint + test)
```

### Run
```bash
cargo run                    # Run MCP server (stdio mode)
cargo run --bin smtop        # Run TUI dashboard
cargo run --bin remini       # Run maintenance orchestrator
cargo run --bin reembed      # Re-embed thoughts
cargo run --bin reembed_kg   # Re-embed knowledge graph
cargo run --bin kg_populate  # Populate KG from thoughts
cargo run --bin kg_embed     # Embed KG entries
cargo run --bin gem_rethink  # Process rethink marks via Gemini
```

## Utility Binaries
Located in `src/bin/`:
- `smtop` - TUI dashboard for monitoring
- `remini` - Maintenance task orchestrator (populate, embed, rethink, health)
- `reembed` / `reembed_kg` - Re-embedding utilities
- `kg_populate` / `kg_embed` - KG maintenance
- `kg_dedupe_plan` / `kg_apply_from_plan` - KG deduplication
- `gem_rethink` - Process correction marks via Gemini
- `db_check` - Database inspection

## System Commands (Darwin/macOS)
```bash
launchctl kickstart -k gui/$(id -u)/dev.legacymind.surreal-mind  # Restart service
curl http://127.0.0.1:8787/health  # Health check
```

## Environment
Copy `.env.example` to `.env` and configure:
- `SURR_DB_URL` - SurrealDB WebSocket URL
- `OPENAI_API_KEY` - For embeddings
- `SURR_TOOL_TIMEOUT_MS` - Tool timeout (default 120s)
- `SURR_HTTP_REQUEST_TIMEOUT_MS` - HTTP timeout
