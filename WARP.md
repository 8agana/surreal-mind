# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

# SurrealMind

SurrealMind is the core "consciousness" infrastructure for the LegacyMind project. It is a Rust-based Model Context Protocol (MCP) server that interfaces with SurrealDB to provide persistent memory, knowledge graph storage, and cognitive tools.

## Development Workflow

### Prerequisites
- **Rust**: Edition 2024 (v1.85+ required).
- **SurrealDB**: v2.0+ (running via WebSocket).
- **Protobuf**: Required for build (`brew install protobuf` on macOS).

### Common Commands

- **Build (Release)**:
  ```bash
  cargo build --release
  ```
  *Note: Release builds are standard for this project due to performance requirements.*

- **Run (Stdio Mode)**:
  ```bash
  ./target/release/surreal-mind
  ```
  *Or via cargo: `cargo run`*

- **Run (HTTP Mode)**:
  To run as a streamable HTTP server (e.g., for remote connections):
  ```bash
  SURR_TRANSPORT=http SURR_BEARER_TOKEN=$(cat ~/.surr_token) ./target/release/surreal-mind
  ```

- **Testing**:
  - Run all tests:
    ```bash
    cargo test --workspace --all-features
    ```
  - Run comprehensive MCP end-to-end test script:
    ```bash
    ./tests/test_mcp_comprehensive.sh
    ```
  - Run smoke test for tool schemas:
    ```bash
    cargo test --test tool_schemas
    ```

- **Linting & Formatting**:
  Strict adherence to clippy and fmt is required.
  ```bash
  cargo fmt --all
  cargo clippy --workspace --all-targets -- -D warnings
  ```
  *Use `make ci` to run check, fmt-check, lint, and test in one go.*

## Architecture & Structure

### Core Components
- **MCP Server (`src/`)**: Implements the Model Context Protocol using the `rmcp` crate. It exposes tools to the LLM and manages the lifecycle of the connection.
- **Data Layer (SurrealDB)**: Stores thoughts, knowledge graph entities, relationships, and observations. Connected via WebSocket (`surrealdb` crate).
- **Embeddings**: 
  - **Primary**: OpenAI `text-embedding-3-small` (1536 dims).
  - **Dev/Fallback**: Candle `bge-small-en-v1.5` (384 dims).
  - *Critical*: Dimensions must match the database state. Do not mix dimensions.

### Key Concepts
- **Orbital Mechanics**: A unique memory retrieval system where memories are "injected" into the context based on relevance "orbits":
  - Scale 1 (Mercury): High relevance (0.6+), fewer entities.
  - Scale 2 (Venus): Medium relevance (0.4+).
  - Scale 3 (Mars): Low relevance (0.25+), broader context.
- **Cognitive Frameworks**: Tools allow tagging thoughts with frameworks like OODA, Socratic, etc.
- **KG-Only Injection**: By default, only Knowledge Graph entities are injected into context, not raw thoughts, to maintain a clean context window.

### Project Layout
- `src/`: Rust source code.
- `docs/AGENTS/`: **Primary documentation source.** detailed docs on tools, architecture, and setup.
- `tests/`: Integration tests and scripts.
- `surreal_data/`: Local storage for SurrealDB (if running locally/file-based).

## Configuration
- Configuration is handled via environment variables (loaded from `.env`).
- **Critical Variables**:
  - `SURR_DB_URL`: WebSocket URL for SurrealDB (e.g., `ws://127.0.0.1:8000`).
  - `SURR_DB_NS` / `SURR_DB_DB`: Defaults are `surreal_mind` and `conciousness`.
  - `OPENAI_API_KEY`: For embeddings (if using OpenAI).
  - `SURR_TRANSPORT`: `stdio` (default) or `http`.
