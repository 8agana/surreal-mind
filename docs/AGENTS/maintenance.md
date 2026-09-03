# Maintenance & Ops

- **Restart (launchd):** `launchctl kickstart -k gui/$(id -u)/dev.legacymind.surreal-mind`
- **Build+restart cycle:** `cargo build --release && launchctl kickstart -k gui/$(id -u)/dev.legacymind.surreal-mind`
- **Binary replacement does not refresh stdio clients:** launchd restarts only the HTTP production process. A long-lived ChatGPT/Codex app child that spawned `surreal-mind` over stdio keeps its original executable inode after an atomic binary rename and may continue serving older code against the same database. After every binary deploy, compare each `surreal-mind` PID's `lsof -a -p <pid> -d txt` inode with the on-disk binary. Exercise a freshly spawned stdio client against the new image; restart a host app or its MCP connection only as a separate, attended action when eliminating live version skew is required.
- **Health checks:** curl `http://127.0.0.1:8787/health`; for DB `http://127.0.0.1:8787/db_health` (auth). Port check: `lsof -nPi tcp:8787`.
- **Verify tool surface:** `/mcp` is authenticated MCP transport, not a human-readable roster. A plain request returns `401`; use a connected MCP client (`howto` with no `tool`) or perform an authenticated initialize/initialized/tools-list sequence with the returned `mcp-session-id`.
- **One-shot smoke test:** `scripts/sm_health.sh` (uses `SURR_BEARER_TOKEN`/`SURR_TOKEN` if present) or `cargo run --bin admin -- simple-test`.
- **Debug Tools:** `kg_debug_tool` (KG introspection) and `kg_embed` (ad hoc embedding). There is no `simple_db_test` or `test_gemini` binary. `tests/test_gemini_call.rs` is legacy shell-out scaffolding with zero `#[test]` cases.
- **Google CLI provider:** `SM_AGENT_PROVIDER=antigravity|gemini` (or `GOOGLE_CLI_PROVIDER` / `SURR_GOOGLE_CLI_PROVIDER`) selects the provider for `kg_populate` and `kg_wander`. Default is `antigravity`; set `gemini` for rollback. Antigravity unattended paths use sandbox/default permissions.
- **Logs:** stdout `~/Library/Logs/surreal-mind.out.log`; stderr `~/Library/Logs/surreal-mind.err.log`.
- **Cloudflared tunnel:** service `com.legacymind.cloudflared-tunnel`; restart with `launchctl kickstart -k gui/$(id -u)/com.legacymind.cloudflared-tunnel`.
- **SurrealDB service:** `com.legacymind.surrealdb` (bind 127.0.0.1:8000).
- **Tool timeouts:** `SURR_HTTP_REQUEST_TIMEOUT_MS`, `SURR_HTTP_MCP_OP_TIMEOUT_MS`, `SURR_TOOL_TIMEOUT_MS` (default 15s).
