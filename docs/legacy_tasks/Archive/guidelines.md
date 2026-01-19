# Surreal Mind — Development Guidelines

This document captures project-specific knowledge to accelerate development, testing, and debugging of the Surreal Mind MCP server.

Audience: experienced Rust developers. Focus: details unique to this repository and its workflows.


## 1. Build and Configuration

- Toolchain: Rust 1.75+ with Cargo.
- Build:
  - Debug: `cargo build`
  - Release: `cargo build --release`
  - Make targets (shortcut):
    - `make build` → `cargo build`
    - `make run` → `cargo run`
    - `make check` → `cargo check --all`
    - `make fmt` / `make fmt-check`
    - `make lint` → clippy with `-D warnings`
    - `make ci` → check + fmt-check + clippy + tests

### Runtime configuration (environment)
The server reads all configuration from environment variables and .env (if present).

- NOMIC_API_KEY
  - Optional. If set (and non-placeholder), embeddings are fetched from Nomic Atlas.
  - If missing/placeholder, the server falls back to deterministic FakeEmbedder (768-D), suitable for local testing.

- SurrealDB (service mode via WebSocket)
  - The server connects to a running SurrealDB instance over ws.
  - Defaults (can be overridden via env):
    - `SURR_DB_URL=127.0.0.1:8000`
    - `SURR_DB_USER=root`
    - `SURR_DB_PASS=root`
    - `SURR_DB_NS=surreal_mind`
    - `SURR_DB_DB=consciousness`
    - `SURR_DB_LIMIT=500` (cap for fallback query when cache is empty)
  - Start SurrealDB locally:
    - `surreal start --user root --pass root --bind 127.0.0.1:8000`

- Retrieval tuning (used by memory injection):
  - `SURR_SIM_THRESH` (0.0–1.0, default 0.5): cosine similarity threshold.
  - `SURR_TOP_K` (1–50, default 5): max memories to inject.

- Logging:
  - `RUST_LOG` default: `surreal_mind=debug,rmcp=info`
  - `MCP_NO_LOG=1` disables tracing initialization (useful for clean JSON streams in tests/integration).

- .env support:
  - The binary loads `.env` automatically if present (`dotenv::dotenv().ok()`).


## 2. Testing

### At a glance
- Unit/integration tests via Cargo work without a DB by default.
- Any test requiring DB is guarded by `RUN_DB_TESTS` env and will be skipped unless you set it.
- External protocol tests are provided as shell scripts; these require the MCP server to run and (by default) a SurrealDB service.

### Running tests
- All tests: `cargo test --all`
- Lint + tests: `make ci`
- Run with logs enabled: `RUST_LOG=debug cargo test` (note: logs go to stderr)
- Enable DB-backed tests explicitly:
  - Start SurrealDB: `surreal start --user root --pass root --bind 127.0.0.1:8000`
  - Export connection env if customizing (see above)
  - Run: `RUN_DB_TESTS=1 cargo test`

The code has built-in unit tests under `src/main.rs` (cfg(test)) for:
- Parameter deserialization
- Data structure invariants
- Cosine similarity behavior
- Embeddings fallback determinism
- Server initialization (only when `RUN_DB_TESTS` is set)

### Adding tests
- Prefer placing new integration tests under `tests/` as separate files. These are compiled as standalone crates against the library/binary code.
- Avoid hard dependency on external services in default test runs. If a test needs SurrealDB, gate it behind an env check:

  Example pattern (validated locally):
  
  ```rust
  #[test]
  fn sanity_math() {
      assert_eq!(2 + 2, 4);
  }
  
  #[tokio::test]
  async fn requires_db() {
      if std::env::var("RUN_DB_TESTS").is_err() {
          eprintln!("skipping DB test — set RUN_DB_TESTS=1 to enable");
          return; // or use `return` early
      }
      // ... connect/use DB here ...
  }
  ```

- Run a single test: `cargo test test_name_substring`
- Show stdout during tests: `cargo test -- --nocapture`

### Example: create and run a simple test
The following example was verified during this session.

1) Create `tests/sanity.rs` with:

```rust
#[test]
fn sanity_math() {
    assert_eq!(2 + 2, 4);
}
```

2) Run it:

```
cargo test --quiet
```

3) Expected result includes `test result: ok. 1 passed` for the new test. Remove the example afterwards if not needed.

Note: In this session, an equivalent temporary test file was added, executed successfully, and then removed to keep the repo clean.

### Protocol test scripts (MCP)
- Quick smoke test (runs the server and performs initialize/tools/list): `./test_simple.sh`
- Filtered JSON response view via jq: `./test_mcp.sh`
- Comprehensive run against release binary: `./test_mcp_comprehensive.sh` (expects `./target/release/surreal-mind`)

These scripts expect:
- A running SurrealDB service (see config above), otherwise `cargo run`/the binary will fail early when connecting.
- For comprehensive script, build first: `cargo build --release`
- Optional: set `MCP_NO_LOG=1` for cleaner JSON-only output (the server normally logs to stderr).


## 3. Additional Development Notes

- Server capabilities and MCP behavior:
  - The server deliberately sets `tools.list_changed = false` in `list_tools` capabilities to accommodate clients that wait on list_changed notifications.
  - `initialize` echoes back the client’s protocol version to minimize client compatibility issues.

- Schema management:
  - On startup, the server initializes SurrealDB schema for tables `thoughts` and `recalls` (id/fields and indexes) if missing.
  - Graph edges are created both directions via a single multi-statement query for efficiency.

- Embeddings:
  - Fake embeddings are deterministic given the same input and fixed dimensionality, which keeps unit tests stable without network or API keys.
  - Nomic usage can be toggled via `NOMIC_API_KEY`; timeouts and error surfaces are explicit for easier debugging.

- Retrieval/injection controls:
  - Injection scale (0–5) maps to allowed orbital distance thresholds. In-memory cache is preferred; the DB is queried as a bounded fallback (`SURR_DB_LIMIT`).
  - Similarity threshold and top-k are tunable at runtime via env variables (see above) to experiment locally without recompiling.

- Logging and diagnostics:
  - Tracing is initialized unless `MCP_NO_LOG` is set. Default filter: `surreal_mind=debug,rmcp=info`.
  - Many operations log at `info` with sensitive payloads redacted; `debug` includes more detail (e.g., content preview lengths), but not full user content at info level.

- Style & quality:
  - Use `make fmt` and `make lint` before submitting changes. CI target in Makefile bundles check+fmt-check+clippy+tests for local preflight.

- Common pitfalls:
  - Running `cargo run` without SurrealDB will error during startup (WebSocket connection/auth). This is expected; use the test gating or start the DB.
  - If you use `jq` in shell test scripts, ensure it’s installed; otherwise, remove the pipe to `jq`.


## 4. Quick Commands Reference
- Build/run:
  - `cargo build`; `cargo run`
  - `cargo build --release` then `./target/release/surreal-mind`
- Start DB: `surreal start --user root --pass root --bind 127.0.0.1:8000`
- Tests:
  - `cargo test --all`
  - `RUN_DB_TESTS=1 cargo test` (with DB running)
  - `./test_simple.sh` (smoke MCP)
  - `./test_mcp_comprehensive.sh` (after release build)
- Quality: `make ci` (check + fmt-check + clippy + tests)

