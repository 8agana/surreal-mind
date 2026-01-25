# Dependency Reference

This document outlines the system and software dependencies required to build and run `surreal-mind`. Use this as a reference when configuring build environments or troubleshooting dependency issues.

## System Dependencies

### Runtime Environment

* **Rust Toolchain**: Stable channel (Edition 2024).
* **SurrealDB**: Version 2.0+ (Protocol: WebSocket `ws://`).
  * The application operates as a client (`surrealdb` crate); a running SurrealDB server instance is required for persistence.

### Build Dependencies

* **Cargo & Rustc**: Managed via `rustup`.
* **Platform Libraries**:
  * **macOS**: typically requires `xcode-select --install` for basic build tools.
  * **Linux**: May require `build-essential`, `pkg-config`, and `libssl-dev` (though `rustls` is preferred, some transitive dependencies may link against system OpenSSL).

## Application Libraries (Crates)

The project leverages the Rust ecosystem for core functionality. See `Cargo.toml` for precise version pinning.

### Core Architecture

* **[rmcp](https://crates.io/crates/rmcp)**: The application's backbone, implementing the Model Context Protocol (MCP).
  * *Features:* `macros`, `transport-io`, `transport-streamable-http-server`.
* **[tokio](https://crates.io/crates/tokio)**: Asynchronous runtime handling I/O, scheduling, and timers.
* **[axum](https://crates.io/crates/axum)**: High-level HTTP server framework used for the HTTP transport layer.
* **[tower](https://crates.io/crates/tower)**: Middleware primitives (Service trait) used for request timeouts and layers.

### Data & Persistence

* **[surrealdb](https://crates.io/crates/surrealdb)**: The primary database driver/client.
  * *Critical Feature:* `protocol-ws` (WebSocket) is required for the connection logic used in `src/db/`.
* **[serde](https://crates.io/crates/serde)** / **[serde_json](https://crates.io/crates/serde_json)**: Universal serialization framework.
* **[dashmap](https://crates.io/crates/dashmap)**: Concurrent associative arrays (likely used for in-memory caching or state).

### Utilities & Tooling

* **[clap](https://crates.io/crates/clap)**: Command-line argument parser (v4).
* **[tracing](https://crates.io/crates/tracing)**: Structural logging and diagnostics.
* **[chrono](https://crates.io/crates/chrono)** / **[time](https://crates.io/crates/time)**: Date and time libraries.
* **[reqwest](https://crates.io/crates/reqwest)**: HTTP Client (configured with `rustls-tls` to reduce system dependency on OpenSSL).
* **[regex](https://crates.io/crates/regex)**: Used for cognitive heuristics and text parsing.

## Environment & Configuration

Runtime behavior is heavily influenced by environment variables, loaded via **[dotenvy](https://crates.io/crates/dotenvy)**.

* **Config Source**: `.env` file (see `.env.example`).
* **Critical Variables**:
  * `SURR_BRAIN_URL`: Connection string for SurrealDB.
  * `SURREAL_OPENAI_API_KEY`: Required if using embedding/vector search features.

## Inspecting Dependencies

To view the full dependency tree including transitive crates:

```bash
cargo tree
```

To find why a specific crate is included (e.g., `rusqlite`):

```bash
cargo tree -i rusqlite
```
