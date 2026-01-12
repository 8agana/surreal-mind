# SurrealMind – Consciousness Persistence MCP Server

SurrealMind is the LegacyMind federation's cognitive kernel: a Rust MCP server that stores thoughts and knowledge in SurrealDB, injects relevant memories with orbital mechanics, and exposes 10 curated tools for continuity.

## What It Does

- **Unified thinking** (`think`) with continuity links, optional hypothesis verification, and KG-only injection.
- **Retrieval and synthesis** (`search`) with injection scales and filters.
- **Knowledge graph authoring** (`remember`).
- **Curiosity-driven exploration** (`wander`) for discovering connections.
- **Operations and introspection** (`maintain`, `howto`).
- **Agent delegation** (`call_gem`, `call_status`, `call_jobs`, `call_cancel`).
- Transports: stdio by default or streamable HTTP with SSE and bearer auth.

## Transports

### Stdio (default)

- Run `./target/release/surreal-mind` (or `cargo run`) and connect with any MCP client.
- Honors `MCP_NO_LOG=1` to keep stdout pure MCP. When `SURR_WRITE_STATE=1`, writes `~/Library/Application Support/surreal-mind/state.json` with pid/start time for discovery.

### Streamable HTTP (Axum + SSE)

- Set `SURR_TRANSPORT=http` before launch. Key envs:
  - `SURR_HTTP_BIND` (default `127.0.0.1:8787`)
  - `SURR_HTTP_PATH` (default `/mcp`)
  - `SURR_BEARER_TOKEN` or `~/.surr_token` (required). `SURR_ALLOW_TOKEN_IN_URL=1` enables `?access_token=` for compatibility.
  - `SURR_HTTP_SSE_KEEPALIVE_SEC` (default 15), `SURR_HTTP_SESSION_TTL_SEC` (default 900), `SURR_HTTP_REQUEST_TIMEOUT_MS` and optional `SURR_HTTP_MCP_OP_TIMEOUT_MS`.
  - `SURR_HTTP_METRICS_MODE` (`basic` default).
- Endpoints:
  - `GET /health` (no auth)
  - `GET /info` (embedding + DB snapshot, auth required)
  - `GET /metrics` (counts, p95 latency, top tools, auth required)
  - `GET /db_health` (optional DB ping/counts when `SURR_DB_STATS=1`, auth required)
- MCP entrypoint mounted at `${SURR_HTTP_PATH}` with SSE keepalive.

## Quick Start

1) Prereqs: Rust 1.85+, SurrealDB 2.0 (ws or wss). `protobuf` via `brew install protobuf` if protoc errors.
2) Configure:

   ```bash
   cd surreal-mind
   cp .env.example .env
   export OPENAI_API_KEY=sk-...
   export SURR_DB_URL=ws://127.0.0.1:8000
   export SURR_DB_USER=root SURR_DB_PASS=root
   ```

3) Run SurrealDB (in-memory example):

   ```bash
   surreal start --user root --pass root --bind 127.0.0.1:8000 memory
   ```

4) Build:

   ```bash
   cargo build --release
   ```

5) Launch (stdio):

   ```bash
   ./target/release/surreal-mind
   ```

   Launch over HTTP:

   ```bash
   SURR_TRANSPORT=http SURR_BEARER_TOKEN=$(cat ~/.surr_token) \
   SURR_HTTP_BIND=127.0.0.1:8787 ./target/release/surreal-mind
   ```

6) Smoke tests:

   ```bash
   cargo test --test tool_schemas
   ./tests/test_mcp_comprehensive.sh
   ```

## Tool Surface (10)

| Tool | Description |
|------|-------------|
| `think` | Unified thinking with continuity links, hypothesis verification, KG injection. Required: `content`. Optional: `hint`, `injection_scale` 0–3, `tags[]`, `significance`, continuity fields. |
| `search` | Unified KG + thoughts search. Optional: `query`, `target`, `include_thoughts`, `top_k_memories/thoughts`, similarity/confidence filters, and `forensic` mode for provenance. |
| `remember` | Create KG `entity\|relationship\|observation`. Supports `upsert`, `source_thought_id`, `confidence`, `data`. |
| `wander` | Explore the knowledge graph. Modes: `random`, `semantic`, `meta`, `marks`. Returns actionable guidance for KG improvement. |
| `rethink` | Revise or mark knowledge graph items for correction. Modes: `mark` (flag for review), `correct` (apply fix with provenance). |
| `corrections`| List recent `correction_events` to inspect the learning journey of the KG. |
| `maintain` | System maintenance: `health_check_embeddings`, `reembed`, `reembed_kg`, `list_removal_candidates`, `export_removals`, `finalize_removal`, `echo_config`, `rethink`, `populate`, `embed`, `wander`, `health`, `report`, `tasks`. |
| `howto` | Get help for any tool. Optional: `tool`, `format` (`compact\|full`). |
| `call_gem` | Delegate prompts to Gemini CLI. Required: `prompt`. Optional: `task_name`, `model`, `cwd`, `timeout_ms`. |
| `call_status` | Check status of a background agent job. Required: `job_id`. |
| `call_jobs` | List active/recent agent jobs. Optional: `limit`, `status_filter`, `tool_name`. |
| `call_cancel` | Cancel a running agent job. Required: `job_id`. |

