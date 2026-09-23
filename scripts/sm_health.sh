#!/bin/bash
# Minimal health/decay check for REMini.
# Marks stale high-volatility entities for research (gemini).

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
SURREAL_BIN=${SURREAL_BIN:-surreal}

if ! [[ "$LIMIT" =~ ^[0-9]+$ && "$HALF_LIFE_DAYS" =~ ^[0-9]+$ ]]; then
  echo "sm_health: STALENESS_LIMIT and VOL_HIGH_HALF_LIFE_DAYS must be non-negative integers" >&2
  exit 2
fi

SQL="LET \$stale = (SELECT id FROM kg_entities WHERE volatility = 'high' AND last_refreshed != NONE AND time::now() - last_refreshed > duration::from_days(${HALF_LIFE_DAYS}) AND marked_for = NONE LIMIT ${LIMIT}); UPDATE \$stale SET marked_for = 'gemini', mark_type = 'research', mark_note = 'Auto-flagged: high volatility, stale', marked_at = time::now(), marked_by = 'health' RETURN NONE;"

if [ -n "$ENDPOINT" ] && [[ "$ENDPOINT" != http://* && "$ENDPOINT" != https://* ]]; then
  ENDPOINT="http://$ENDPOINT"
fi

ARGS=(
  sql
  --username "$USER"
  --password "$PASS"
  --namespace "$NS"
  --database "$DB"
  --json
  --hide-welcome
)
if [ -n "$ENDPOINT" ]; then
  ARGS+=(--endpoint "$ENDPOINT")
fi

WORK_DIR=$(mktemp -d "${TMPDIR:-/tmp}/sm_health.XXXXXX")
cleanup() {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

STDOUT_FILE="$WORK_DIR/surreal.stdout"
STDERR_FILE="$WORK_DIR/surreal.stderr"

if printf '%s' "$SQL" | "$SURREAL_BIN" "${ARGS[@]}" >"$STDOUT_FILE" 2>"$STDERR_FILE"; then
  SURREAL_RC=0
else
  SURREAL_RC=$?
fi

# `surreal sql --json --hide-welcome` emits one JSON value followed by a line
# terminator. Command substitution strips trailing LF; deleting CR makes CRLF
# and LF equivalent without accepting any other byte difference.
NORMALIZED_OUTPUT=$(tr -d '\r' <"$STDOUT_FILE")
EXPECTED_OUTPUT='[null,[]]'

emit_bounded_file() {
  local label=$1
  local file=$2
  local bytes
  bytes=$(wc -c <"$file" | tr -d ' ')
  [ "$bytes" -eq 0 ] && return 0

  echo "sm_health: surreal ${label} (${bytes} bytes; first 2048 bytes):" >&2
  head -c 2048 "$file" >&2
  if [ "$bytes" -gt 2048 ]; then
    echo >&2
    echo "sm_health: ${label} truncated" >&2
  elif [ "$(tail -c 1 "$file" | wc -l | tr -d ' ')" -eq 0 ]; then
    echo >&2
  fi
}

if [ "$SURREAL_RC" -ne 0 ] || [ "$NORMALIZED_OUTPUT" != "$EXPECTED_OUTPUT" ] || [ -s "$STDERR_FILE" ]; then
  echo "sm_health: SurrealDB health query failed contract (exit=${SURREAL_RC}, expected=${EXPECTED_OUTPUT})" >&2
  emit_bounded_file stdout "$STDOUT_FILE"
  emit_bounded_file stderr "$STDERR_FILE"
  exit 1
fi

printf '%s\n' "$NORMALIZED_OUTPUT"
