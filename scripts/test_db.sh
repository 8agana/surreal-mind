#!/usr/bin/env bash
# scripts/test_db.sh
#
# fed-93bfee: standing ephemeral SurrealDB wrapper for `cargo test --features
# db_integration`. Starts (or reuses) a throwaway in-memory `surreal start
# memory` instance on a loopback port, exports exactly the environment the
# DB-gated test suite reads, refuses outright if anything resolves to the
# production endpoint (127.0.0.1:8000, ns surreal_mind / db consciousness),
# and tears down only the instance it started itself.
#
# THIS SCRIPT NEVER TOUCHES PRODUCTION. If in doubt, read the HARD REFUSAL
# check below before making any change.
#
# Usage:
#   scripts/test_db.sh [--keep] [--dry-run] [cargo-test-args...]
#     --keep       leave the ephemeral SurrealDB instance running after the
#                  test run and print its connection env for manual poking.
#                  Only meaningful when this script started the instance
#                  itself -- a reused pre-existing instance is never killed
#                  regardless of --keep.
#     --dry-run    print the plan (port, reuse/start, ns/db, env names) and
#                  exit 0 without starting anything or running any test.
#     [cargo-test-args...] forwarded verbatim to
#       `cargo test --features db_integration <args>`, e.g.
#       `--test reembed_dry_run_contract` to target one integration test
#       binary, or a test-name substring filter.
#
# Env contract this script sets (see docs/tasks/20260904-standing-test-db/README.md
# for the full file:line sourcing of every name below):
#   SURR_DB_URL / SURR_DB_NS / SURR_DB_DB / SURR_DB_USER / SURR_DB_PASS
#     -- read by src/config.rs (Config::load), consumed by
#        SurrealMindServer::new() (src/server/db.rs) for every test that
#        builds a full server (tests/mcp_integration.rs, tests/mcp_protocol.rs,
#        tests/test_wander.rs, tests/stdio_smoke.rs, tests/dimension_hygiene.rs).
#   RUN_DB_TESTS=1
#     -- the runtime skip-gate checked by every DB-backed #[tokio::test]/#[test]
#        in this suite (tests/mcp_protocol.rs, tests/mcp_integration.rs,
#        tests/test_wander.rs, tests/stdio_smoke.rs, tests/dimension_hygiene.rs,
#        tests/reembed_dry_run_contract.rs, tests/reembed_bin_dry_run.rs,
#        src/tools/knowledge_graph.rs, src/maintenance/reembed.rs, src/http.rs).
#        Without it every one of those tests prints a skip notice and
#        returns Ok(()) -- the crate's default `cargo test` stays green and
#        never touches a socket.
#   SURR_TEST_DB_URL
#     -- read by tests/reembed_dry_run_contract.rs and
#        tests/reembed_bin_dry_run.rs's own `disposable_db()`/`test_db_url()`
#        guards, which build their own scratch namespace internally (they do
#        NOT use SURR_DB_NS/SURR_DB_DB) and independently refuse any URL
#        containing ":8000".
#   OPENAI_API_KEY -- src/config.rs, consumed by src/embeddings.rs's
#        `create_embedder`/`is_placeholder` check. Set to an obviously-fake
#        value (never the literal "changeme", which embeddings.rs treats as
#        an accepted placeholder rather than a rejected one, and never
#        empty). No test in this suite makes a real embedding call (they
#        either never invoke Embedder::embed, or -- reembed_dry_run_contract.rs
#        -- inject a CountingEmbedder mock instead of the real one), so this
#        key is never sent anywhere; it exists so a regression that DID
#        reach the real embedder would 401 loudly rather than silently using
#        a real credential.
#   GEMINI_API_KEY -- set defensively (obviously fake). NOT currently read by
#        any code path in this crate (verified: `grep -rn '"GEMINI_API_KEY"' src/`
#        is empty) -- Gemini auth in this crate goes through the `gemini` CLI
#        binary on PATH, not an API key env var. Kept per the task brief in
#        case a future test path starts reading it.
#   SM_AGENT_PROVIDER=antigravity -- src/clients/google_cli.rs
#        (GoogleCliProvider::from_env_or_config), top of precedence chain.
#        Forces Google CLI provider selection away from whatever the
#        operator's ambient shell exports, mirroring
#        scripts/test_dryrun_contract.sh. Only tests/test_wander.rs in this
#        suite calls a wander path, and its "random"/"unknown_mode" cases
#        never reach the Google CLI client -- but this is forced anyway so a
#        future db_integration test that DOES exercise kg_populate/kg_wander
#        cannot silently fall through to a real `gemini` CLI call.
#   ANTIGRAVITY_CLI_BIN -- src/clients/antigravity.rs:79. Points at a fake
#        stub script this wrapper writes to $WORK_DIR (same shape as
#        scripts/test_dryrun_contract.sh's fake-agy), so if provider
#        selection above is ever reached it hits a canned response instead
#        of a real CLI.
#
# HARD REFUSAL: if the resolved DB URL, or any pre-existing SURR_DB_URL /
# SURR_TEST_DB_URL in the CALLING environment, contains ":8000" (the
# production SurrealMind port, ns surreal_mind / db consciousness -- see
# AGENTS.md/CLAUDE.md), this script exits 3 before starting anything.
#
# Never runs with `2>/dev/null` anywhere; every command's stderr is visible.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_DIR="$REPO_ROOT/scripts/dryrun_contract"

