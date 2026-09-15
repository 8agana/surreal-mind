#!/bin/bash
# scripts/dryrun_contract/test_health_dryrun.sh
#
# Discriminating contract for scripts/sm_health.sh and REMini's health-task
# reporting. The harness owns a fresh in-memory SurrealDB process, uses a
# unique namespace/database, refuses production port 8000, runs every CLI
# from a disposable CWD, and proves the canonical worktree is unchanged.
#
# Coverage:
#   - DRY_RUN issues no SQL and leaves a seeded stale row unchanged.
#   - A real stale row is marked and independently read back.
#   - An empty eligible set still returns the exact successful query shape.
#   - Surreal CLI malformed JSON, query-error JSON with exit 0, and a nonzero
#     CLI exit all make sm_health fail with bounded diagnostics.
#   - REMini reports health success/failure from the script's exit status.
#
# Usage: scripts/dryrun_contract/test_health_dryrun.sh
# Exit code: 0 if every assertion passes, 1 otherwise.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/health_contract.XXXXXX")"
REAL_SURREAL_BIN="${SURREAL_BIN:-surreal}"
if ! command -v "$REAL_SURREAL_BIN" >/dev/null 2>&1; then
  if [ -x /opt/homebrew/bin/surreal ]; then
    REAL_SURREAL_BIN=/opt/homebrew/bin/surreal
  else
    echo "surreal CLI not found" >&2
    exit 1
  fi
fi

CANONICAL_REPO="${CANONICAL_REPO:-$(git -C "$REPO_ROOT" worktree list --porcelain | awk '/^worktree / { print substr($0, 10); exit }')}"
REMINI_BIN="${REMINI_BIN:-$REPO_ROOT/target/release/remini}"
if [ ! -x "$REMINI_BIN" ] && [ -x "$CANONICAL_REPO/target/release/remini" ]; then
  REMINI_BIN="$CANONICAL_REPO/target/release/remini"
fi

TEST_NS="test_fed4b1bab_health_$$"
TEST_DB=contract
SURREAL_PID=""
FAIL=0

pass() { echo "  PASS: $1"; }
fail() { echo "  FAIL: $1"; FAIL=1; }

port_is_listening() {
  /usr/sbin/lsof -nP -iTCP:"$1" -sTCP:LISTEN 2>/dev/null | grep -q LISTEN
}

port_is_safe() {
  local port=$1
  [[ "$port" =~ ^[0-9]+$ ]] || return 1
  [ "$port" -ge 1024 ] && [ "$port" -le 65535 ] || return 1
  [ "$port" -ne 8000 ] || return 1
  ! port_is_listening "$port"
}

choose_port() {
  local candidate
  local attempt=0
  while [ "$attempt" -lt 100 ]; do
    candidate=$((20000 + (RANDOM % 30000)))
    if port_is_safe "$candidate"; then
      echo "$candidate"
      return 0
    fi
    attempt=$((attempt + 1))
  done
  return 1
}

stop_surreal() {
  local pid="${SURREAL_PID:-}"
  local attempt=0
  [ -n "$pid" ] || return 0

  if kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    while kill -0 "$pid" 2>/dev/null && [ "$attempt" -lt 50 ]; do
      sleep 0.1
      attempt=$((attempt + 1))
    done
    if kill -0 "$pid" 2>/dev/null; then
      kill -KILL "$pid" 2>/dev/null || true
    fi
    wait "$pid" 2>/dev/null || true
  fi
  SURREAL_PID=""
}

