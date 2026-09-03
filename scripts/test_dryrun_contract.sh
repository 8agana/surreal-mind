#!/bin/bash
# scripts/test_dryrun_contract.sh
#
# fed-734b8f step 0.5: the REMini DRY_RUN contract test.
#
# Contract (Codex #128 S1): under --dry-run, remini's six tasks must produce
#   - zero provider invocations (no real call to any AI CLI)
#   - zero data/schema mutations
#   - bounded termination
#   - only the explicitly selected report artifact written
#
# This script proves that contract two ways:
#   POSITIVE control: run `remini --all --dry-run` against a seeded throwaway
#     SurrealDB with a fake provider stub wired in. Assert 0 provider calls,
#     identical DB snapshot before/after, bounded exit, and that only the
#     requested report path was written.
#   NEGATIVE control: reseed, then run the same binary WITHOUT --dry-run
#     (excluding the "embed" task -- see NOTE below) and assert the provider
#     WAS called and the DB DID change. Without this half, a positive result
#     proves nothing -- a test that can't fail isn't a test.
#
# NEVER calls a real AI provider: the "embed" task is deliberately excluded
# from the negative (live) control. kg_embed's OpenAI embedding call fires
# synchronously with no fake-stub layer this repo owns (unlike the
# Antigravity CLI, which IS stubbed via ANTIGRAVITY_CLI_BIN below) -- running
# it live would hit a real third-party endpoint, which the operating
# constraints for this test explicitly forbid. embed's own dry-run path is
# still exercised and asserted in the positive control.
#
# Always runs against a throwaway in-memory SurrealDB instance this script
# starts itself and tears down on exit. Never touches production.
#
# Usage: scripts/test_dryrun_contract.sh
# Exit code: 0 if every assertion passes, 1 otherwise.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_DIR="$REPO_ROOT/scripts/dryrun_contract"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/dryrun_contract.XXXXXX")"

TEST_PORT="${DRYRUN_CONTRACT_PORT:-8100}"
TEST_NS="test_fed734b8f_$$"
TEST_DB="dry"
SURREAL_BIN="${SURREAL_BIN:-surreal}"
if ! command -v "$SURREAL_BIN" >/dev/null 2>&1; then
  if [ -x /opt/homebrew/bin/surreal ]; then
    SURREAL_BIN=/opt/homebrew/bin/surreal
  fi
fi

FAKE_AGY="$WORK_DIR/fake-agy"
CALLS_FILE="$WORK_DIR/fake-agy.calls"
SURREAL_PID=""
FAIL=0

pass() { echo "  PASS: $1"; }
fail() { echo "  FAIL: $1"; FAIL=1; }

cleanup() {
  if [ -n "$SURREAL_PID" ] && kill -0 "$SURREAL_PID" 2>/dev/null; then
    kill "$SURREAL_PID" 2>/dev/null
    wait "$SURREAL_PID" 2>/dev/null
  fi
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

sql() {
  # `surreal sql` writes a readline history file (history.txt) into the
  # CWD it's invoked from with no flag to redirect it -- run it from the
  # scratch work dir so it never clobbers the repo's own history.txt (a
  # tracked file this script silently overwrote once during development).
  (cd "$WORK_DIR" && "$SURREAL_BIN" sql --endpoint "http://127.0.0.1:$TEST_PORT" \
    --username root --password root \
    --namespace "$TEST_NS" --database "$TEST_DB" --pretty)
}

apply_sql_file() {
  sql < "$1" > "$WORK_DIR/last_sql_apply.log" 2>&1
}

snapshot() {
  # snapshot > outfile
  SNAPSHOT_DB_PORT="$TEST_PORT" SNAPSHOT_DB_NS="$TEST_NS" SNAPSHOT_DB_DB="$TEST_DB" \
    python3 "$FIXTURE_DIR/snapshot_db.py"
}

digest_of() {
  python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['digest'])" "$1"
}

