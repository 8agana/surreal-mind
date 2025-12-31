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
cargo run --bin reembed      # Re-embed thoughts
cargo run --bin reembed_kg   # Re-embed knowledge graph
```

## Utility Binaries
Located in `src/bin/`:
- `smtop` - TUI dashboard for monitoring
- `reembed` / `reembed_kg` - Re-embedding utilities
- `db_check` / `check_db_contents` - Database inspection
- `kg_inspect` / `kg_dedupe_plan` / `kg_apply_from_plan` - KG maintenance
- `test_gemini` - Test Gemini CLI integration

## System Commands (Darwin/macOS)
```bash
launchctl kickstart -k gui/$(id -u)/dev.legacymind.surreal-mind  # Restart service
curl http://127.0.0.1:8787/health  # Health check
```

## Environment
Copy `.env.example` to `.env` and configure:
- `SURR_DB_URL` - SurrealDB WebSocket URL
- `OPENAI_API_KEY` - For embeddings
- `GEMINI_TIMEOUT_MS` - Gemini CLI timeout (default 60s)
