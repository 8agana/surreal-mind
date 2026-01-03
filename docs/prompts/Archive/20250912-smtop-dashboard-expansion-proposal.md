# smtop Dashboard Expansion Proposal

Author: Junie
Date: 2025-09-12

## Summary
smtop is already a useful TUI for monitoring the remote MCP over HTTP and the cloudflared tunnel. This proposal outlines a low-risk, incremental plan to expand smtop beyond the remote MCP view to include:

- Active session visibility (HTTP/SSE sessions now; stdio sessions next)
- SurrealDB service status and quick health indicators
- Basic MCP tool usage/error metrics and latency trends
- Local resource usage (surreal-mind process CPU/mem)
- Better log navigation and filtering

Changes are deliberately minimal and staged. Most functionality comes from extending the existing /info and /metrics endpoints and adding one optional lightweight status file for stdio transport discovery.

## Current State (as of src/bin/smtop.rs)
- Shows MCP HTTP service status: local/remote /health + latency
- Tracks MCP request count via /metrics and estimates RPS
- Displays cloudflared process status and merged logs
- Supports copying URL and bearer token, header vs query auth toggle

Back-end endpoints (src/http.rs):
- GET /health → "ok"
- GET /info → server + embedding + DB config summary
- GET /metrics → { total_requests, last_request_unix }
- Streamable HTTP MCP mounted at runtime.http_path with stateful session manager

## Goals
1) Expand observability without adding mandatory runtime dependencies
2) Keep default test runs DB-free and network-free (as per project guidelines)
3) Provide immediate value even without SurrealDB or cloudflared running
4) Make all new features optional and gated via config/env

## Proposed Features

### 1) Session Visibility
- HTTP/SSE sessions (Phase 1)
  - Add counters to /metrics:
    - http_active_sessions (current open streamable sessions)
    - http_total_sessions (cumulative since start)
  - Implementation: wire LocalSessionManager events into HttpMetrics (Arc<Mutex<_>>), or expose a small accessor to count live sessions when serving /metrics.

- Stdio sessions (Phase 2)
  - Because stdio transport has no HTTP server, smtop can discover active stdio sessions via an optional local status file written by the server in any transport mode:
    - Path: ~/.local/share/surreal-mind/state.json (macOS: ~/Library/Application Support/surreal-mind/state.json)
    - Contents (example): { "start_unix": 1694544000, "transport": "stdio", "pid": 12345, "client": "aider 0.22", "sessions": 1 }
  - Server-side: write/update file on start and session changes; remove on shutdown. Gated behind env SURR_WRITE_STATE=1 (default off). No impact on unit tests.
  - Codex: write atomically (temp file + rename) and set permissions 0600 to avoid partial reads and restrict access. Tolerate missing/invalid file in smtop.
  - smtop reads file if present and surfaces a simple “Stdio Sessions” panel.

### 2) SurrealDB Status (Phase 1)
- Extend /info to include db_connected boolean and optional ping_ms if the DB client is initialized.
  - Implementation: on handling /info, attempt a cheap no-op or select 1; time it with a short timeout (e.g., 250ms). If DB is not configured/connected, return db_connected: false.
  - Codex: cache the last successful ping in-memory with a short TTL (e.g., SURR_DB_PING_TTL_MS, default 1500ms). /info should return the cached ping when within TTL to avoid adding latency under load. Never block beyond the configured timeout.
- Optionally provide /db_health (Phase 2) with a little more detail:
  - { connected, ping_ms, ns, db, server_version?, thoughts_count?, recalls_count? }
  - counts are gated and bounded (e.g., fast COUNT estimates with LIMIT, or skip by default and enable via SURR_DB_STATS=1)

Note: All DB calls must be optional and fast; return quickly with “unknown” if not available. For tests, gate any DB-dependent logic behind RUN_DB_TESTS or feature flags so default cargo test remains offline.

