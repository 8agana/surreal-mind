# Setup / Quickstart

- Build: `cargo build --release`
- Format: `cargo fmt --all`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Tests: `cargo test --workspace --all-features` (plus `./tests/test_mcp.sh` when applicable). Use targeted binaries/tests for narrower verification when needed.
- Run stdio: `./target/release/surreal-mind`
- Run HTTP: `SURR_TRANSPORT=http SURR_BEARER_TOKEN=$(cat ~/.surr_token) SURR_HTTP_BIND=127.0.0.1:8787 ./target/release/surreal-mind`
- Env templates: see `docs/AGENTS/connections.md` for transport, `docs/AGENTS/arch.md` for embeddings defaults. Keep `~/.surr_token` present for HTTP.
- Database baseline: SurrealDB 3.0+.
- Embeddings: OpenAI `text-embedding-3-small` (1536) primary. No mixed dims—re-embed if switching providers/models.
- Google CLI delegation: `kg_populate` and `kg_wander` select `antigravity` or `gemini` with `SM_AGENT_PROVIDER`, `GOOGLE_CLI_PROVIDER`, or `SURR_GOOGLE_CLI_PROVIDER`; default is `antigravity`, with `gemini` retained as rollback. For Antigravity, complete `agy` browser sign-in in the same logged-in GUI/user context and verify `agy --print "Reply with exactly: ok" --print-timeout 30s` works after restart/reboot.
- Keep root clean: AGENTS.md (index), README.md (human quick start), CHANGELOG.md (version history), .env.example (never commit secrets).