cleanup() {
  stop_surreal
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT INT TERM

snapshot_repo() {
  local repo=$1
  {
    git -C "$repo" branch --show-current
    git -C "$repo" rev-parse HEAD
    git -C "$repo" status --porcelain=v1 --untracked-files=all
    git -C "$repo" diff --binary HEAD
  } | shasum -a 256 | awk '{print $1}'
}

CANONICAL_SNAPSHOT_BEFORE=$(snapshot_repo "$CANONICAL_REPO")
TEST_WORKTREE_SNAPSHOT_BEFORE=$(snapshot_repo "$REPO_ROOT")
CANONICAL_HEAD_BEFORE=$(git -C "$CANONICAL_REPO" rev-parse HEAD)
CANONICAL_BRANCH_BEFORE=$(git -C "$CANONICAL_REPO" branch --show-current)

if port_is_safe 8000; then
  fail "port guard accepted production port 8000"
else
  pass "port guard refuses production port 8000"
fi

if [ -n "${DRYRUN_CONTRACT_PORT:-}" ]; then
  TEST_PORT=$DRYRUN_CONTRACT_PORT
  if ! port_is_safe "$TEST_PORT"; then
    echo "refusing unsafe or occupied DRYRUN_CONTRACT_PORT=$TEST_PORT" >&2
    exit 1
  fi
else
  TEST_PORT=$(choose_port) || {
    echo "could not find a free high port" >&2
    exit 1
  }
fi

SQL_ENDPOINT="http://127.0.0.1:$TEST_PORT"
SURREAL_LOG="$WORK_DIR/surreal.log"

echo "== fed-4b1bab sm_health/REMini contract =="
echo "worktree: $REPO_ROOT"
echo "canonical: $CANONICAL_REPO ($CANONICAL_BRANCH_BEFORE $CANONICAL_HEAD_BEFORE)"
echo "namespace: $TEST_NS  db: $TEST_DB  port: $TEST_PORT"

"$REAL_SURREAL_BIN" start memory --bind "127.0.0.1:$TEST_PORT" --user root --pass root \
  >"$SURREAL_LOG" 2>&1 &
SURREAL_PID=$!
OWNED_PID=$SURREAL_PID

READY=0
for _ in $(seq 1 100); do
  if curl -fsS "$SQL_ENDPOINT/health" >/dev/null 2>&1; then
    READY=1
    break
  fi
  if ! kill -0 "$SURREAL_PID" 2>/dev/null; then
    echo "owned SurrealDB exited before readiness:" >&2
    head -c 4096 "$SURREAL_LOG" >&2
    exit 1
  fi
  sleep 0.1
done
if [ "$READY" -ne 1 ]; then
  echo "owned SurrealDB did not become ready within 10 seconds:" >&2
  head -c 4096 "$SURREAL_LOG" >&2
  exit 1
fi

if /usr/sbin/lsof -nP -a -p "$SURREAL_PID" -iTCP:"$TEST_PORT" -sTCP:LISTEN 2>/dev/null | grep -q LISTEN; then
  pass "fresh SurrealDB listener is owned by PID $SURREAL_PID"
else
  fail "listener on $TEST_PORT is not attributable to owned PID $SURREAL_PID"
fi
if port_is_safe "$TEST_PORT"; then
  fail "occupied-port guard accepted the live test listener"
else
  pass "occupied-port guard rejects an existing listener"
fi

real_sql() {
  (cd "$WORK_DIR" && printf '%s' "$1" | "$REAL_SURREAL_BIN" sql \
    --endpoint "$SQL_ENDPOINT" \
    --username root --password root \
    --namespace "$TEST_NS" --database "$TEST_DB" \
    --json --hide-welcome)
}

expect_real_sql() {
  local label=$1
  local query=$2
  local expected=$3
  local output
  local rc

  if output=$(real_sql "$query" 2>"$WORK_DIR/real_sql.stderr"); then
    rc=0
  else
    rc=$?
  fi
  output=$(printf '%s' "$output" | tr -d '\r')
  if [ "$rc" -eq 0 ] && [ ! -s "$WORK_DIR/real_sql.stderr" ] && [ "$output" = "$expected" ]; then
    pass "$label"
  else
    fail "$label (exit=$rc expected=$expected got=$output stderr=$(head -c 512 "$WORK_DIR/real_sql.stderr"))"
  fi
}

expect_real_sql "define isolated kg_entities table" \
  "DEFINE TABLE kg_entities SCHEMALESS;" '[null]'
expect_real_sql "seed one stale high-volatility row" \
  "CREATE kg_entities:staleent1 SET name = 'StaleEntityOne', volatility = 'high', last_refreshed = time::now() - 200d RETURN NONE;" '[[]]'
expect_real_sql "seed readback begins unmarked" \
  "SELECT VALUE [marked_for, mark_type, marked_by] FROM ONLY kg_entities:staleent1;" '[[null,null,null]]'

SHIM_DIR="$WORK_DIR/shim"
mkdir -p "$SHIM_DIR"
SHIM_LOG="$WORK_DIR/shim.invocations"
SHIM_BIN="$SHIM_DIR/surreal"
cat >"$SHIM_BIN" <<'SHIM'
#!/bin/bash
set -u
printf 'invoked\n' >>"${HEALTH_SURREAL_LOG:?}"
case "${HEALTH_SURREAL_SHIM_MODE:-proxy}" in
  proxy)
    cd "${HEALTH_SURREAL_CWD:?}"
    exec "${HEALTH_REAL_SURREAL:?}" "$@"
    ;;
  malformed)
    printf 'not-json\n'
    exit 0
    ;;
  execution_error)
    printf '%s\n' '["forced execution error",[]]'
    exit 0
    ;;
  cli_failure)
    echo 'forced transport failure' >&2
    exit 7
    ;;
  *)
    echo "unknown shim mode: ${HEALTH_SURREAL_SHIM_MODE}" >&2
    exit 64
    ;;
