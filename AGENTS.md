# Repository Guidelines

## Project Structure & Module Organization
- Source code: `src/` with the binary entrypoint in `src/main.rs`. The MCP server (`SurrealMindServer`) currently exposes one tool: `convo_think`.
- Package manifest: `Cargo.toml` (Rust 2024 edition). Key crates: `rmcp` (MCP), `surrealdb` (RocksDB backend enabled), `tokio`, `serde`, `tracing`.
- Build artifacts: `target/` (debug/release outputs).
- Tests: inline `#[cfg(test)]` modules or integration tests in `tests/`.

## Build, Test, and Development Commands
- Build: `cargo build` (use `--release` for optimized binary).
- Run (stdio MCP server): `cargo run`.
- Test: `cargo test` (add tests under `tests/` or alongside modules).
- Format: `cargo fmt` (ensure rustfmt is installed via `rustup component add rustfmt`).
- Lint: `cargo clippy -- -D warnings` (install with `rustup component add clippy`).

## Coding Style & Naming Conventions
- Indentation: 4 spaces; keep lines readable (<100 cols preferred).
- Naming: `snake_case` for functions/modules/files, `CamelCase` for types/traits, `SCREAMING_SNAKE_CASE` for constants.
- Formatting: keep code `cargo fmt`-clean; fix clippy warnings.
- Patterns: return `anyhow::Result` from async entrypoints; use `tracing` (`info!`, `warn!`, `error!`) instead of `println!`.

## Testing Guidelines
- Framework: built-in Rust tests. Use `#[tokio::test]` for async tests.
- Placement: integration tests in `tests/` (e.g., `tests/convo_think_test.rs`); unit tests via `mod tests { ... }` next to code.
- Running examples:
  - All tests: `cargo test`
  - By name: `cargo test convo_think`
  - With logs: `RUST_LOG=debug cargo test -- --nocapture`

## Commit & Pull Request Guidelines
- Commits: prefer Conventional Commits (e.g., `feat: add convo_think tool`, `fix: handle missing content`). Make logical, minimal commits.
- PRs: include a clear description, linked issues, test plan/output, and any relevant logs or screenshots. Keep scope focused and update docs when behavior changes.
- CI expectations: code formatted, `clippy` clean, and `cargo test` passing.

## Security & Configuration Tips
- Transport is stdio; avoid logging sensitive data. Default log filters favor `surreal_mind=debug,rmcp=info`—lower verbosity for production.
- `surrealdb` with `kv-rocksdb` is enabled but not wired yet; when persisting, keep DB paths out of VCS and prefer env-driven configuration.