reseed() {
  echo "REMOVE DATABASE $TEST_DB;" | sql > /dev/null 2>&1
  apply_sql_file "$FIXTURE_DIR/schema.surql"
  apply_sql_file "$FIXTURE_DIR/seed.surql"
}

# Run a binary in the background with a hard wall-clock bound (no `timeout`
# binary on this macOS box). Writes stdout+stderr to $1, sets $BOUND_RC and
# $BOUND_ELAPSED. Exceeding the bound kills the process group and reports it.
run_bounded() {
  local outfile="$1" bound="$2"
  shift 2
  local start end i pid
  start=$(date +%s)
  "$@" > "$outfile" 2>&1 &
  pid=$!
  i=0
  while kill -0 "$pid" 2>/dev/null; do
    sleep 1
    i=$((i + 1))
    if [ "$i" -ge "$bound" ]; then
      kill -9 "$pid" 2>/dev/null
      break
    fi
  done
  wait "$pid"
  BOUND_RC=$?
  end=$(date +%s)
  BOUND_ELAPSED=$((end - start))
  BOUND_TIMED_OUT=0
  if [ "$i" -ge "$bound" ]; then
    BOUND_TIMED_OUT=1
  fi
}

echo "== fed-734b8f dry-run contract test =="
echo "worktree: $REPO_ROOT"
echo "namespace: $TEST_NS  db: $TEST_DB  port: $TEST_PORT"

REMINI="$REPO_ROOT/target/release/remini"
if [ ! -x "$REMINI" ]; then
  echo "Building release binaries (remini + task binaries)..."
  (cd "$REPO_ROOT" && cargo build --release \
    --bin remini --bin kg_populate --bin kg_wander --bin kg_embed \
    --bin gem_rethink --bin kg_consolidate) || {
    echo "build failed"; exit 1;
  }
fi

echo "Starting throwaway SurrealDB (memory, 127.0.0.1:$TEST_PORT)..."
"$SURREAL_BIN" start memory --bind "127.0.0.1:$TEST_PORT" --user root --pass root \
  > "$WORK_DIR/surreal.log" 2>&1 &
SURREAL_PID=$!
for _ in $(seq 1 20); do
  if curl -s -o /dev/null "http://127.0.0.1:$TEST_PORT/health"; then
    break
  fi
  sleep 0.5
done
if ! curl -s -o /dev/null "http://127.0.0.1:$TEST_PORT/health"; then
  echo "throwaway SurrealDB did not come up; see $WORK_DIR/surreal.log"
  exit 1
fi

cat > "$FAKE_AGY" << EOF
#!/bin/bash
# Fake Antigravity CLI stub. Never calls a real provider.
echo "\$(date -u +%Y-%m-%dT%H:%M:%SZ) \$*" >> "$CALLS_FILE"
cat <<'JSON'
{"extractions":[],"summary":"fake-agy canned response: fed-734b8f dry-run contract test, no real provider called"}
JSON
exit 0
EOF
chmod +x "$FAKE_AGY"

export SURR_DB_URL="127.0.0.1:$TEST_PORT"
export SURR_DB_NS="$TEST_NS"
export SURR_DB_DB="$TEST_DB"
export SURR_DB_USER=root
export SURR_DB_PASS=root
export ANTIGRAVITY_CLI_BIN="$FAKE_AGY"

echo
echo "-- seeding --"
reseed
echo "seeded: $(snapshot | python3 -c 'import json,sys; print(json.load(sys.stdin)["counts"])')"

echo
echo "== POSITIVE control: remini --all --dry-run =="
export OPENAI_API_KEY="sk-fake-fed734b8f-not-a-real-key-000000000000"
snapshot > "$WORK_DIR/snap_pos_before.json"
: > "$CALLS_FILE"
POS_REPORT="$WORK_DIR/remini_dry_positive.json"
run_bounded "$WORK_DIR/remini_pos.stdout" 300 \
  "$REMINI" --all --dry-run --timeout 120 --report-path "$POS_REPORT"
snapshot > "$WORK_DIR/snap_pos_after.json"

