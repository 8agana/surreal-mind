# Connections & Endpoints

- **Transport (default stdio):** `./target/release/surreal-mind`
- **HTTP (streamable/SSE):**
  - Bind: `127.0.0.1:8787` (override `SURR_HTTP_BIND`)
  - Path: `/mcp` (override `SURR_HTTP_PATH`)
  - Auth: bearer token from `~/.surr_token` or `SURR_BEARER_TOKEN`; `SURR_ALLOW_TOKEN_IN_URL=1` to accept `?access_token=`
  - Health: `/health` (no auth), `/info`, `/metrics`, `/db_health` (auth)
- **Cloudflare tunnel:** `legacymind-mcp` → https://mcp.samataganaphotography.com/mcp (token required).
- **Database:** SurrealDB `ws://localhost:8000/rpc`, user `root`/`root`; namespaces `legacymind` (dev) and `photography` (for photo MCP; kept separate).
- **Ports in use:** 8787 (surreal-mind HTTP), 8000 (SurrealDB), 8080 (lightroom web demo), tunnel process `cloudflared`.
