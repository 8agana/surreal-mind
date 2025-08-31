# Repository Guidelines

## Project Structure & Module Organization
- `src/`: Rust sources
  - `src/main.rs`: MCP server entrypoint (stdio transport, dotenv, logging)
  - `src/embeddings.rs`: `Embedder` trait and providers (OpenAI/Nomic)
- `tests/`: Integration tests; unit tests live alongside modules with `#[cfg(test)]`.
- `.cargo/config.toml`: Cargo aliases (e.g., `cargo lint`, `cargo ci`).
- `.env.example` → `.env`: Local configuration and secrets (not committed).

## Build, Test, and Development Commands
- Build: `cargo build` (release: `cargo build --release`).
- Run (loads `.env`): `cargo run`.
- Run with logs: `RUST_LOG=surreal_mind=debug,rmcp=info cargo run`.
- Format: `make fmt` (runs `cargo fmt --all`).
- Lint: `make lint` (Clippy with `-D warnings`).
- CI bundle: `make ci` (check, fmt --check, clippy, tests).
- Tests: `cargo test --all` | single: `cargo test test_list_tools_returns_convo_think`.
- With logs: `RUST_LOG=debug cargo test -- --nocapture`.

## Coding Style & Naming Conventions
- Rustfmt as source of truth; run `make fmt` before pushing.
- Clippy clean: no warnings allowed.
- Naming: modules/files `snake_case`; functions/vars `snake_case`; types/traits `PascalCase`; constants `SCREAMING_SNAKE_CASE`.
- Keep functions small; return `anyhow::Result` on main paths; prefer `thiserror`/`McpError` for protocol errors.

## Testing Guidelines
- Unit tests close to code; integration tests in `tests/`.
- No external network in tests; mock embeddings and DB interactions.
- Name tests `test_*` and keep them deterministic.

## Commit & Pull Request Guidelines
- Commits: imperative mood, concise subject (≤72 chars), useful body; reference issues (e.g., `Closes #123`).
- PRs: clear description, motivation, and testing steps; link issues; include logs/screenshots if relevant.
- Requirements: `make ci` passes; update docs when behavior or interfaces change.

## Security & Configuration Tips
- Copy `.env.example` to `.env` before running.
- Key vars: `OPENAI_API_KEY` or `SURR_EMBED_PROVIDER=nomic` with `NOMIC_API_KEY`; SurrealDB: `SURR_DB_URL`, `SURR_DB_NS`, `SURR_DB_DB`, `SURR_DB_USER`, `SURR_DB_PASS`; tuning: `SURR_DB_LIMIT`, `SURR_SIM_THRESH`, `SURR_TOP_K`.
- Do not commit secrets; rotate keys if exposed.

## Architecture Overview
- Rust MCP server persisting “thoughts” to SurrealDB with an in-memory LRU cache for fast retrieval.
- Embeddings via OpenAI or Nomic.
- Exposed tools: `convo_think`, `tech_think`, `search_thoughts`, `detailed_help`.

