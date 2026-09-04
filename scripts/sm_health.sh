#!/bin/bash
# Minimal health/decay check for REMini
# Marks stale high-volatility entities for research (gemini)

set -euo pipefail

# Under DRY_RUN, exit before any SQL is issued. Truthiness matches the
# convention used across the other maintenance binaries (bool_env in
# kg_embed.rs/kg_wander.rs/reembed_kg.rs, and the `== "1" ||
# eq_ignore_ascii_case("true")` checks in kg_populate.rs/gem_rethink.rs/
# kg_consolidate.rs): "1", "true"/"TRUE"/"True", "yes", "on".
case "${DRY_RUN:-}" in
  1|true|TRUE|True|yes|YES|on|ON)
    echo "[DRY_RUN] sm_health: skipping stale-entity UPDATE, no SQL issued"
    exit 0
    ;;
esac

NS=${SURR_DB_NS:-surreal_mind}
DB=${SURR_DB_DB:-consciousness}
USER=${SURR_DB_USER:-root}
PASS=${SURR_DB_PASS:-root}
ENDPOINT=${SURR_DB_URL:-}
LIMIT=${STALENESS_LIMIT:-100}
HALF_LIFE_DAYS=${VOL_HIGH_HALF_LIFE_DAYS:-90}

SQL="LET \$stale = (SELECT id FROM kg_entities WHERE volatility = 'high' AND last_refreshed != NONE AND time::now() - last_refreshed > duration::days(${HALF_LIFE_DAYS}) AND marked_for = NONE LIMIT ${LIMIT}); UPDATE \$stale SET marked_for = 'gemini', mark_type = 'research', mark_note = 'Auto-flagged: high volatility, stale', marked_at = time::now(), marked_by = 'health' RETURN NONE;"

if [ -n "$ENDPOINT" ]; then
  # Normalize endpoint to include scheme if missing
  if [[ "$ENDPOINT" != http://* && "$ENDPOINT" != https://* ]]; then
    ENDPOINT="http://$ENDPOINT"
  fi
  printf "%s" "$SQL" | surreal sql --endpoint "$ENDPOINT" --username "$USER" --password "$PASS" --namespace "$NS" --database "$DB" --pretty
else
  printf "%s" "$SQL" | surreal sql --username "$USER" --password "$PASS" --namespace "$NS" --database "$DB" --pretty
fi