esac
SHIM
chmod 755 "$SHIM_BIN"

export HEALTH_REAL_SURREAL="$REAL_SURREAL_BIN"
export HEALTH_SURREAL_CWD="$WORK_DIR"
export HEALTH_SURREAL_LOG="$SHIM_LOG"
export SURREAL_BIN="$SHIM_BIN"
export SURR_DB_URL="$SQL_ENDPOINT"
export SURR_DB_NS="$TEST_NS"
export SURR_DB_DB="$TEST_DB"
export SURR_DB_USER=root
export SURR_DB_PASS=root

run_health() {
  local label=$1
  local mode=$2
  local dry_run=$3
  local stdout_file="$WORK_DIR/${label}.stdout"
  local stderr_file="$WORK_DIR/${label}.stderr"
  local rc

  : >"$SHIM_LOG"
  if [ "$dry_run" = 1 ]; then
    (cd "$WORK_DIR" && HEALTH_SURREAL_SHIM_MODE="$mode" DRY_RUN=1 bash "$REPO_ROOT/scripts/sm_health.sh") \
      >"$stdout_file" 2>"$stderr_file"
    rc=$?
  else
    (cd "$WORK_DIR" && unset DRY_RUN; HEALTH_SURREAL_SHIM_MODE="$mode" bash "$REPO_ROOT/scripts/sm_health.sh") \
      >"$stdout_file" 2>"$stderr_file"
    rc=$?
  fi
  echo "$rc"
}

echo
echo "-- DRY_RUN short circuit --"
DRY_RC=$(run_health dry proxy 1)
[ "$DRY_RC" -eq 0 ] && pass "DRY_RUN exits 0" || fail "DRY_RUN exits $DRY_RC"
grep -qi 'dry.run' "$WORK_DIR/dry.stdout" && pass "DRY_RUN reports its short circuit" || fail "DRY_RUN message missing"
[ ! -s "$SHIM_LOG" ] && pass "DRY_RUN issued zero surreal invocations" || fail "DRY_RUN invoked surreal"
expect_real_sql "DRY_RUN preserves the stale row" \
  "SELECT VALUE [marked_for, mark_type, marked_by] FROM ONLY kg_entities:staleent1;" '[[null,null,null]]'

echo
echo "-- real stale-row update --"
LIVE_RC=$(run_health live proxy 0)
[ "$LIVE_RC" -eq 0 ] && pass "valid stale-row query exits 0" || fail "valid stale-row query exits $LIVE_RC"
[ "$(cat "$WORK_DIR/live.stdout")" = '[null,[]]' ] && pass "success stdout is the exact [null,[]] contract" || fail "unexpected success stdout: $(cat "$WORK_DIR/live.stdout")"
[ ! -s "$WORK_DIR/live.stderr" ] && pass "success stderr is empty" || fail "success stderr was not empty: $(cat "$WORK_DIR/live.stderr")"
[ "$(wc -l <"$SHIM_LOG" | tr -d ' ')" -eq 1 ] && pass "valid run invoked surreal exactly once" || fail "valid run invocation count was not one"
expect_real_sql "independent readback sees the intended stale mark" \
  "SELECT VALUE [marked_for, mark_type, marked_by] FROM ONLY kg_entities:staleent1;" '[["gemini","research","health"]]'

echo
echo "-- empty eligible set --"
expect_real_sql "remove the seeded stale row" \
  "DELETE kg_entities:staleent1 RETURN NONE;" '[[]]'
EMPTY_RC=$(run_health empty proxy 0)
[ "$EMPTY_RC" -eq 0 ] && pass "empty eligible set exits 0" || fail "empty eligible set exits $EMPTY_RC"
[ "$(cat "$WORK_DIR/empty.stdout")" = '[null,[]]' ] && pass "empty set preserves exact success contract" || fail "empty-set stdout was $(cat "$WORK_DIR/empty.stdout")"
expect_real_sql "independent readback confirms no eligible rows" \
  "SELECT * FROM kg_entities WHERE volatility = 'high' AND marked_for = NONE;" '[[]]'

echo
echo "-- failure-shape controls --"
MALFORMED_RC=$(run_health malformed malformed 0)
[ "$MALFORMED_RC" -ne 0 ] && pass "malformed stdout is rejected" || fail "malformed stdout was accepted"
grep -q 'expected=\[null,\[\]\]' "$WORK_DIR/malformed.stderr" && pass "malformed rejection names the exact contract" || fail "malformed diagnostic did not name the contract"

ERROR_RC=$(run_health execution_error execution_error 0)
[ "$ERROR_RC" -ne 0 ] && pass "query-error JSON with CLI exit 0 is rejected" || fail "query-error JSON was accepted"
grep -q 'forced execution error' "$WORK_DIR/execution_error.stderr" && pass "query-error diagnostic preserves bounded CLI evidence" || fail "query-error evidence missing"