### 3) MCP Tool Metrics (Phase 2)
- Extend /metrics:
  - errors_total (non-2xx or MCP-level failures)
  - avg_latency_ms (rolling)
  - p95_latency_ms (rolling; small ring buffer)
  - tools_top_5: [{ name, count }]
- Implementation: minimal counters inside the HTTP middleware around the MCP mount; record start/stop timestamps and response/error classification. For tool usage, increment by tool name inside the SurrealMindServer handler (already centralized).
  - Codex: add `metrics_version: "1"` to /metrics for forward-compatible parsing. Maintain rolling latency using a fixed-size ring buffer or O(1) exponential decay; compute p95 from the ring buffer (bounded memory).

### 4) Local Resource Usage (Phase 1)
- smtop adds a small panel that shows surreal-mind process CPU%, RSS, and uptime.
  - Implementation: use the existing is_process_running approach (pgrep) to get PID(s) and then read from /proc or use ps -o on Unix/macOS. Keep it best-effort and optional; if unavailable, show “–”.

### 5) Logs UX Improvements (Phase 1)
- Add basic filters: [all | stdout | stderr | cloudflared]
- Add jump-to-end (End) and jump-to-start (Home)
- Keep tail length bounded (already does 400); make it configurable via env SMTOP_LOG_TAIL=400

### 6) Cloudflared Details (Phase 2)
- If available, parse and display tunnel hostname and connection count from logs or cloudflared tunnel list (if CLI present). Otherwise, keep current on/off and health-only view.

## TUI Layout Proposal
- Header (unchanged): title, URL, auth mode, quick shortcuts
- Row 1 (two columns):
  - Left: MCP Service (local health, bind, path, RPS sparkline)
  - Right: Cloudflared (status, remote health, requests, tunnel info when available)
- Row 2 (two columns):
  - Left: Sessions
    - HTTP Sessions: active/total
    - Stdio Sessions: active (from state.json if present)
  - Right: SurrealDB
    - Connected: yes/no, ping ms
    - NS/DB from config
- Row 3: Logs panel (filterable; PgUp/PgDn/Home/End)
- Footer: Help/Keybindings

Key additions:
- s: cycle log source filter
- h: toggle auth header (existing)
- e: jump to end; b: jump to beginning

## Configuration and Env
- Server-side (optional):
  - SURR_DB_STATS=1 → enables richer /db_health (/info add-ons). Default off.
  - SURR_WRITE_STATE=1 → writes ~/.local/share/surreal-mind/state.json (0600, atomic replace). Default off.
  - SURR_DB_PING_TTL_MS (default 1500) → TTL for cached DB ping used by /info.
- smtop-side (optional):
  - SMTOP_LOG_TAIL (default 400)
  - SMTOP_STATUS_PATH override for state.json discovery
  - SMTOP_DB_PING_TIMEOUT_MS (default 250; only affects display rate if we ever make smtop call DB directly—prefer server-side info instead)
  - SMTOP_INFO_TTL_MS (default 1000) → optional client cache to avoid hammering /info

All new features should degrade gracefully when endpoints/files are missing.

## Implementation Plan

Phase 1 (Low risk, immediate value)
1) Extend /metrics with http_active_sessions and http_total_sessions; include `metrics_version: "1"`.
2) Extend /info with { db_connected, db_ping_ms? } using a short timeout.
3) smtop: add new panels for Sessions and DB, plus resource usage panel and log UX keys; optionally cache /info for SMTOP_INFO_TTL_MS.
4) Add env SMTOP_LOG_TAIL. Keep defaults unchanged.

Phase 2 (Optional depth)
5) Add optional state.json write (SURR_WRITE_STATE=1) for stdio session discovery; document path conventions per OS; write atomically with 0600 perms; delete on shutdown.
6) Add /db_health with richer info, gated by SURR_DB_STATS=1.
7) Extend /metrics with latency buckets, errors_total, and tools_top_5.
8) smtop: display stdio sessions when file exists; show advanced metrics conditionally.