echo "remini exit=$BOUND_RC elapsed=${BOUND_ELAPSED}s timed_out=$BOUND_TIMED_OUT"
[ "$BOUND_TIMED_OUT" -eq 0 ] && pass "bounded termination (${BOUND_ELAPSED}s < 300s)" \
  || fail "exceeded 300s wall-clock bound"
[ "$BOUND_RC" -eq 0 ] && pass "process exit 0" || fail "process exit $BOUND_RC"

CALLS=$(wc -l < "$CALLS_FILE" | tr -d ' ')
[ "$CALLS" -eq 0 ] && pass "zero provider invocations (fake-agy.calls empty)" \
  || fail "fake-agy.calls has $CALLS line(s) under --dry-run"

DIGEST_BEFORE=$(digest_of "$WORK_DIR/snap_pos_before.json")
DIGEST_AFTER=$(digest_of "$WORK_DIR/snap_pos_after.json")
[ "$DIGEST_BEFORE" = "$DIGEST_AFTER" ] && pass "zero DB mutation (snapshot digest unchanged)" \
  || fail "DB snapshot changed under --dry-run (before=$DIGEST_BEFORE after=$DIGEST_AFTER)"

[ -f "$POS_REPORT" ] && pass "requested report artifact was written ($POS_REPORT)" \
  || fail "report artifact missing at $POS_REPORT"
if [ -f "$REPO_ROOT/logs/remini_report.json" ]; then
  fail "default logs/remini_report.json was written even though --report-path was given"
else
  pass "default logs/remini_report.json was NOT written (only the selected path)"
fi

TASK_COUNT=$(jq '.task_details | length' "$POS_REPORT" 2>/dev/null || echo -1)
[ "$TASK_COUNT" -eq 6 ] && pass "report has six task_details entries" \
  || fail "report has $TASK_COUNT task_details entries, expected 6"

echo
echo "== NEGATIVE control: remini (no --dry-run), tasks=populate,rethink,consolidate =="
echo "   (embed excluded: its OpenAI call is unstubbed, wander/health excluded: unrelated live preconditions)"
reseed
unset OPENAI_API_KEY
snapshot > "$WORK_DIR/snap_neg_before.json"
: > "$CALLS_FILE"
NEG_REPORT="$WORK_DIR/remini_live_negative.json"
run_bounded "$WORK_DIR/remini_neg.stdout" 300 \
  "$REMINI" --tasks populate,rethink,consolidate --timeout 120 --report-path "$NEG_REPORT"
snapshot > "$WORK_DIR/snap_neg_after.json"

echo "remini exit=$BOUND_RC elapsed=${BOUND_ELAPSED}s"

NEG_CALLS=$(grep -c -- "--print-timeout" "$CALLS_FILE" 2>/dev/null || echo 0)
[ "$NEG_CALLS" -gt 0 ] && pass "provider WAS invoked without --dry-run ($NEG_CALLS call(s))" \
  || fail "fake-agy was never invoked in the live run -- negative control did not fire"

NEG_DIGEST_BEFORE=$(digest_of "$WORK_DIR/snap_neg_before.json")
NEG_DIGEST_AFTER=$(digest_of "$WORK_DIR/snap_neg_after.json")
[ "$NEG_DIGEST_BEFORE" != "$NEG_DIGEST_AFTER" ] && pass "DB WAS mutated without --dry-run" \
  || fail "DB snapshot unchanged in the live run -- negative control did not fire"

if [ "$NEG_CALLS" -eq 0 ] && [ "$NEG_DIGEST_BEFORE" = "$NEG_DIGEST_AFTER" ]; then
  fail "NEGATIVE CONTROL DID NOT FIRE AT ALL -- the positive-control zero above is untested, not confirmed"
fi

echo
if [ "$FAIL" -eq 0 ]; then
  echo "== ALL ASSERTIONS PASSED =="
  exit 0
else
  echo "== ONE OR MORE ASSERTIONS FAILED (see FAIL lines above) =="
  exit 1
fi
