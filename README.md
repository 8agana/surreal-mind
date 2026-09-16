# SurrealMind – Consciousness Persistence MCP Server

SurrealMind is the LegacyMind federation's cognitive kernel: a Rust MCP server that stores thoughts and knowledge in SurrealDB, injects relevant memories with orbital mechanics, and exposes 10 curated tools for continuity.

## What It Does

- **Unified thinking** (`think`) with continuity links, optional hypothesis verification, and KG-only injection.
- **Retrieval and synthesis** (`search`) with injection scales and filters.
- **Knowledge graph authoring** (`remember`).
- **Curiosity-driven exploration** (`wander`) for discovering connections.
- **Operations and introspection** (`maintain`, `howto`).
- Transports: stdio by default or streamable HTTP with SSE, bearer auth, and an OAuth 2.1 endpoint set for remote MCP clients.

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
  - `SURR_HTTP_ALLOWED_HOSTS` (rmcp 3.1.4+ `Host`-header allowlist, a DNS-rebinding defense): `localhost`/`127.0.0.1`/`::1` are always accepted; this is a comma-separated list of *additional* hosts that extends that loopback set — it never replaces it. A value that is set but empty, or that contains an empty entry (stray/leading/trailing comma), fails startup loudly instead of silently degrading to loopback-only or accept-all. For the public Cloudflare tunnel to reach `/mcp`, the deployed value must include `mcp.samataganaphotography.com` (set in launchd/env configuration, not this repo). This allowlist applies only to the nested `/mcp` Streamable HTTP route; `/health`, `/info`, `/metrics`, and `/db_health` are plain routes and are not Host-validated by this mechanism.
  - `SURR_OAUTH_ISSUER` (default `http://${SURR_HTTP_BIND}`), `SURR_OAUTH_CLIENT_ID`, `SURR_OAUTH_CLIENT_SECRET`.
- Endpoints:
  - `GET /health` (no auth — the **only** path exempt from the bearer middleware)
  - `GET /info` (embedding + DB snapshot, auth required)
  - `GET /metrics` (counts, p95 latency, top tools, auth required)
  - `GET /db_health` (DB ping plus thought/entity counts when reachable, auth required)
- MCP entrypoint mounted at `${SURR_HTTP_PATH}` with SSE keepalive.

### OAuth 2.1 (remote MCP clients)

- Mounted automatically whenever a bearer token is configured — i.e. on every HTTP launch. Implements the OAuth 2.1 subset the MCP spec requires (RFC 8414 metadata, RFC 7591 dynamic registration, PKCE) so claude.ai's MCP connector proxy can attach.
- Single-user server: `/authorize` auto-approves every request. No consent screen, no per-user identity.
- **These four routes deliberately require no bearer auth — they are how a client obtains one:**
  - `GET /.well-known/oauth-authorization-server` (RFC 8414 metadata)
  - `GET /authorize` (auto-approves, redirects with code; PKCE `S256`)
  - `POST /token` (grants `authorization_code`, `refresh_token`, `client_credentials`; returns the configured bearer token)
  - `POST /register` (RFC 7591; returns the pre-configured client rather than minting a new one)
- Envs: `SURR_OAUTH_ISSUER` (defaults to `http://${SURR_HTTP_BIND}`), `SURR_OAUTH_CLIENT_ID`, `SURR_OAUTH_CLIENT_SECRET`. If the id/secret are unset the server generates ephemeral UUIDs at startup, logs a warning, and loses them on restart — set both for any persistent remote client.
- Because `/token` hands out the server's bearer token to a caller presenting valid client credentials, treat the client secret as equal in sensitivity to `~/.surr_token`, and do not expose the bind address beyond a trusted proxy.

## Quick Start

1) Prereqs: Rust 1.85+, SurrealDB 3.0+ (ws or wss). See [docs/DEPENDENCIES.md](docs/DEPENDENCIES.md) for a complete list of system and crate dependencies.
   `protobuf` via `brew install protobuf` if protoc errors.
