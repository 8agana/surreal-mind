#!/bin/bash
# Minimal health/decay check for REMini
# Marks stale high-volatility entities for research (gemini)

set -euo pipefail

NS=${SURR_DB_NS:-surreal_mind}
DB=${SURR_DB_DB:-consciousness}
USER=${SURR_DB_USER:-root}
PASS=${SURR_DB_PASS:-root}
ENDPOINT=${SURR_DB_URL:-}
LIMIT=${STALENESS_LIMIT:-100}
HALF_LIFE_DAYS=${VOL_HIGH_HALF_LIFE_DAYS:-90}

SQL="LET \$stale = (SELECT id FROM kg_entities WHERE volatility = 'high' AND last_refreshed != NONE AND time::now() - last_refreshed > duration::days(${HALF_LIFE_DAYS}) AND marked_for = NONE LIMIT ${LIMIT}); UPDATE \$stale SET marked_for = 'gemini', mark_type = 'research', mark_note = 'Auto-flagged: high volatility, stale', marked_at = time::now(), marked_by = 'health' RETURN NONE;"

if [ -n "$ENDPOINT" ]; then
  printf "%s" "$SQL" | surreal sql --endpoint "$ENDPOINT" --username "$USER" --password "$PASS" --namespace "$NS" --database "$DB" --pretty
else
  printf "%s" "$SQL" | surreal sql --username "$USER" --password "$PASS" --namespace "$NS" --database "$DB" --pretty
fi
