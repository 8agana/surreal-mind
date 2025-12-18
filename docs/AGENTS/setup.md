# Setup / Quickstart

- Build: `cargo build --release`
- Format: `cargo fmt --all`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Tests: `cargo test --workspace --all-features` (plus `./tests/test_mcp_comprehensive.sh` when applicable)
- Run stdio: `./target/release/surreal-mind`
- Run HTTP: `SURR_TRANSPORT=http SURR_BEARER_TOKEN=$(cat ~/.surr_token) SURR_HTTP_BIND=127.0.0.1:8787 ./target/release/surreal-mind`
- Env templates: see `Docs/AGENTS/connections.md` for transport, `Docs/AGENTS/arch.md` for embeddings defaults. Keep `~/.surr_token` present for HTTP.
- Embeddings: OpenAI `text-embedding-3-small` (1536) primary; Candle `bge-small-en-v1.5` (384) dev. No mixed dims—re-embed if switching providers.
- Keep root clean: AGENTS.md (index), README.md (human quick start), CHANGELOG.md (version history), .env.example (never commit secrets).
