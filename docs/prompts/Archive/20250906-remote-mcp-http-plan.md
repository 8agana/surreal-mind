# Remote MCP over HTTP — Implementation Plan (surreal-mind)

Owner: Codex • Date: 2025-09-04

Goal: Replace UI2’s Cloudflared‑exposed MCP endpoint with surreal‑mind over HTTP, without changing the public hostname. Keep stdio as the default; gate HTTP behind env/config with strict auth and minimal logs. Axum is the primary transport; Actix is a watch item only.

## Scope
- Primary: Axum + rmcp streamable HTTP server (consistent with UI2).
- Optional (feature‑gated): actix‑web via `rmcp-actix-web` for a thinner, SSE‑ready path.

## Non‑Goals
- No behavior change for stdio clients.
- No public exposure without bearer auth; no secrets in logs.

## Design Overview
- Transport selection via env: `SURR_TRANSPORT=stdio|http` (default: stdio).
- HTTP server binds at `SURR_HTTP_BIND` (default `127.0.0.1:8788`). For cutover, run on `127.0.0.1:8787` to reuse Cloudflared.
- MCP mounted at `SURR_HTTP_PATH` (default `/mcp`).
- Bearer auth from `SURR_BEARER_TOKEN` (fallback `~/.surr_token`). Optional query‑param token only if `SURR_ALLOW_TOKEN_IN_URL=1`.
- Health/info/metrics endpoints outside the MCP path.
- SSE keep‑alive configurable (`SURR_HTTP_SSE_KEEPALIVE_SEC`, default 15).
- Logging: honor `MCP_NO_LOG` for stdio; HTTP logs at error by default (no embeddings or secrets).

## Env / Config Surface
- `SURR_TRANSPORT` (stdio|http) — default stdio.
- `SURR_HTTP_BIND` (SocketAddr) — default `127.0.0.1:8788`.
- `SURR_HTTP_PATH` (string) — default `/mcp`.
- `SURR_BEARER_TOKEN` (string) — bearer required; fallback `~/.surr_token`.
- `SURR_ALLOW_TOKEN_IN_URL` (bool) — default false.
- `SURR_HTTP_SSE_KEEPALIVE_SEC` (u64) — default 15.
 - `SURR_HTTP_SESSION_TTL_SEC` (u64) — default 900. Session eviction for stateful mode.
 - `SURR_HTTP_REQUEST_TIMEOUT_MS` (u64) — default 10000. Applies to non‑MCP endpoints.
 - `SURR_HTTP_MCP_OP_TIMEOUT_MS` (u64) — optional; default disabled. If set, cancels long MCP tool ops.
 - `SURR_HTTP_METRICS_MODE` (basic|prom) — default basic.

## Axum Implementation (Primary)
1) Dependencies
   - `rmcp = { version = "0.6.1", features = ["macros","transport-io","transport-streamable-http-server"] }`
   - `axum = { version = "0.7", default-features = false, features = ["http1","json","tokio"] }`

2) Module `src/http.rs`
   - Build `StreamableHttpService<SurrealMindServer, LocalSessionManager>` with `stateful_mode=true` and `sse_keep_alive` from env.
   - Mount under `SURR_HTTP_PATH` using `axum::Router::nest_service`.
   - Add middleware for bearer auth (header `Authorization: Bearer <token>`, optional `?access_token=` when enabled). Allow `/health` without auth.
   - Add `GET /health` → "ok".
   - Add `GET /info` → JSON summary `{ embedding:{provider,model,dim}, db:{url,ns,db}, inner_voice:{enabled,topk_default,mix}, indexes_ok }`.
   - Add `GET /metrics` → `{ total_requests, last_request_unix }` (counts MCP path only).

3) Main switch in `src/main.rs`
   - Read `SURR_TRANSPORT`; if `http`, call `start_http_server(config, server)`; else keep current stdio path.
   - Keep `MCP_NO_LOG` semantics intact for stdio; do not print protocol noise.
   - Default bind `127.0.0.1:8787` in deploy docs to drop‑in replace UI2 behind Cloudflared.

4) Security & Ops
   - Require bearer for all MCP calls; 401 otherwise.
   - Document Cloudflared mapping in README (hostname → `http://127.0.0.1:8788`).
   - Recommend CF Access/JWT or mTLS for stronger perimeter if exposed.

## Actix Variant (Optional, Feature‑Gated — watch only)
- Defer implementation; keep Axum primary. Track `rmcp-actix-web` maturity and rmcp compatibility (0.6.x). If adopted later, gate behind `http-actix` with identical env surface.

