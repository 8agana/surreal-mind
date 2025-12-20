#!/usr/bin/env bash
set -euo pipefail

BASE="${SURR_HTTP_BASE:-http://127.0.0.1:8787}"
TOKEN="${SURR_BEARER_TOKEN:-${SURR_TOKEN:-}}"
AUTH=()
if [[ -n "${TOKEN}" ]]; then
  AUTH+=(-H "Authorization: Bearer ${TOKEN}")
fi

echo ">> Health (${BASE}/health)"
curl -fsSL "${AUTH[@]}" "${BASE}/health" || { echo "health check failed"; exit 1; }

if curl -fsSL "${AUTH[@]}" "${BASE}/db_health" >/dev/null 2>&1; then
  echo ">> DB health ok (${BASE}/db_health)"
else
  echo ">> DB health endpoint not available or unauthorized"
fi

if curl -fsSL "${AUTH[@]}" "${BASE}/mcp" >/dev/null 2>&1; then
  echo ">> Tools endpoint ok (${BASE}/mcp)"
else
  echo ">> Tools endpoint protected or unavailable"
fi