# --- argument parsing: consume our own flags, forward the rest to cargo test ---
KEEP=0
DRY_RUN=0
CARGO_ARGS=()
while [ $# -gt 0 ]; do
  case "$1" in
    --keep)
      KEEP=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    *)
      CARGO_ARGS+=("$1")
      shift
      ;;
  esac
done

log() { echo "[test_db.sh] $*"; }
run() { log "+ $*"; "$@"; }

# bash 3.2 (macOS system bash) throws "unbound variable" under `set -u` when
# expanding "${arr[@]}" on a genuinely empty array -- this helper renders an
# array as a display string without ever performing that unguarded expansion.
cargo_args_display() {
  if [ "${#CARGO_ARGS[@]}" -gt 0 ]; then
    printf '%s ' "${CARGO_ARGS[@]}"
  fi
}

# --- HARD REFUSAL: check the CALLING environment first, before anything else runs ---
for var in SURR_DB_URL SURR_TEST_DB_URL; do
  val="$(eval "echo \"\${${var}:-}\"")"
  if [ -n "$val" ] && printf '%s' "$val" | grep -q ':8000'; then
    log "REFUSING: \$$var=$val in the calling environment contains ':8000' -- that is the production SurrealMind endpoint (ns surreal_mind / db consciousness). Point it at a disposable instance or unset it. Exiting before starting anything."
    exit 3
  fi
done

# --- resolve surreal binary (non-interactive SSH/CI shells often lack /opt/homebrew/bin on PATH) ---
SURREAL_BIN="${SURREAL_BIN:-surreal}"
if ! command -v "$SURREAL_BIN" >/dev/null 2>&1; then
  if [ -x /opt/homebrew/bin/surreal ]; then
    SURREAL_BIN=/opt/homebrew/bin/surreal
  fi
fi
if ! command -v "$SURREAL_BIN" >/dev/null 2>&1; then
  log "surreal binary not found on PATH and /opt/homebrew/bin/surreal does not exist. Cannot proceed."
  exit 1
fi

# --- port selection: try 8100; reuse if a surreal-memory instance already owns it; else pick a free port ---
DEFAULT_PORT="${TEST_DB_PORT:-8100}"

port_listening() {
  lsof -nP -iTCP:"$1" -sTCP:LISTEN >/dev/null 2>&1
}

surreal_memory_owns_port() {
  pgrep -f "surreal start memory --bind 127.0.0.1:$1 " >/dev/null 2>&1
}

PORT="$DEFAULT_PORT"
REUSE=0
if port_listening "$PORT"; then
  if surreal_memory_owns_port "$PORT"; then
    log "port $PORT is already bound by a 'surreal start memory' process (pid: $(pgrep -f "surreal start memory --bind 127.0.0.1:$PORT " | tr '\n' ' ')) -- reusing it, not starting a new instance, will NOT kill it on exit."
    REUSE=1
  else
    log "port $PORT is in use by something that is NOT a 'surreal start memory' instance -- picking a different free port."
    CANDIDATE=$((PORT + 1))
    while port_listening "$CANDIDATE"; do
      CANDIDATE=$((CANDIDATE + 1))
    done
    PORT="$CANDIDATE"
    log "using free port $PORT instead."
  fi