## Configuration Quick Reference

- Database: `SURR_DB_URL` (ws/wss/http/https), `SURR_DB_NS`, `SURR_DB_DB`, `SURR_DB_USER`, `SURR_DB_PASS`, `SURR_DB_TIMEOUT_MS`, `SURR_DB_SERIAL` (serialize queries), `SURR_DB_RECONNECT`.
- Embeddings: `SURR_EMBED_PROVIDER=openai|candle`, `SURR_EMBED_MODEL`, `SURR_EMBED_STRICT`, `SURR_SKIP_DIM_CHECK`, `SURR_EMBED_RETRIES`, `SURR_EMBED_DIM` (inferred), `OPENAI_API_KEY`. Primary: text-embedding-3-small (1536); Candle dev: bge-small-en-v1.5 (384). Never mix dims—reembed when switching.
- Retrieval/injection: `SURR_INJECT_T1/T2/T3` (defaults 0.6/0.4/0.25), `SURR_INJECT_FLOOR` (0.15), `SURR_KG_CANDIDATES` (default 200), `SURR_RETRIEVE_CANDIDATES` (default 500), `SURR_CACHE_MAX` (5000), `SURR_CACHE_WARM` (64), `SURR_INJECT_DEBOUNCE`, `SURR_KG_GRAPH_BOOST`, `SURR_KG_MAX_NEIGHBORS`, `SURR_KG_TIMEOUT_MS`.
- Runtime/logging: `SURR_TOOL_TIMEOUT_MS` (default 15000), `MCP_NO_LOG`, `RUST_LOG`, `SURR_WRITE_STATE=1` to emit state.json.
- Hypothesis verification defaults: `SURR_VERIFY_TOPK` (100), `SURR_VERIFY_MIN_SIM` (0.70), `SURR_VERIFY_EVIDENCE_LIMIT` (10), `SURR_PERSIST_VERIFICATION`.

- Brain datastore: `SURR_ENABLE_BRAIN`, `SURR_BRAIN_URL/NS/DB/USER/PASS`.
- HTTP transport: `SURR_TRANSPORT`, `SURR_HTTP_BIND`, `SURR_HTTP_PATH`, `SURR_BEARER_TOKEN` or `~/.surr_token`, `SURR_ALLOW_TOKEN_IN_URL`, `SURR_HTTP_SSE_KEEPALIVE_SEC`, `SURR_HTTP_SESSION_TTL_SEC`, `SURR_HTTP_REQUEST_TIMEOUT_MS`, `SURR_HTTP_MCP_OP_TIMEOUT_MS`, `SURR_HTTP_METRICS_MODE`.

## Memory Model

- Injection scales: 1→5 entities @0.6, 2→10 @0.4, 3→20 @0.25. Floor `SURR_INJECT_FLOOR` clamps low-sim hits. KG-only injection by default.

## Binaries

- `surreal-mind` (MCP server, stdio or http)
- `reembed`, `reembed_kg` (dimension hygiene)
- `kg_apply_from_plan`, `kg_dedupe_plan`, `kg_populate`, `kg_embed` (KG ops)
- `kg_debug_tool`, `kg_wander` (exploration/debugging)
- `migration`, `smtop`, `admin` (consolidated admin utilities)

Run with `cargo build --release` to produce all.

## Testing & CI

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-features
./tests/test_mcp_comprehensive.sh               # MCP end-to-end
```

## Change Log Highlights

- 2026-01-06: Tool rename (v0.7.5): `think`, `search`, `remember`, `wander`, `maintain`, `howto`, `call_*`. Dead code cleanup (~220 lines removed).
- 2026-01-02: Documentation synced with codebase; added agent job tools.
- 2025-11-29: Cognitive kernel cleanup; legacy photography binaries removed.
- 2025-11-24: Photography split finalized; all photo MCP tools removed (now in photography-mind).

## License

Part of the LegacyMind project.