## Testing Strategy
- Unit-lite: auth middleware unit tests (header/query cases), config parsing.
- Integration: spin HTTP on `127.0.0.1:0` → perform MCP handshake (`initialize`, `tools/list`) and a `tools/call` against `think_convo`.
- Auth paths: 401 with no/invalid token, 200 with header token, query token only when enabled.
- Metrics: counter increments only on MCP path; `/health` excluded.
- Concurrency: simple parallel `tools/call` check (stateful sessions OK).
 - DB: prefer MCP‑only smoke tests that do not require DB (initialize, tools/list). For tool calls requiring DB, run behind `SURREAL_TEST_URL` and mark as `#[ignore]` in CI unless available.

## Clarifications (Per Grok Review)

1) Session Management
- Use `LocalSessionManager` in stateful mode and add a lightweight TTL eviction loop: every `min(60, TTL/3)` seconds, drop sessions idle beyond `SURR_HTTP_SESSION_TTL_SEC` (default 900s). SSE keep‑alives reset last‑seen.

2) DB Connection Reuse
- The HTTP transport shares a single `SurrealMindServer` instance constructed in `main.rs`. It reuses the existing `Arc<Surreal<Client>>` created from `config.system.database_url`. No duplicate pools are created; requests clone the `Arc` only.

3) Error Responses
- Non‑MCP endpoints return standard JSON errors: `{ "error": { "code": <int>, "message": <string> } }` with appropriate HTTP status codes (e.g., 401, 500).
- MCP path is handled by rmcp; errors propagate as MCP error frames.

4) Timeouts
- Non‑MCP endpoints are wrapped with `tower::timeout` using `SURR_HTTP_REQUEST_TIMEOUT_MS` (default 10s).
- Tool execution already respects `tool_timeout_ms` from runtime config; we will not impose an HTTP‑layer timeout on streaming MCP by default. Optional kill‑switch via `SURR_HTTP_MCP_OP_TIMEOUT_MS` for runaway ops.

5) Metrics Granularity
- Keep plan’s basic counters and add error count and simple latency tracking (rolling avg and last N samples) exposed in `/metrics` when `SURR_HTTP_METRICS_MODE=basic`.
- Leave Prometheus histograms under a future `prom` mode.

6) Integration Tests
- Include an HTTP smoke test that boots on `127.0.0.1:0` with a stub embedder and exercises: `/health`, `initialize`, `tools/list`.
- For DB‑touching tool calls, run only when `SURREAL_TEST_URL` is set; otherwise skip to keep CI hermetic.

## Acceptance Criteria
- Default: `cargo run` starts stdio MCP exactly as today.
- HTTP mode: server binds at requested addr; `/mcp` serves MCP; `/health` returns 200; `/info` and `/metrics` return JSON.
- Auth: Missing/invalid token → 401; valid header bearer → OK; query token works only when `SURR_ALLOW_TOKEN_IN_URL=1`.
- 401 payload now follows OAuth style: `{"error":"invalid_token","error_description":"Unauthorized"}` so clients like Claude Code parse it cleanly.
- Logging: No embeddings/secrets in logs; stdio honors `MCP_NO_LOG`.

## Cutover From UI2 (Cloudflared)

Cloudflared today: `~/.cloudflared/config.yml` maps `mcp.samataganaphotography.com` → `http://localhost:8787`.

Path A — Port Reuse (no Cloudflared change)
- Stop UI2 on 8787.
- Start surreal‑mind on 8787:
  `SURR_TRANSPORT=http SURR_HTTP_BIND=127.0.0.1:8787 SURR_HTTP_PATH=/mcp SURR_BEARER_TOKEN=$(cat ~/.surr_token) cargo run -p surreal-mind`
- Verify local: `/health`, `/info`, `/metrics`.
- Verify remote: `https://mcp.samataganaphotography.com/info` with bearer.

Path B — New Port then Flip
- Run surreal‑mind on 8788; validate locally.
- Update Cloudflared service to `http://localhost:8788`; restart tunnel.
- Verify remote; decommission UI2.

Readiness checks
- `GET /health` → 200.
- `tools/list` over HTTP returns surreal‑mind tool catalog.
- Auth enforced (401 without bearer).
- SSE keep‑alives working (default 15s) through Cloudflare.

Rollback
- Path A: stop surreal‑mind; start UI2 on 8787.
- Path B: revert Cloudflared service to 8787.

## Rollout Steps
1) Land code behind env switch; stdio remains default.
2) Smoke test HTTP locally on target port (8787 for swap).
3) Execute Path A or B; validate remote via Cloudflared.
4) Update README and AGENTS.md with HTTP quick start and cutover notes.

## Future Enhancements
- CF Access JWT validation middleware.
- mTLS listener variant.
- Prometheus exporter for metrics.
- Rate limiting on MCP path.
- Track `rmcp-actix-web` as an alternative transport; if adopted later, keep the same env surface and Cloudflared mapping.