else
  log "port $PORT is free."
fi

# --- resolved DB URL + refusal check on what we are ABOUT to use ---
DB_URL="127.0.0.1:$PORT"
if printf '%s' "$DB_URL" | grep -q ':8000'; then
  log "REFUSING: resolved DB URL $DB_URL contains ':8000'. This should be unreachable (port selection above never picks 8000), but refusing anyway rather than proceeding. Exiting before starting anything."
  exit 3
fi

# --- scratch namespace/database, unique per run ---
TEST_NS="test_fed93bfee_$$_$(date +%s)"
TEST_DB="scratch"

if [ "$DRY_RUN" -eq 1 ]; then
  log "DRY RUN -- plan only, nothing started, nothing run."
  log "  port: $PORT (reuse=$REUSE)"
  log "  db url: $DB_URL"
  log "  namespace: $TEST_NS  database: $TEST_DB"
  log "  fixture: $FIXTURE_DIR/schema.surql + seed.surql"
  log "  env that would be exported: SURR_DB_URL SURR_DB_NS SURR_DB_DB SURR_DB_USER SURR_DB_PASS RUN_DB_TESTS SURR_TEST_DB_URL OPENAI_API_KEY GEMINI_API_KEY SM_AGENT_PROVIDER ANTIGRAVITY_CLI_BIN"
  log "  would run: cargo test --features db_integration --no-fail-fast $(cargo_args_display)"
  exit 0
fi

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/test_db.XXXXXX")"
log "scratch work dir: $WORK_DIR"

SURREAL_PID=""
WE_STARTED_SURREAL=0

