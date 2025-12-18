# Maintenance & Ops

- **Restart (launchd):** `launchctl kickstart -k gui/$(id -u)/dev.legacymind.surreal-mind`
- **Build+restart cycle:** `cargo build --release && launchctl kickstart -k gui/$(id -u)/dev.legacymind.surreal-mind`
- **Health checks:** curl `http://127.0.0.1:8787/health`; for DB `http://127.0.0.1:8787/db_health` (auth). Port check: `lsof -nPi tcp:8787`.
- **Verify tool surface:** `curl http://127.0.0.1:8787/mcp` to see exposed tools.
- **Logs:** stdout `~/Library/Logs/surreal-mind.out.log`; stderr `~/Library/Logs/surreal-mind.err.log`.
- **Cloudflared tunnel:** service `com.legacymind.cloudflared-tunnel`; restart with `launchctl kickstart -k gui/$(id -u)/com.legacymind.cloudflared-tunnel`.
- **SurrealDB service:** `com.legacymind.surrealdb` (bind 127.0.0.1:8000).
- **Tool timeouts:** `SURR_HTTP_REQUEST_TIMEOUT_MS`, `SURR_HTTP_MCP_OP_TIMEOUT_MS`, `SURR_TOOL_TIMEOUT_MS` (default 15s).
