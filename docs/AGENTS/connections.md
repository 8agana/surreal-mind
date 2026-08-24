# Connections & Endpoints

- **Transport (default stdio):** `./target/release/surreal-mind`
- **HTTP (streamable/SSE):**
  - Bind: `127.0.0.1:8787` (override `SURR_HTTP_BIND`)
  - Path: `/mcp` (override `SURR_HTTP_PATH`)
  - Auth: bearer token from `~/.surr_token` or `SURR_BEARER_TOKEN`; `SURR_ALLOW_TOKEN_IN_URL=1` to accept `?access_token=`
  - Host allowlist (rmcp 3.1.4+, DNS-rebinding defense): `localhost`/`127.0.0.1`/`::1` always accepted; `SURR_HTTP_ALLOWED_HOSTS` is a comma-separated list of *additional* hosts that extends (never replaces) that loopback set — set but empty, or containing an empty entry, fails startup instead of silently falling back. For the Cloudflare tunnel above to reach `/mcp`, this must include `mcp.samataganaphotography.com` in the deployed launchd/env configuration (not committed to this repo). Applies only to the nested `/mcp` route — not to `/health`, `/info`, `/metrics`, or `/db_health` below.
  - Health: `/health` (no auth), `/info`, `/metrics`, `/db_health` (auth)
- **Cloudflare tunnel:** `legacymind-mcp` → https://mcp.samataganaphotography.com/mcp (token required).
- **Database:** SurrealDB 3.x at `ws://localhost:8000/rpc`, user `root`/`root`; SurrealMind default namespace/database is `surreal_mind` / `consciousness`. Legacy namespaces may still exist after migrations.
- **Ports in use:** 8787 (surreal-mind HTTP), 8000 (SurrealDB), 8080 (lightroom web demo), tunnel process `cloudflared`.
