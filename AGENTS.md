# Repository Guidelines

## File Organization
- **Test Files**: All test files should go into the `/tests` folder 

## Project Structure & Module Organization

- **Source code**: Located in `src/` directory, with main modules including `main.rs`, `lib.rs`, `config.rs`, `embeddings.rs`, `schemas.rs`, and subdirectories for specific functionality.
- **Modules**: `bin/` for binaries, `cognitive/` for cognitive processing, `frameworks/` for thinking frameworks, `server/` for MCP server logic, `tools/` for MCP tool implementations, `utils/` for utility functions.
- **Tests**: Integration and unit tests in `tests/` directory.
- **Configuration**: Environment variables via `.env` file, with `surreal_mind.toml` as fallback.
- **Assets**: Local models in `models/`, database data in `surreal_data/`.

## Build, Test, and Development Commands

- **Build**: `cargo build` (development) or `cargo build --release` (production binary).
- **Run MCP server**: `cargo run` (stdio mode).
- **Test**: `cargo test --workspace --all-features` (includes integration tests).
- **Format code**: `cargo fmt --all`.
- **Lint**: `cargo clippy --workspace --all-targets -- -D warnings` (warnings treated as errors).
- **Check compilation**: `cargo check --workspace --locked --all-targets`.
- **Example**: After changes, run `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`.

## Coding Style & Naming Conventions

- **Language**: Rust (edition 2024).
- **Formatting**: Use `cargo fmt` (rustfmt) for consistent indentation (4 spaces) and line wrapping.
- **Linting**: Clippy rules enforced; treat warnings as errors.
- **Naming**: snake_case for functions/variables/modules, CamelCase for types/structs, UPPER_SNAKE_CASE for constants.
- **Imports**: Group by std, external crates, local modules; sort alphabetically.
- **Documentation**: Use `///` for public items; keep comments concise and actionable.
- **Examples**: Variable: `embedding_provider`, Struct: `EmbeddingConfig`, Constant: `DEFAULT_DIM`.

## Testing Guidelines

- **Framework**: Built-in Rust tests (`#[test]`).
- **Coverage**: Aim for unit tests near modules; integration tests in `tests/` for end-to-end.
- **Isolation**: Mock external dependencies (e.g., DB, embeddings); avoid network calls in unit tests.
- **Naming**: `test_function_name` or `test_case_description`.
- **Running tests**: `cargo test --workspace --all-features`; use `RUST_LOG=debug cargo test -- --nocapture` for logs.
- **CI**: Tests run in GitHub Actions with matrix for different embedders.

## Commit & Pull Request Guidelines

- **Commit messages**: Concise, imperative mood, e.g., "Add photography database health check" or "Refactor code for clarity".
- **PR template**: Include Why, Scope, Safety, Testing checkboxes.
- **Requirements**: Run `cargo fmt`, `cargo clippy`, `cargo test` before submitting.
- **Reviews**: Small, focused PRs preferred; link related issues.
- **Branching**: Feature branches; suggest branch name in PR description.

## Security & Configuration Tips

- **Environment-first**: Configure via env vars (e.g., `OPENAI_API_KEY`); avoid hardcoding.
- **Embeddings**: Use OpenAI primary; Candle for dev; maintain dimension hygiene (filter by `embedding_dim`).
- **Secrets**: Never log API keys; use query-param warnings for HTTP auth.
- **Guardrails**: No fake/deterministic embedders; KG-only injection; respect provider/model/dim stamps.
- **DB**: WebSocket connection to SurrealDB; health checks via `maintenance_ops`.
