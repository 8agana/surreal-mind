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
#     SurrealDB with fake provider stubs wired in for BOTH Google CLIs
#     (Antigravity via ANTIGRAVITY_CLI_BIN, Gemini via a PATH-shadowed
#     `gemini` stub) and SM_AGENT_PROVIDER forced to antigravity at the top
#     of the crate's precedence chain, so provider selection cannot depend
#     on the operator's ambient shell. Assert 0 calls to either fake CLI,
#     an identical DB snapshot before/after, bounded exit, that only the
#     requested report path was written, and that the report lists exactly
#     the six canonical remini tasks each with success:true.
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
FAKE_BIN_DIR="$WORK_DIR/fakebin"
GEMINI_CALLS_FILE="$WORK_DIR/fake-gemini.calls"
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

# Fake `gemini` CLI stub, PATH-shadowed ahead of any real `gemini` binary.
# kg_populate/kg_wander's Gemini client shells out via `Command::new("gemini")`
# with a bare command name looked up on PATH -- there is no env var to
# redirect it (src/clients/gemini.rs:291), unlike Antigravity's
# ANTIGRAVITY_CLI_BIN override above. If provider selection ever picks
# Gemini despite the forcing below (a regression, a future task that skips
# the DRY_RUN guard, ambient config CC didn't anticipate), this is what
# stands between that call and the real `gemini` CLI. It records every
# invocation and fails loudly rather than silently succeeding, so a
# misselection surfaces as a failed task, not a quiet real call.
mkdir -p "$FAKE_BIN_DIR"
cat > "$FAKE_BIN_DIR/gemini" << EOF
#!/bin/bash
echo "\$(date -u +%Y-%m-%dT%H:%M:%SZ) \$*" >> "$GEMINI_CALLS_FILE"
echo "FAKE GEMINI CALLED -- dry-run contract test forces provider=antigravity; this should never run" >&2
exit 17
EOF
chmod +x "$FAKE_BIN_DIR/gemini"

export SURR_DB_URL="127.0.0.1:$TEST_PORT"
export SURR_DB_NS="$TEST_NS"
export SURR_DB_DB="$TEST_DB"
export SURR_DB_USER=root
export SURR_DB_PASS=root
export ANTIGRAVITY_CLI_BIN="$FAKE_AGY"

# Force provider selection to Antigravity (the fake-agy stub above) at the
# TOP of the precedence chain, so this test cannot silently fall through to
# a real Gemini call because of whatever the operator's ambient shell
# happens to export. Precedence, src/clients/google_cli.rs:11-17:
#   SM_AGENT_PROVIDER > GOOGLE_CLI_PROVIDER > SURR_GOOGLE_CLI_PROVIDER
#     > config value > default (Antigravity)
# SM_AGENT_PROVIDER wins regardless of what the lower-precedence vars or
# config hold, so setting it alone is sufficient -- no need to unset the
# others. remini's child processes inherit this script's environment
# (src/bin/remini.rs's Command doesn't call env_clear()), so it reaches
# kg_populate and kg_wander unchanged.
export SM_AGENT_PROVIDER=antigravity
# PATH-shadow: fakebin first, so a bare `gemini` lookup (gemini.rs:291)
# hits the failing stub above, never a real install on this machine's PATH.
export PATH="$FAKE_BIN_DIR:$PATH"
echo "forced provider: SM_AGENT_PROVIDER=$SM_AGENT_PROVIDER (fake-agy=$FAKE_AGY, fake-gemini=$FAKE_BIN_DIR/gemini shadowing PATH)"

echo
echo "-- seeding --"
reseed
echo "seeded: $(snapshot | python3 -c 'import json,sys; print(json.load(sys.stdin)["counts"])')"

echo
echo "== POSITIVE control: remini --all --dry-run =="
export OPENAI_API_KEY="sk-fake-fed734b8f-not-a-real-key-000000000000"
# OpenAI instrumentation: the embedder's HTTP endpoint is hardcoded
# (src/embeddings.rs:155, https://api.openai.com/v1/embeddings) with no
# OPENAI_BASE_URL or similar override in this crate, so redirecting it to a
# local counting stub is not possible from this test harness -- that gap is
# DEFERRED to the same "no offline embedder" deferral this task inherited.
# What we can and do assert: the key is an obviously-fake value, so if the
# DRY_RUN guard in kg_embed's embed path (which never calls .embed(), only
# constructs the embedder) were ever bypassed by a regression, the resulting
# call would 401 against the real endpoint rather than silently succeeding.
case "$OPENAI_API_KEY" in
  sk-fake-*) pass "OPENAI_API_KEY is an obviously-fake value (a real call would 401, not succeed)" ;;
  *) fail "OPENAI_API_KEY does not match the expected fake pattern -- a real key may have leaked in" ;;