2) Configure:

   ```bash
   cd surreal-mind
   cp .env.example .env
   export OPENAI_API_KEY=sk-...
   export SURR_DB_URL=127.0.0.1:8000   # host:port only, no scheme
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
   ./tests/test_mcp.sh
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
| `maintain` | System maintenance: `health_check_embeddings`, `reembed`, `reembed_kg`, `embed_pending`, `list_removal_candidates`, `export_removals`, `finalize_removal`, `echo_config`, `rethink`, `populate`, `embed`, `wander`, `health`, `report`, `tasks`. |
| `journal` | Research thread management over the KG: create threads, add entries, review dashboard state, and update thread status. |
| `howto` | Get help for any tool. Optional: `tool`, `format` (`compact\|full`). |
| `test_notification` | Emit an MCP logging notification to the connected client. Required: `message`. Optional: `level` (`debug\|info\|notice\|warning\|error\|critical\|alert\|emergency`, default `info`). |

## Configuration Quick Reference

- Database: `SURR_DB_URL` (host:port only — no scheme; the WebSocket engine prepends `ws://` and appends `/rpc` itself), `SURR_DB_NS`, `SURR_DB_DB`, `SURR_DB_USER`, `SURR_DB_PASS`, `SURR_DB_RECONNECT` (`1`/`true` retries the initial connection up to 5 times; off by default). HTTP ping tuning: `SURR_DB_PING_TTL_MS` (default 1500, caches the `/info` ping result), `SURR_DB_PING_TIMEOUT_MS` (default 250). A `ws://`/`wss://`/`http://`/`https://` prefix is stripped by the MCP server but passed through verbatim by the auxiliary binaries (`kg_populate`, `reembed`, `gem_rethink`, `admin`), where it produces a malformed endpoint and the connection fails. `wss://` does **not** enable TLS — the connection is plaintext WebSocket either way. Note that the committed `surreal_mind.toml:10` still carries the scheme'd form (`ws://127.0.0.1:8000`); `SURR_DB_URL` in `.env` overrides it.
- Embeddings: `SURR_EMBED_PROVIDER` overrides `surreal_mind.toml` `[system].embedding_provider`; model and dimensions remain TOML-only (`embedding_model`, `embedding_dimensions`). Other knobs are `SURR_EMBED_STRICT`, `SURR_SKIP_DIM_CHECK`, `SURR_EMBED_RETRIES` (default 3), `SURR_RETRY_DELAY_MS` (default 500), `SURR_EMBED_RPS` (default 1.0), and `OPENAI_API_KEY`. `SURR_EMBED_DIM` is referenced in a code comment only and is read by nothing. The supported deployed path is OpenAI `text-embedding-3-small` (1536). Never mix dims—reembed when switching. The offline `fake` provider exists only when the non-default `test-embedder` Cargo feature is compiled; a plain release excludes it, while `--features test-embedder` or `--all-features` includes the arm. `SURR_ALLOW_FAKE_EMBEDDER=1` is operational opt-in policy in such a build, not an authorization boundary. (Local Candle support has been removed.)
- Retrieval/injection: `SURR_INJECT_T1/T2/T3` (defaults 0.6/0.4/0.25), `SURR_INJECT_FLOOR` (0.15), `SURR_KG_CANDIDATES` (default 200), `SURR_RETRIEVE_CANDIDATES` (default 500), `SURR_CACHE_MAX` (5000), `SURR_CACHE_WARM` (64), `SURR_INJECT_DEBOUNCE`, `SURR_KG_GRAPH_BOOST`, `SURR_KG_MAX_NEIGHBORS`, `SURR_KG_TIMEOUT_MS`.
- Runtime/logging: `SURR_TOOL_TIMEOUT_MS` (default 15000), `MCP_NO_LOG`, `RUST_LOG`, `SURR_WRITE_STATE=1` to emit state.json.
- Google CLI delegation: `SM_AGENT_PROVIDER`, `GOOGLE_CLI_PROVIDER`, or `SURR_GOOGLE_CLI_PROVIDER` selects `antigravity` or `gemini` (default `antigravity`; set `gemini` for rollback). Antigravity uses `agy --print`, defaults model to `auto`, and requires auth in the same GUI/user context as the running service. `KG_POPULATE_MODEL` and `KG_WANDER_MODEL` override model per unattended binary.
- Hypothesis verification defaults: `SURR_VERIFY_TOPK` (100), `SURR_VERIFY_MIN_SIM` (0.70), `SURR_VERIFY_EVIDENCE_LIMIT` (10), `SURR_PERSIST_VERIFICATION`.
- HTTP transport: `SURR_TRANSPORT`, `SURR_HTTP_BIND`, `SURR_HTTP_PATH`, `SURR_BEARER_TOKEN` or `~/.surr_token`, `SURR_ALLOW_TOKEN_IN_URL`, `SURR_HTTP_SSE_KEEPALIVE_SEC`, `SURR_HTTP_SESSION_TTL_SEC`, `SURR_HTTP_REQUEST_TIMEOUT_MS`, `SURR_HTTP_MCP_OP_TIMEOUT_MS`, `SURR_HTTP_METRICS_MODE`, `SURR_OAUTH_ISSUER`, `SURR_OAUTH_CLIENT_ID`, `SURR_OAUTH_CLIENT_SECRET`.

### Environment trust boundary

Inherited environment variables and dotenv files are trusted operator configuration. Dotenv values are merged into the process-global environment and become indistinguishable from exported values, so an environment-keyed opt-in is accidental-action friction, not authentication or authorization. `SURR_ENV_FILE` pins helper-routed callers to an exact dotenv file; when it is unset, `Config::load` preserves its distinct local-then-conditional-parent fallback while other shared callers use dotenv's ordinary upward search. Security-sensitive permits must use authenticated capabilities, OS/file permissions, or another explicit programmatic source dotenv cannot manufacture.

## Memory Model

- Injection scales: 1→5 entities @0.6, 2→10 @0.4, 3→20 @0.25. Floor `SURR_INJECT_FLOOR` clamps low-sim hits. KG-only injection by default.

## Binaries

- `surreal-mind` (MCP server, stdio or http)
- `reembed`, `reembed_kg` (dimension hygiene)
- `kg_apply_from_plan`, `kg_dedupe_plan`, `kg_populate`, `kg_embed` (KG ops)
- `kg_debug_tool`, `kg_wander` (exploration/debugging)
- `migration`, `admin` (consolidated admin utilities)

Run with `cargo build --release` to produce all.

## Testing & CI

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-features
./tests/test_mcp.sh                             # MCP end-to-end
```

## Change Log Highlights

- 2026-05-25: OAuth 2.1 endpoints added for remote MCP clients (`d20a3cc`).
- 2026-01-06: Tool rename (v0.7.5): `think`, `search`, `remember`, `wander`, `maintain`, `howto`, `call_*`. Dead code cleanup (~220 lines removed).
- 2025-12-12: `brain_store` tool removed along with its schema, router entry, and `SURR_ENABLE_BRAIN` / `SURR_BRAIN_*` configuration (`3dd589e`).
- 2026-01-02: Documentation synced with codebase; added agent job tools.
- 2025-11-29: Cognitive kernel cleanup; legacy photography binaries removed.
- 2025-11-24: Photography split finalized; all photo MCP tools removed (now in photography-mind).

## License

Part of the LegacyMind project.
