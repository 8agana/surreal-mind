#!/bin/bash
# P1 binary contract for kg_populate response validation and retirement.
# Against the pre-fix binary, the empty_object case is the preserved red
# witness: it exits with the blanket-retirement reason instead of passing.

set -uo pipefail

REPO_ROOT="${REPO_ROOT:-/Users/samuelatagana/Projects/LegacyMind/surreal-mind-wt-fed70cb9c}"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fed70cb9c-red.XXXXXX")"
SURREAL_BIN="${SURREAL_BIN:-surreal}"
PORT="${KG_POPULATE_CONTRACT_PORT:-}"
TEST_NS="fed70cb9c_red_$$"
TEST_DB="scratch"
CASE="${KG_POPULATE_CONTRACT_CASE:-empty_object}"
SQL_INVOKE_LOG="${KG_POPULATE_SQL_INVOKE_LOG:-}"
FORCE_LISTENER_MISMATCH="${KG_POPULATE_FORCE_LISTENER_MISMATCH:-0}"
SURREAL_PID=""

cleanup() {
  if [ -n "$SURREAL_PID" ] && kill -0 "$SURREAL_PID" 2>/dev/null; then
    kill "$SURREAL_PID" 2>/dev/null || true
    wait "$SURREAL_PID" 2>/dev/null || true
  fi
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

if ! command -v "$SURREAL_BIN" >/dev/null 2>&1; then
  if [ -x /opt/homebrew/bin/surreal ]; then
    SURREAL_BIN=/opt/homebrew/bin/surreal
  else
    echo "surreal CLI not found" >&2
    exit 3
  fi
fi

if [ -n "$PORT" ]; then
  if [ "$PORT" -eq 8000 ]; then
    echo "REFUSING production port 8000" >&2
    exit 3
  fi
  if /usr/sbin/lsof -nP -iTCP:"$PORT" -sTCP:LISTEN 2>/dev/null | grep -q LISTEN; then
    echo "refusing occupied disposable port $PORT" >&2
    exit 3
  fi
else
  for candidate in $(seq 8600 8699); do
    if ! /usr/sbin/lsof -nP -iTCP:"$candidate" -sTCP:LISTEN 2>/dev/null | grep -q LISTEN; then
      PORT="$candidate"
      break
    fi
  done
  if [ -z "$PORT" ]; then
    echo "could not find a free disposable port" >&2
    exit 3
  fi
fi

SQL_ENDPOINT="ws://127.0.0.1:$PORT"
SQL_HTTP="http://127.0.0.1:$PORT"

sql() {
  if [ -n "$SQL_INVOKE_LOG" ]; then
    printf 'sql\n' >>"$SQL_INVOKE_LOG"
  fi
  printf '%s' "$1" | "$SURREAL_BIN" sql \
    --endpoint "http://127.0.0.1:$PORT" --username root --password root \
    --namespace "$TEST_NS" --database "$TEST_DB" --json --hide-welcome
}

"$SURREAL_BIN" start memory --bind "127.0.0.1:$PORT" --user root --pass root \
  >"$WORK_DIR/surreal.log" 2>&1 &
SURREAL_PID=$!

ready=0
for _ in $(seq 1 100); do
  if curl -fsS "$SQL_HTTP/health" >/dev/null 2>&1; then
    ready=1
    break
  fi
  sleep 0.1
done
if [ "$ready" -ne 1 ]; then
  echo "disposable SurrealDB did not become ready" >&2
  head -c 4096 "$WORK_DIR/surreal.log" >&2
  exit 1
fi

ATTRIBUTION_PID="$SURREAL_PID"
if [ "$FORCE_LISTENER_MISMATCH" = 1 ]; then
  ATTRIBUTION_PID=0
fi
if /usr/sbin/lsof -nP -a -p "$ATTRIBUTION_PID" -iTCP:"$PORT" -sTCP:LISTEN 2>/dev/null | grep -q LISTEN; then
  echo "listener_owned_pid=$SURREAL_PID"
else
  echo "listener ownership failed: port=$PORT expected_pid=$SURREAL_PID" >&2
  exit 1
fi

sql "DEFINE TABLE thoughts SCHEMALESS; DEFINE TABLE kg_entities SCHEMALESS; CREATE thoughts:pending_a SET content = 'pending A', created_at = time::now(), extracted_to_kg = false; CREATE thoughts:pending_b SET content = 'pending B', created_at = time::now(), extracted_to_kg = false;" \
  >"$WORK_DIR/seed.json"

FAKE_AGY="$WORK_DIR/fake-agy"
cat >"$FAKE_AGY" <<'FAKE'
#!/bin/bash
case "${KG_POPULATE_CONTRACT_CASE:-empty_object}" in
  empty_object) printf '{}\n' ;;
  missing_extractions) printf '{"summary":"missing"}\n' ;;
  wrong_typed_extractions) printf '{"extractions":{}}\n' ;;
  empty_extractions) printf '{"extractions":[],"summary":"empty"}\n' ;;
  partial_omission) printf '%s\n' '{"extractions":[{"thought_id":"pending_a"}]}' ;;
  duplicate_id) printf '%s\n' '{"extractions":[{"thought_id":"pending_a"},{"thought_id":"pending_a"}]}' ;;
  unknown_id) printf '%s\n' '{"extractions":[{"thought_id":"pending_a"},{"thought_id":"unknown"}]}' ;;
  complete) printf '%s\n' '{"extractions":[{"thought_id":"pending_a"},{"thought_id":"pending_b"}],"summary":"complete"}' ;;
  no_entity) printf '%s\n' '{"extractions":[{"thought_id":"pending_a","entities":[],"relationships":[],"observations":[],"boundaries":[]},{"thought_id":"pending_b","entities":[],"relationships":[],"observations":[],"boundaries":[]}],"summary":"no entity"}' ;;
  entity) printf '%s\n' '{"extractions":[{"thought_id":"pending_a","entities":[{"name":"Red Control Entity","type":"concept"}]},{"thought_id":"pending_b"}],"summary":"entity"}' ;;
  *) echo "unknown KG_POPULATE_CONTRACT_CASE=$KG_POPULATE_CONTRACT_CASE" >&2; exit 64 ;;