esac
snapshot > "$WORK_DIR/snap_pos_before.json"
: > "$CALLS_FILE"
: > "$GEMINI_CALLS_FILE"
POS_REPORT="$WORK_DIR/remini_dry_positive.json"
run_bounded "$WORK_DIR/remini_pos.stdout" 300 \
  "$REMINI" --all --dry-run --timeout 120 --report-path "$POS_REPORT"
snapshot > "$WORK_DIR/snap_pos_after.json"

echo "remini exit=$BOUND_RC elapsed=${BOUND_ELAPSED}s timed_out=$BOUND_TIMED_OUT"
[ "$BOUND_TIMED_OUT" -eq 0 ] && pass "bounded termination (${BOUND_ELAPSED}s < 300s)" \
  || fail "exceeded 300s wall-clock bound"
[ "$BOUND_RC" -eq 0 ] && pass "process exit 0" || fail "process exit $BOUND_RC"

CALLS=$(wc -l < "$CALLS_FILE" | tr -d ' ')
[ "$CALLS" -eq 0 ] && pass "zero Antigravity provider invocations (fake-agy.calls empty)" \
  || fail "fake-agy.calls has $CALLS line(s) under --dry-run"

GEMINI_CALLS=$(wc -l < "$GEMINI_CALLS_FILE" 2>/dev/null | tr -d ' ')
[ "${GEMINI_CALLS:-0}" -eq 0 ] && pass "zero Gemini provider invocations (fake-gemini.calls empty)" \
  || fail "fake-gemini.calls has $GEMINI_CALLS line(s) under --dry-run -- provider selection leaked to Gemini"

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

# Exact positive assertion: not just "six entries" but the six canonical
# remini task names (src/bin/remini.rs:147-152), each reporting success:true.
# A count-only check would pass if, say, "populate" silently failed while an
# unexpected seventh task name ran and something else was subtracted -- name
# and success matter, not just the number.
EXPECTED_TASKS_JSON=$(printf '%s\n' consolidate embed health populate rethink wander \
  | jq -R -c -s 'split("\n") | map(select(length > 0)) | map({name: ., success: true}) | sort_by(.name)')
ACTUAL_TASKS_JSON=$(jq -c -S '[.task_details[] | {name, success}] | sort_by(.name)' "$POS_REPORT" 2>/dev/null)
if [ "$ACTUAL_TASKS_JSON" = "$EXPECTED_TASKS_JSON" ]; then
  pass "report has exactly the six expected tasks, each success:true ($ACTUAL_TASKS_JSON)"
else
  fail "report task_details mismatch -- expected $EXPECTED_TASKS_JSON got ${ACTUAL_TASKS_JSON:-<unparseable>}"
fi

echo
echo "== NEGATIVE control: remini (no --dry-run), tasks=populate,rethink,consolidate =="
echo "   (embed excluded: its OpenAI call is unstubbed, wander/health excluded: unrelated live preconditions)"
reseed
unset OPENAI_API_KEY
snapshot > "$WORK_DIR/snap_neg_before.json"
: > "$CALLS_FILE"
: > "$GEMINI_CALLS_FILE"
NEG_REPORT="$WORK_DIR/remini_live_negative.json"
run_bounded "$WORK_DIR/remini_neg.stdout" 300 \
  "$REMINI" --tasks populate,rethink,consolidate --timeout 120 --report-path "$NEG_REPORT"
snapshot > "$WORK_DIR/snap_neg_after.json"

echo "remini exit=$BOUND_RC elapsed=${BOUND_ELAPSED}s"

NEG_CALLS=$(grep -c -- "--print-timeout" "$CALLS_FILE" 2>/dev/null || echo 0)
[ "$NEG_CALLS" -gt 0 ] && pass "provider WAS invoked without --dry-run ($NEG_CALLS call(s))" \
  || fail "fake-agy was never invoked in the live run -- negative control did not fire"

NEG_GEMINI_CALLS=$(wc -l < "$GEMINI_CALLS_FILE" 2>/dev/null | tr -d ' ')
[ "${NEG_GEMINI_CALLS:-0}" -eq 0 ] && pass "the live provider call went to Antigravity, not Gemini" \
  || fail "fake-gemini.calls has $NEG_GEMINI_CALLS line(s) in the live run -- forced provider selection did not hold"

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