Phase 3 (Nice-to-have)
9) Cloudflared tunnel hostname parsing and display. Use best-effort from logs or CLI if present.
10) Optional detail view for top tools/errors when pressing "t".

## Testing Plan
- Unit tests (no DB):
  - Metrics struct serialization with new fields (including metrics_version)
  - Auth header vs query token behavior unchanged
  - smtop JSON parsing resilient to missing fields
- Integration tests (no DB):
  - Start HTTP server, hit /health, /info, /metrics; assert presence of new fields with plausible values; verify /metrics served under auth
- DB-gated tests (RUN_DB_TESTS=1):
  - Verify db_connected and db_ping_ms are populated when SurrealDB is reachable; verify /info ping uses TTL cache (responses stable within TTL)
- Manual TUI smoke test: run ./test_simple.sh (requires DB for full run) and launch smtop; validate panels render with or without cloudflared

## Risks and Mitigations
- Risk: Blocking DB ping under load
  - Mitigation: very short timeout; return unknown quickly
- Risk: Stdio state file drift
  - Mitigation: atomic write (temp + rename), 0600 perms, write on changes and delete on shutdown; tolerate missing/invalid file in smtop
- Risk: Metrics growth
  - Mitigation: keep ring buffers fixed (e.g., last 60 points), counters u64
- Security: No secret leakage
  - Mitigation: never serialize bearer tokens; smtop displays token only from local user file (masked by default); respect existing header vs query toggle; /metrics and /info remain behind current auth
 - Risk: DB ping overhead under load
  - Mitigation: cache with TTL; hard timeout; skip when unavailable

## Work Breakdown (Minimal Diffs)
- http.rs
  - Add HttpMetrics fields (active_sessions, total_sessions)
  - Read LocalSessionManager count when serving /metrics, or maintain counters via callbacks if available in rmcp lib
  - In info_handler, compute db_connected (+ optional ping)
- src/bin/smtop.rs
  - Add Session pane and DB pane (conditional fields)
  - Parse new JSON fields; handle absent fields without panic
  - Add log filter state and key handling; add Home/End shortcuts; SMTOP_LOG_TAIL env
- Optional: server writes state.json when SURR_WRITE_STATE=1 (stdio discovery)

## Timeline
- Phase 1: 2–4 hours (including tests and docs)
- Phase 2: 4–6 hours (richer metrics, stdio state, DB details)
- Phase 3: 1–2 hours (cloudflared extras and tool detail pane)

## Acceptance Criteria
- smtop shows Sessions and DB panels without errors when server is running in HTTP mode
- /metrics includes metrics_version="1", http_active_sessions and http_total_sessions
- /metrics and /info are served only with the existing auth in place; unauthenticated requests are rejected as today
- /info includes db_connected (true/false) and, when enabled, db_ping_ms reflecting a TTL-cached ping; default responses meet offline test constraints
- Default cargo test remains offline and green
- When SURR_WRITE_STATE=1, state.json is created 0600, updated atomically, and removed on shutdown
- Default cargo test remains offline and green

## Clarifying Questions

1. For HTTP/SSE Session Visibility: How exactly to wire LocalSessionManager events into HttpMetrics? Is there an existing accessor in the rmcp library to count active sessions, or do we need to add callbacks/events to increment counters?

2. For stdio sessions and state.json: Is the state.json file intended only for stdio transport, or for any transport mode? The proposal mentions "in any transport mode" but focuses on stdio discovery. Should it include session count for HTTP as well, or is it redundant with /metrics?

3. For SurrealDB Status in /info: What specific cheap query should be used for the ping? Something like `SELECT 1` or a no-op? And how to handle if DB is not configured (e.g., no URL set)?

4. For MCP Tool Metrics in /metrics: How to implement the rolling p95_latency_ms with a ring buffer? Should we use a fixed-size Vec and sort for p95, or approximate with percentiles?