CLI_RC=$(run_health cli_failure cli_failure 0)
[ "$CLI_RC" -ne 0 ] && pass "nonzero surreal CLI exit is rejected" || fail "nonzero surreal CLI exit was accepted"
grep -q 'exit=7' "$WORK_DIR/cli_failure.stderr" && pass "nonzero CLI diagnostic preserves exit 7" || fail "nonzero CLI exit missing from diagnostic"

assert_remini_report() {
  local report=$1
  local expected_success=$2
  /usr/bin/python3 - "$report" "$expected_success" <<'PY'
import json
import sys

path, expected_success = sys.argv[1], sys.argv[2] == "true"
with open(path, encoding="utf-8") as handle:
    report = json.load(handle)
details = report.get("task_details")
assert report.get("tasks_run") == ["health"]
assert isinstance(details, list) and len(details) == 1
detail = details[0]
assert detail.get("name") == "health"
assert detail.get("success") is expected_success
if expected_success:
    assert report.get("summary") == {"tasks_succeeded": 1, "tasks_failed": 0}
    assert detail.get("stdout", "").strip() == "[null,[]]"
    assert detail.get("stderr") == ""
else:
    assert report.get("summary") == {"tasks_succeeded": 0, "tasks_failed": 1}
    assert "failed contract" in detail.get("stderr", "")
PY
}

echo
echo "-- REMini report controls --"
if [ ! -x "$REMINI_BIN" ]; then
  fail "REMini binary unavailable (set REMINI_BIN or build it separately): $REMINI_BIN"
else
  POS_REPORT="$WORK_DIR/remini-positive.json"
  if (cd "$REPO_ROOT" && HEALTH_SURREAL_SHIM_MODE=proxy "$REMINI_BIN" \
      --tasks health --report-path "$POS_REPORT") >"$WORK_DIR/remini-positive.stdout" 2>"$WORK_DIR/remini-positive.stderr"; then
    REMINI_POS_RC=0
  else
    REMINI_POS_RC=$?
  fi
  [ "$REMINI_POS_RC" -eq 0 ] && pass "REMini positive wrapper exits 0" || fail "REMini positive wrapper exits $REMINI_POS_RC"
  if assert_remini_report "$POS_REPORT" true; then
    pass "REMini positive report records health success"
  else
    fail "REMini positive report contract failed"
  fi

  NEG_REPORT="$WORK_DIR/remini-negative.json"
  if (cd "$REPO_ROOT" && HEALTH_SURREAL_SHIM_MODE=execution_error "$REMINI_BIN" \
      --tasks health --report-path "$NEG_REPORT") >"$WORK_DIR/remini-negative.stdout" 2>"$WORK_DIR/remini-negative.stderr"; then
    REMINI_NEG_RC=0
  else
    REMINI_NEG_RC=$?
  fi
  # REMini currently returns process exit 0 even when a task fails; this
  # control scores the persisted report, which is its truthful task witness.
  [ "$REMINI_NEG_RC" -eq 0 ] && pass "REMini negative wrapper retains its current exit-0 contract" || fail "REMini negative wrapper exits $REMINI_NEG_RC"
  if assert_remini_report "$NEG_REPORT" false; then
    pass "REMini negative report records health failure"
  else
    fail "REMini negative report contract failed"
  fi
fi

echo
echo "-- cleanup and repository invariants --"
stop_surreal
if kill -0 "$OWNED_PID" 2>/dev/null || port_is_listening "$TEST_PORT"; then
  fail "owned SurrealDB survived cleanup"
else
  pass "owned SurrealDB PID and listener are gone"
fi

CANONICAL_SNAPSHOT_AFTER=$(snapshot_repo "$CANONICAL_REPO")
TEST_WORKTREE_SNAPSHOT_AFTER=$(snapshot_repo "$REPO_ROOT")
[ "$CANONICAL_SNAPSHOT_AFTER" = "$CANONICAL_SNAPSHOT_BEFORE" ] \
  && pass "canonical branch/head/status/diff are byte-identical after the test" \
  || fail "canonical repository changed during the test"
[ "$TEST_WORKTREE_SNAPSHOT_AFTER" = "$TEST_WORKTREE_SNAPSHOT_BEFORE" ] \
  && pass "test worktree gained no history.txt or other test artifact" \
  || fail "test worktree changed during the test"

echo
if [ "$FAIL" -eq 0 ]; then
  echo "== ALL ASSERTIONS PASSED =="
  exit 0
else
  echo "== ONE OR MORE ASSERTIONS FAILED (see FAIL lines above) =="
  exit 1
fi
