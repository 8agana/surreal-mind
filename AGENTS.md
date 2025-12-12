# SurrealMind Agent Guide

Single-mind MCP server for LegacyMind. Photography tooling was split out (Nov 2025); this surface now exposes seven thinking/ops tools only. KG-only injection, strict embedding hygiene, and minimal blast radius remain the defaults.

## Tool Surface
- `legacymind_think` — unified thinking; continuity fields (`session_id`, `chain_id`, `previous_thought_id`, `revises_thought`, `branch_from`); `hint` (`debug|build|plan|stuck|question|conclude`); `injection_scale` 0–3; `tags`, `significance`; optional hypothesis verification (`needs_verification`, `verify_top_k`, `min_similarity`, `evidence_limit`, `contradiction_patterns`).
- `legacymind_search` — KG + optional thoughts search; `query`, `target`, `include_thoughts`, `thoughts_content`, `top_k_memories/thoughts`, `sim_thresh`, confidence/date bounds, chain/session filters.
- `inner_voice` — retrieval + synthesis with optional auto KG extraction; knobs `top_k`, `floor`, `mix`, `include_private`, tag include/exclude, `auto_extract_to_kg`, `previous_thought_id`, `include_feedback`. Provider chain: Grok primary → local fallback. Gate with `SURR_ENABLE_INNER_VOICE` / `SURR_DISABLE_INNER_VOICE`. Auto-extraction now depends on the LLM appending a JSON `candidates` block at the end of the answer; no heuristic extractor runs.
- `memories_create` — create KG entities/relationships/observations; `upsert`, `source_thought_id`, `confidence`, `data`.
- `memories_moderate` — review/decide staged KG candidates; `action` (`review|decide|review_and_decide`), `target`, `status`, `min_conf`, paging; decisions payload.
- `maintenance_ops` — ops subcommands: `list_removal_candidates`, `export_removals`, `finalize_removal`, `health_check_embeddings`, `health_check_indexes`, `reembed`, `reembed_kg`, `ensure_continuity_fields`, `echo_config`.
- `detailed_help` — deterministic schemas/prompts.

## Transports & Auth
- **Stdio (default):** run `./target/release/surreal-mind` (or `cargo run`). `MCP_NO_LOG=1` keeps stdio clean. `SURR_WRITE_STATE=1` emits `~/Library/Application Support/surreal-mind/state.json` for discovery.
- **HTTP (streamable, SSE):** set `SURR_TRANSPORT=http`. Env: `SURR_HTTP_BIND` (default `127.0.0.1:8787`), `SURR_HTTP_PATH` (default `/mcp`), `SURR_BEARER_TOKEN` or `~/.surr_token` (required), `SURR_ALLOW_TOKEN_IN_URL=1` to accept `?access_token=`, `SURR_HTTP_SSE_KEEPALIVE_SEC`, `SURR_HTTP_SESSION_TTL_SEC`, `SURR_HTTP_REQUEST_TIMEOUT_MS`, optional `SURR_HTTP_MCP_OP_TIMEOUT_MS`, `SURR_HTTP_METRICS_MODE`. Endpoints: `/health` (no auth), `/info`, `/metrics`, `/db_health` (auth).
 - **HTTP restart (after rebuild):** if running via launchd, `launchctl kickstart -k gui/$(id -u)/dev.legacymind.surreal-mind`; manual run: `SURR_TRANSPORT=http SURR_BEARER_TOKEN=$(cat ~/.surr_token) SURR_HTTP_BIND=127.0.0.1:8787 ./target/release/surreal-mind`.

## Config Guardrails
- **Embeddings:** primary OpenAI `text-embedding-3-small` (1536); dev Candle `bge-small-en-v1.5` (384). No mixed dims; reembed when switching. Envs: `SURR_EMBED_PROVIDER`, `SURR_EMBED_MODEL`, `SURR_EMBED_STRICT`, `SURR_SKIP_DIM_CHECK`, `SURR_EMBED_RETRIES`, `OPENAI_API_KEY`.
- **Database:** `SURR_DB_URL` (ws/wss/http/https), `SURR_DB_NS`, `SURR_DB_DB`, `SURR_DB_USER/PASS`; `SURR_DB_TIMEOUT_MS`, `SURR_DB_SERIAL` when WS deadlocks; optional `SURR_DB_RECONNECT`.
- **Retrieval:** `SURR_INJECT_T1/T2/T3` (0.6/0.4/0.25), `SURR_INJECT_FLOOR` (0.15), `SURR_KG_CANDIDATES`, `SURR_RETRIEVE_CANDIDATES`, `SURR_CACHE_MAX/WARM`, `SURR_KG_MAX_NEIGHBORS`, `SURR_KG_GRAPH_BOOST`, `SURR_KG_TIMEOUT_MS`. KG-only injection by default.
- **Verification:** `SURR_VERIFY_TOPK`, `SURR_VERIFY_MIN_SIM`, `SURR_VERIFY_EVIDENCE_LIMIT`, `SURR_PERSIST_VERIFICATION`.
- **Inner Voice:** `SURR_INNER_VOICE_TOPK_DEFAULT`, `SURR_INNER_VOICE_MIX`, `SURR_INNER_VOICE_MIN_FLOOR`, `SURR_INNER_VOICE_MAX_CANDIDATES_PER_SOURCE`, `SURR_IV_PLAN`, `SURR_IV_AUTO_EXTRACT_KG`, `IV_ALLOW_GROK`, `INNER_VOICE_LOCAL_FALLBACK`.
- **Brain datastore:** `SURR_ENABLE_BRAIN`, `SURR_BRAIN_URL/NS/DB/USER/PASS`.
- **Runtime/logging:** `SURR_TOOL_TIMEOUT_MS`, `RUST_LOG`, `MCP_NO_LOG`.
- **HTTP:** `SURR_TRANSPORT`, `SURR_HTTP_*` (bind/path/token/keepalive/ttl/timeouts/metrics), `SURR_ALLOW_TOKEN_IN_URL`.

## Ops & Binaries (target/release)
- Server: `surreal-mind` (stdio/http).
- Hygiene: `reembed`, `reembed_kg`, `fix_dimensions`.
- Connectivity: `db_check`, `check_db_contents`, `simple_db_test`.
- KG utilities: `kg_inspect`, `kg_apply_from_plan`, `kg_dedupe_plan`.
- Misc: `migration`, `smtop`, `sanity_cosine`.
- `maintenance_ops` health checks: `health_check_embeddings`, `health_check_indexes`; `echo_config` for runtime snapshot; re-embed subcommands for repairs.

## Dev Workflow
- Build: `cargo build --release`
- Format: `cargo fmt --all`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Tests: `cargo test --workspace --all-features` and `./tests/test_mcp_comprehensive.sh`

## Guardrails
- KG-only injection; no fake/deterministic embedders; maintain provider/dimension hygiene.
- Defaults favor local stdio + ws DB; secure HTTP with bearer if exposed.
- Photography lives in `photography-mind`; do not reintroduce photo MCP tools here.