cleanup() {
  if [ "$KEEP" -eq 1 ] && [ "$WE_STARTED_SURREAL" -eq 1 ]; then
    log "--keep set: leaving the SurrealDB instance this script started (pid $SURREAL_PID) running on 127.0.0.1:$PORT."
    log "  connection env for manual poking:"
    log "    export SURR_DB_URL=$DB_URL SURR_DB_NS=$TEST_NS SURR_DB_DB=$TEST_DB SURR_DB_USER=root SURR_DB_PASS=root"
    rm -rf "$WORK_DIR"
    return
  fi
  if [ "$WE_STARTED_SURREAL" -eq 1 ] && [ -n "$SURREAL_PID" ] && kill -0 "$SURREAL_PID" 2>/dev/null; then
    log "killing the SurrealDB instance this script started (pid $SURREAL_PID)."
    kill "$SURREAL_PID" 2>/dev/null || true
    wait "$SURREAL_PID" 2>/dev/null || true
  fi
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

if [ "$REUSE" -eq 0 ]; then
  log "starting throwaway SurrealDB (memory, 127.0.0.1:$PORT)..."
  run "$SURREAL_BIN" start memory --bind "127.0.0.1:$PORT" --user root --pass root \
    > "$WORK_DIR/surreal.log" 2>&1 &
  SURREAL_PID=$!
  WE_STARTED_SURREAL=1
  log "spawned surreal pid $SURREAL_PID, waiting for readiness..."
  READY=0
  for _ in $(seq 1 40); do
    if "$SURREAL_BIN" is-ready --endpoint "http://127.0.0.1:$PORT" >/dev/null 2>&1; then
      READY=1
      break
    fi
    sleep 0.5
  done
  if [ "$READY" -ne 1 ]; then
    log "throwaway SurrealDB did not become ready within 20s; see $WORK_DIR/surreal.log"
    cat "$WORK_DIR/surreal.log" || true
    exit 1
  fi
  log "SurrealDB is ready on 127.0.0.1:$PORT."
else
  if ! "$SURREAL_BIN" is-ready --endpoint "http://127.0.0.1:$PORT" >/dev/null 2>&1; then
    log "reused instance on 127.0.0.1:$PORT did not answer is-ready. Aborting."
    exit 1
  fi
  log "reused SurrealDB instance on 127.0.0.1:$PORT answers is-ready."
fi

# --- apply the shared fixture (fed-734b8f's schema.surql + seed.surql) to the scratch ns/db ---
sql() {
  # `surreal sql` writes a readline history.txt into its CWD with no flag to
  # redirect it -- run it from the scratch work dir so it never dirties the
  # repo (same reason scripts/test_dryrun_contract.sh's sql() helper does this).
  (cd "$WORK_DIR" && "$SURREAL_BIN" sql --endpoint "http://127.0.0.1:$PORT" \
    --username root --password root \
    --namespace "$TEST_NS" --database "$TEST_DB" --pretty)
}

log "applying fixture: schema.surql"
sql < "$FIXTURE_DIR/schema.surql" > "$WORK_DIR/schema_apply.log" 2>&1
cat "$WORK_DIR/schema_apply.log"
log "applying fixture: seed.surql"
sql < "$FIXTURE_DIR/seed.surql" > "$WORK_DIR/seed_apply.log" 2>&1
cat "$WORK_DIR/seed_apply.log"

# --- fake Antigravity CLI stub (mirrors scripts/test_dryrun_contract.sh) ---
FAKE_AGY="$WORK_DIR/fake-agy"
cat > "$FAKE_AGY" << 'FAKE_AGY_EOF'
#!/bin/bash
# Fake Antigravity CLI stub for scripts/test_db.sh. Never calls a real provider.
cat <<'JSON'
{"extractions":[],"summary":"fake-agy canned response: fed-93bfee test_db.sh, no real provider called"}
JSON
exit 0
FAKE_AGY_EOF
chmod +x "$FAKE_AGY"

# --- export exactly the env the tests read (see file:line contract in the header comment above) ---
export SURR_DB_URL="$DB_URL"
export SURR_DB_NS="$TEST_NS"
export SURR_DB_DB="$TEST_DB"
export SURR_DB_USER=root
export SURR_DB_PASS=root
export RUN_DB_TESTS=1
export SURR_TEST_DB_URL="$DB_URL"
export OPENAI_API_KEY="sk-fake-testdb"
export GEMINI_API_KEY="fake-testdb"
export SM_AGENT_PROVIDER=antigravity
export ANTIGRAVITY_CLI_BIN="$FAKE_AGY"

log "environment exported:"
log "  SURR_DB_URL=$SURR_DB_URL SURR_DB_NS=$SURR_DB_NS SURR_DB_DB=$SURR_DB_DB"
log "  SURR_DB_USER=$SURR_DB_USER SURR_DB_PASS=$SURR_DB_PASS"
log "  RUN_DB_TESTS=$RUN_DB_TESTS SURR_TEST_DB_URL=$SURR_TEST_DB_URL"
log "  OPENAI_API_KEY=$OPENAI_API_KEY GEMINI_API_KEY=$GEMINI_API_KEY"
log "  SM_AGENT_PROVIDER=$SM_AGENT_PROVIDER ANTIGRAVITY_CLI_BIN=$ANTIGRAVITY_CLI_BIN"

# --no-fail-fast: this is a STANDING wrapper meant to report results across
# every db_integration test binary in one pass, not stop at the first
# failing one -- without it, cargo test's default fail-fast behavior means
# a single failing test in an alphabetically-early binary (observed:
# tests/dimension_hygiene.rs) silently prevents every later binary
# (tests/mcp_integration.rs, tests/reembed_dry_run_contract.rs, etc.) from
# running at all, which would make this wrapper's pass/fail report useless
# for anything but the first failure.
log "running: cargo test --features db_integration --no-fail-fast $(cargo_args_display)"
cd "$REPO_ROOT"
set +e
if [ "${#CARGO_ARGS[@]}" -gt 0 ]; then
  cargo test --features db_integration --no-fail-fast "${CARGO_ARGS[@]}"
else
  cargo test --features db_integration --no-fail-fast
fi
CARGO_RC=$?
set -e
log "cargo test exit code: $CARGO_RC"
exit "$CARGO_RC"
