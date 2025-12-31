# SurrealMind – Consciousness Persistence MCP Server

SurrealMind is the LegacyMind federation's thinking surface: a Rust MCP server that stores thoughts and knowledge in SurrealDB, injects relevant memories with orbital mechanics, and exposes eight curated tools for continuity.

## What It Does
- Unified thinking (`legacymind_think`) with continuity links, optional hypothesis verification, and KG-only injection.
- Retrieval and synthesis (`legacymind_search`) with injection scales and filters.
- Knowledge graph authoring (`memories_create`, `memories_moderate`).
- Operations and introspection (`maintenance_ops`, `detailed_help`).
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

## Tool Surface (9)
- `legacymind_think`: `content` required. Optional `hint` (`debug|build|plan|stuck|question|conclude`), `injection_scale` 0–3, `tags[]`, `significance`, continuity fields (`session_id`, `chain_id`, `previous_thought_id`, `revises_thought`, `branch_from`), `hypothesis` + `needs_verification` with `verify_top_k`, `min_similarity`, `evidence_limit`, `contradiction_patterns`, `verbose_analysis`.
- `legacymind_search`: Unified KG search; `query` (text/struct), `target` (`entity|relationship|observation|mixed`), `include_thoughts`, `thoughts_content`, `top_k_memories/thoughts`, `sim_thresh`, confidence/date bounds, chain/session filters.
- `memories_create`: Create KG `entity|relationship|observation`; supports `upsert`, `source_thought_id`, `confidence`, `data`.
- `delegate_gemini`: Delegate prompts to Gemini CLI with persisted exchange tracking. `prompt` required, optional `task_name`, `model`.
- `curiosity_add`: Add lightweight curiosity entries. `content` required, optional `tags[]`, `agent`, `topic`, `in_reply_to`.
- `curiosity_get`: Fetch recent curiosity entries. Optional `limit` (1-100, default 20), `since` (YYYY-MM-DD).
- `curiosity_search`: Search curiosity entries via embeddings. `query` required, optional `top_k`, `recency_days`.
- `maintenance_ops`: Operational subcommands:
  - `list_removal_candidates`, `export_removals`, `finalize_removal`
  - `health_check_embeddings`, `health_check_indexes`
  - `reembed`, `reembed_kg`, `ensure_continuity_fields`
  - `echo_config` (safe runtime snapshot)
- `detailed_help`: Deterministic schemas for tools/prompts.

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

## Binaries (target/release)
- `surreal-mind` (MCP server, stdio or http)
- `reembed`, `reembed_kg`, `fix_dimensions` (dimension hygiene)
- `db_check`, `check_db_contents`, `simple_db_test` (connectivity smoke)
- `kg_inspect`, `kg_apply_from_plan`, `kg_dedupe_plan` (KG ops)
- `migration`, `smtop`, `sanity_cosine`
Run with `cargo build --release` to produce all.

## Testing & CI
```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-features           # 52 tests currently
./tests/test_mcp_comprehensive.sh               # MCP end-to-end
```

## Change Log Highlights
- 2025-11-29: Cognitive kernel cleanup; legacy photography binaries removed; tool surface fixed at 8 (now 7 after brain_store removal on 2025-12-05).
- 2025-11-24: Photography split finalized; all photo MCP tools removed (now in photography-mind).
- 2025-11-20–22: Safety hardening, fuzzy competition matching (now lives in photography-mind), clippy clean.

## License
Part of the LegacyMind project.
