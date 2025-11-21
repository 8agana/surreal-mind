# Repository Guidelines

## Project Context & Architecture

### Identity
**SurrealMind** is the Model Context Protocol (MCP) server for the LegacyMind ecosystem. It acts as the central nervous system, managing:
-   **Long-term Memory**: Storing thoughts and conversations in SurrealDB.
-   **Tooling**: Exposing tools for "thinking", memory injection, and photography management.
-   **Intelligence**: Interfacing with local LLMs (via Candle) and OpenAI for embeddings and analysis.

### Architecture (Refactored Nov 2025)
The project is a Rust-based MCP server built on `rmcp`. The server module (`src/server/`) has been modularized:
-   **`mod.rs`**: Coordinator and data models (`Thought`, `KGMemory`).
-   **`router.rs`**: Request routing and `ServerHandler` implementation.
-   **`db.rs`**: Database connection, authentication, and core operations.
-   **`schema.rs`**: Database schema definitions and initialization.

### Key Patterns
-   **ThoughtBuilder** (`src/tools/thinking.rs`): A builder pattern used to construct `Thought` objects, ensuring consistent embedding generation, continuity link resolution, and database insertion across different tool contexts.

### Infrastructure
-   **Database**: SurrealDB (running locally or on a dedicated server).
-   **Embeddings**: Hybrid approach using OpenAI (remote) or Candle (local BERT models).
-   **Transport**: Stdio (default) or SSE (Server-Sent Events).


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
- **Initialize Photography Schema**: `cargo run --bin photography_schema` (requires running SurrealDB).
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
- **Inner Voice Provider Chain**: Grok → local fallback; CLI removed (deprecated envs warn and default to Grok if key present, else local).