esac
FAKE
chmod +x "$FAKE_AGY"

set +e
(
  cd "$WORK_DIR" || exit 1
  SURR_DB_URL="127.0.0.1:$PORT" \
  SURR_DB_NS="$TEST_NS" \
  SURR_DB_DB="$TEST_DB" \
  SURR_DB_USER=root \
  SURR_DB_PASS=root \
  SM_AGENT_PROVIDER=antigravity \
  ANTIGRAVITY_CLI_BIN="$FAKE_AGY" \
  ANTIGRAVITY_TIMEOUT_MS=5000 \
  ANTIGRAVITY_PRINT_TIMEOUT_MS=5000 \
  KG_POPULATE_BATCH_SIZE=2 \
  KG_POPULATE_MAX_BATCHES=1 \
  cargo run --quiet --manifest-path "$REPO_ROOT/Cargo.toml" --bin kg_populate
) >"$WORK_DIR/populate.stdout" 2>"$WORK_DIR/populate.stderr"
populate_rc=$?
set -e

thoughts_json=$(sql "SELECT meta::id(id) AS id, extracted_to_kg FROM thoughts ORDER BY id;" \
  | jq -c '.[0] | sort_by(.id)')
pending_count=$(sql "SELECT count() AS count FROM thoughts WHERE extracted_to_kg = false OR extracted_to_kg IS NONE GROUP ALL;" \
  | jq -r '.[0][0].count // 0')
entity_count=$(sql "SELECT count() AS count FROM kg_entities GROUP ALL;" \
  | jq -r '.[0][0].count // 0')
entities_json=$(sql "SELECT meta::id(id) AS id, name, source_thought_ids FROM kg_entities ORDER BY name;" \
  | jq -c '.[0] | sort_by(.name)')
entity_source=$(printf '%s' "$entities_json" | jq -r '.[0].source_thought_ids[0] // ""')

echo "case=$CASE"
echo "populate_rc=$populate_rc"
echo "thoughts=$thoughts_json"
echo "pending_count=$pending_count"
echo "entity_count=$entity_count"
echo "entities=$entities_json"
echo "entity_source=$entity_source"

if [ "$populate_rc" -ne 0 ]; then
  echo "red control could not exercise the binary" >&2
  cat "$WORK_DIR/populate.stderr" >&2
  exit 1
fi
case "$CASE" in
  empty_object|missing_extractions|wrong_typed_extractions|empty_extractions|partial_omission|duplicate_id|unknown_id)
    expected_pending=2
    expected_entities=0
    expected_source=""
    ;;
  complete|no_entity)
    expected_pending=0
    expected_entities=0
    expected_source=""
    ;;
  entity)
    expected_pending=0
    expected_entities=1
    expected_source=pending_a
    ;;
  *)
    echo "unknown expected case $CASE" >&2
    exit 1
    ;;
esac

if [ "$pending_count" -ne "$expected_pending" ] || [ "$entity_count" -ne "$expected_entities" ] || [ "$entity_source" != "$expected_source" ]; then
  echo "CASE FAILED: case=$CASE expected_pending=$expected_pending expected_entities=$expected_entities expected_source=$expected_source" >&2
  exit 1
fi

echo "CASE PASSED: case=$CASE"