5. For Local Resource Usage: On macOS, using `ps -o` should work, but what specific command/format? Also, how to handle multiple PIDs if there are multiple surreal-mind processes?

6. For Logs UX Improvements: The filter [all | stdout | stderr | cloudflared] – how to distinguish stdout/stderr from cloudflared? Currently, logs are from specific files (surreal-mind.out.log, .err.log, cloudflared.out.log, .err.log). So filter by source file?

7. In the TUI Layout, Row 2 has Sessions and SurrealDB. For Sessions, HTTP from /metrics, stdio from state.json. What if both are present? Display both?

8. For Phase 2 /db_health: Should this be a new endpoint, or extend /info? The proposal says "optionally provide /db_health", but /info is already extended.

9. For stdio state.json: The path uses ~/.local/share on Linux, but macOS is ~/Library/Application Support. Is there a cross-platform way, like using dirs crate?

10. For metrics_version in /metrics: Is this to allow forward compatibility, so smtop can handle older servers without the new fields?

## Zed Q&A — Answers (Decisions)

1) Wiring HTTP/SSE session metrics
- Prefer accessor: use the existing session manager to read `active_sessions.len()` at /metrics time.
- If rmcp lacks a public accessor, add tiny hooks where sessions open/close and maintain `Arc<AtomicUsize>` counters for `http_active_sessions` and `http_total_sessions`.
- Fallback is acceptable to compute active on demand by iterating internal map under a read lock; counters give O(1) and are preferred.

2) state.json scope
- Primary purpose is stdio discovery. Keep file write gated and transport-agnostic, but by default only include stdio sessions.
- HTTP session counts remain from /metrics; smtop displays both when present (HTTP from /metrics, stdio from file) with clear labels; no duplication required.

3) DB ping in /info
- Use a trivial, fast query with short timeout: `SELECT 1;` (or `RETURN 1;`) after issuing `USE NS/DB` as usual.
- If DB not configured or client uninitialized: return `db_connected: false` and omit `db_ping_ms`.
- Apply TTL cache as described; never block longer than the configured timeout.

4) Rolling p95 implementation
- Keep a fixed-size ring buffer (e.g., 256 samples). On /metrics, compute p95 with `select_nth_unstable` on a copy (O(n)); n is small and bounded.
- Alternative (if we need even cheaper): fixed histogram buckets (e.g., 0–50/50–100/… ms) and approximate the percentile; ring buffer is fine to start.

5) Local resource usage (macOS)
- Command: `ps -o %cpu=,rss= -p <pid>`; convert RSS (KB) → MB. Uptime via `ps -o lstart= -p <pid>` or compute from `ps -o etime=`.
- If multiple surreal-mind PIDs are found, aggregate (sum RSS, avg CPU over sample) or select the one owning the configured HTTP bind (best-effort).

6) Log source filtering
- Filter by source file: surreal-mind.out.log (stdout), surreal-mind.err.log (stderr), cloudflared*.log (cloudflared). Maintain a simple filter state [all|stdout|stderr|cloudflared].

7) Sessions panel when both present
- Show both: `HTTP Sessions: <active>/<total>` and `Stdio Sessions: <active>` when state.json exists. Hide a line when its source is absent.

8) /db_health vs /info
- Keep /info lightweight (booleans + cached ping). Add `/db_health` for expanded stats, gated by `SURR_DB_STATS=1`. smtop attempts `/db_health` only when the flag is on and endpoint returns 200; otherwise uses /info only.

9) Cross‑platform state path
- Use `directories` (dirs-next) crate to resolve per‑platform data dir: macOS `~/Library/Application Support/surreal-mind/state.json`, Linux `~/.local/share/surreal-mind/state.json`, Windows `%APPDATA%\\surreal-mind\\state.json`.

10) metrics_version purpose
- Yes. Allows smtop to detect the schema version and fall back when fields are missing. If `metrics_version` absent, treat as v0 and only parse the legacy fields.
