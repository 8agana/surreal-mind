#!/usr/bin/env bash
# scripts/test_db.sh
#
# fed-93bfee: standing ephemeral SurrealDB wrapper for `cargo test --features
# db_integration`. Every run launches and OWNS a fresh throwaway in-memory
# `surreal start memory` instance on a loopback port (no reuse of anything
# already listening, including whatever else may be running on this
# machine), exports exactly the environment the DB-gated test suite reads,
# explicitly sanitizes every network-enabling test gate this wrapper knows
# about (an absent var can otherwise silently reappear via this crate's own
# bare `dotenvy::dotenv()` calls -- see "Environment sanitization" below),
# refuses outright if anything resolves to the production endpoint
# (127.0.0.1:8000, ns surreal_mind / db consciousness), and tears down the
# exact child process it started -- verified, not assumed.
#
# fed-77afac: by default this wrapper now also builds with the
# `test-embedder` Cargo feature and points the embedding provider at the
# offline, deterministic, zero-network `FakeEmbedder` (SURR_EMBED_PROVIDER
# =fake, SURR_EMBED_STRICT=1) instead of the real OpenAI API. The 3 tests
# that used to require --allow-network and a live api.openai.com call just
# to exercise `handle_legacymind_think` at all (tests/mcp_integration.rs::
# test_think_handler, test_think_with_continuity,
# tests/mcp_protocol.rs::test_call_tool_continuity_fallback_protocol) now
# run every time, offline, with real assertions on the resulting
# `embedding_status`. --allow-network is still available and still means
# something real: it switches those same 3 tests to the pre-existing
# intentionally-invalid-key network path and asserts the OPPOSITE
# (graceful degradation), instead of skipping.
#
# THIS SCRIPT NEVER TOUCHES PRODUCTION and NEVER REUSES AN EXISTING PROCESS.
# If in doubt, read the HARD REFUSAL check and the fresh-instance-only port
# loop below before making any change.
#
# Usage:
#   scripts/test_db.sh [--keep] [--dry-run] [--seed] [--allow-network]
#                       [--port N] [cargo-test-args...]
#     --keep           leave the ephemeral SurrealDB instance running after
#                       the test run and print its connection env for manual
#                       poking. Only meaningful for the instance THIS
#                       invocation started (there is no other kind now).
#     --dry-run        print the plan (start port, port-scan bound, fixture
#                       plan, sanitized/exported env names, the cargo command
#                       that would run) and exit 0 without starting anything
#                       or running any test.
#     --seed           ALSO apply scripts/dryrun_contract/seed.surql on top
#                       of schema.surql (schema is always applied; seed data
#                       is opt-in -- see "Fixture: schema always, seed
#                       opt-in" in docs/tasks/20260904-standing-test-db/
#                       README.md for why).
#     --allow-network  explicit, loud opt-in for the 3 tests that CAN reach
#                       api.openai.com for real (fed-77afac: this is now a
#                       SWITCH, not a skip-gate -- without it those 3 tests
#                       still run, offline, against the deterministic
#                       FakeEmbedder). With --allow-network they instead
#                       exercise the real network path with an
#                       intentionally invalid key and assert it degrades
#                       gracefully rather than hard-failing (see the
#                       README's "Network contract" section). Sets
#                       ALLOW_NETWORK_EMBED=1 for the cargo test invocation
#                       and prints a banner. Without this flag,
#                       ALLOW_NETWORK_EMBED is explicitly unset regardless
#                       of what the calling environment had, and
#                       SURR_EMBED_PROVIDER=fake / SURR_EMBED_STRICT=1 are
#                       exported instead.
#     --port N         start the fresh-instance port scan at N instead of
#                       the default 8100 (or $TEST_DB_PORT). Must be numeric,
#                       1024-65535.
#     [cargo-test-args...] forwarded verbatim to
#       `cargo test --features db_integration,test-probe,test-embedder --no-fail-fast <args>`,
#       e.g. `--test reembed_dry_run_contract` to target one integration
#       test binary, or a test-name substring filter.
#
# Env contract this script sets (see docs/tasks/20260904-standing-test-db/
# README.md for the full file:line sourcing of every name below and the
# complete env::var( inventory this pass grepped across tests/ and src/):
#   SURR_DB_URL / SURR_DB_NS / SURR_DB_DB / SURR_DB_USER / SURR_DB_PASS
#     -- read by src/config.rs (Config::load), consumed by
#        SurrealMindServer::new() (src/server/db.rs) for every test that
#        builds a full server.
#   RUN_DB_TESTS=1 -- the runtime skip-gate checked by every DB-backed test
#        in this suite. Without it every one of those tests prints a skip
#        notice and returns Ok(())/passes trivially.
#   SURR_TEST_DB_URL -- read by tests/reembed_dry_run_contract.rs and
#        tests/reembed_bin_dry_run.rs's own disposable_db()/test_db_url()
#        guards, which build their own scratch namespace internally and
#        independently refuse any URL containing ":8000".
#   OPENAI_API_KEY="sk-fake-testdb" / GEMINI_API_KEY="fake-testdb"
#        -- obviously-fake values, never empty, never the literal
#        "changeme" (src/embeddings.rs's is_placeholder treats that as an
#        ACCEPTED placeholder, not rejected). Only actually reaches the
#        network with --allow-network (see SURR_EMBED_PROVIDER below);
#        otherwise it's inert since the fake provider never looks at it.
#   SURR_ALLOW_FAKE_EMBEDDER=1 (fed-77afac review round 2) -- exported ONLY on
#        the default (offline) path, alongside SURR_EMBED_PROVIDER=fake.
#        create_embedder()'s "fake" arm (src/embeddings.rs) refuses to build a
#        FakeEmbedder unless this is exactly "1". This wrapper sanitizes then
#        re-sets the variable to make its own subprocess deterministic. That
#        env-keyed check is operational policy, not an authorization barrier:
#        dotenv and inherited env become indistinguishable once loaded.
#        The deployed barrier is the non-default `test-embedder` feature:
#        plain `cargo build --release` excludes the fake arm entirely, while
#        `cargo build --release --features test-embedder` and `--all-features`
#        include it.
#   SURR_EMBED_PROVIDER=fake / SURR_EMBED_STRICT=1 (fed-77afac) -- default
#        (no --allow-network): selects the offline, deterministic,
#        zero-network FakeEmbedder (src/embeddings.rs, requires this
#        wrapper's `test-embedder` cargo feature) instead of a real OpenAI
#        call. SURR_EMBED_PROVIDER is read by Config::load (src/config.rs);
#        SURR_EMBED_STRICT only gates src/main.rs's startup dimension
#        preflight, which none of this suite's tests reach directly, but is
#        set anyway to match documented intent. With --allow-network,
#        SURR_EMBED_PROVIDER is left UNSET so Config::load falls back to
#        surreal_mind.toml's "openai", and the 3 network-capable tests use
#        the (fake, bound-to-degrade) OPENAI_API_KEY above for real.
#   SM_AGENT_PROVIDER=antigravity / GOOGLE_CLI_PROVIDER (unset) /
#   SURR_GOOGLE_CLI_PROVIDER (unset) -- forces Google CLI provider selection
#        at the top of src/clients/google_cli.rs's precedence chain,
#        regardless of what the calling environment exports for the two
#        lower-precedence names (which are explicitly unset here too, for
#        defense in depth even though SM_AGENT_PROVIDER already wins).
#   ANTIGRAVITY_CLI_BIN -- points at a fake stub this wrapper writes into
#        its scratch work dir; a canned JSON response, never a real CLI.
#   ALLOW_NETWORK_EMBED -- unset by default; set to exactly "1" only with
#        --allow-network. The three tests that read it now require an EXACT
#        "1" match (not mere presence). fed-77afac: this no longer skips
#        those 3 tests -- it switches them from the offline FakeEmbedder
#        path to the real, intentionally-invalid-key network path (and
#        their assertions flip accordingly; see the top-of-file fed-77afac
#        note).
#
# Environment sanitization (network-enabling gates this wrapper knows about,
# explicitly unset every run regardless of what the CALLING shell already
# exported -- "not setting a flag does not unset an inherited one"):
#   RUN_GEMINI_TESTS, SURR_SMOKE_TEST, REEMBED_TEST_CONFIRM_DISPOSABLE_NS,
#   ALLOW_NETWORK_EMBED, GOOGLE_CLI_PROVIDER, SURR_GOOGLE_CLI_PROVIDER.
# This closes the "hostile inherited shell" case (proven: even with all of
# the above pre-set to network-enabling values in the CALLING shell before
# invoking this script, the exported/sanitized values win).
#
# SURR_ENV_FILE is pinned to an empty scratch file this wrapper creates, so
# every helper-routed dotenv caller reached by this suite loads that exact
# empty file rather than an ancestor `.env`.
#
# CORRECTED INVENTORY (fed-acca0a): the three callers formerly named here --
# embeddings::create_embedder, lib::load_env, and
# tests/gemini_client_integration -- now route through load_env_file() and
# honor this wrapper's SURR_ENV_FILE pin. The crate still has 16 direct bare
# `dotenvy::dotenv()` calls across 11 `src/bin/*.rs` files. None of those
# binaries is spawned by this DB-backed test suite, so they do not bypass
# this wrapper's subprocess contract. They remain relevant to the broader
# architecture: their upward-searching dotenv behavior is trusted operator
# configuration, not a security boundary.
#
# HARD REFUSAL: if the resolved DB URL, or any pre-existing SURR_DB_URL /
# SURR_TEST_DB_URL in the CALLING environment, contains ":8000" (the
# production SurrealMind port, ns surreal_mind / db consciousness -- see
# AGENTS.md/CLAUDE.md), this script exits 3 before starting anything.
#
# FRESH INSTANCE ONLY: this script never reuses an already-listening
# instance, including any pre-existing `surreal start memory` process. If
# the candidate port is occupied by ANYTHING, it is skipped -- never
# signaled, never touched -- and the next candidate port is tried, up to
# MAX_PORT_ATTEMPTS times.
#
# Never runs with `2>/dev/null` anywhere; every command's stderr is visible.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_DIR="$REPO_ROOT/scripts/dryrun_contract"
MAX_PORT_ATTEMPTS=20

# --- argument parsing: consume our own flags, forward the rest to cargo test ---
KEEP=0
DRY_RUN=0
SEED=0
ALLOW_NETWORK=0
PORT_ARG=""
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
    --seed)
      SEED=1
      shift
      ;;
    --allow-network)
      ALLOW_NETWORK=1
      shift
      ;;
    --port)
      PORT_ARG="${2:-}"
      shift 2
      ;;
    *)
      CARGO_ARGS+=("$1")
      shift
      ;;
  esac
done

log() { echo "[test_db.sh] $*"; }

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

# --- port validation ---
is_valid_port() {
  case "$1" in
    '' | *[!0-9]*) return 1 ;;
  esac
  [ "$1" -ge 1024 ] && [ "$1" -le 65535 ]
}

if [ -n "$PORT_ARG" ]; then
  if ! is_valid_port "$PORT_ARG"; then
    log "REFUSING: --port $PORT_ARG is not a valid numeric port in 1024-65535."
    exit 2
  fi
  START_PORT="$PORT_ARG"
else
  START_PORT="${TEST_DB_PORT:-8100}"
  if ! is_valid_port "$START_PORT"; then
    log "REFUSING: TEST_DB_PORT=$START_PORT is not a valid numeric port in 1024-65535."
    exit 2
  fi
fi

port_listening() {
  lsof -nP -iTCP:"$1" -sTCP:LISTEN >/dev/null 2>&1
}

refuse_if_prod_url() {
  if printf '%s' "$1" | grep -q ':8000'; then
    log "REFUSING: $1 contains ':8000'. Exiting before starting anything."
    exit 3
  fi
}

# --- scratch namespace/database, unique per run ---
TEST_NS="test_fed93bfee_$$_$(date +%s)"
TEST_DB="scratch"

if [ "$DRY_RUN" -eq 1 ]; then
  log "DRY RUN -- plan only, nothing started, nothing run."
  log "  fresh instance only: start scanning at port $START_PORT, up to $MAX_PORT_ATTEMPTS candidates, never reusing anything already listening"
  log "  namespace: $TEST_NS  database: $TEST_DB"
  if [ "$SEED" -eq 1 ]; then
    log "  fixture: $FIXTURE_DIR/schema.surql + seed.surql (--seed passed)"
  else
    log "  fixture: $FIXTURE_DIR/schema.surql only (pass --seed to also apply seed.surql)"
  fi
  log "  env sanitized (explicitly unset unless the corresponding opt-in flag is passed): RUN_GEMINI_TESTS SURR_SMOKE_TEST REEMBED_TEST_CONFIRM_DISPOSABLE_NS ALLOW_NETWORK_EMBED GOOGLE_CLI_PROVIDER SURR_GOOGLE_CLI_PROVIDER SURR_ALLOW_FAKE_EMBEDDER"
  log "  SURR_ENV_FILE pinned to an empty scratch file; every helper-routed dotenv caller reached by this suite honors the pin. SURREAL_MIND_CONFIG pinned to $REPO_ROOT/surreal_mind.toml (belt-and-suspenders, config-file resolution unambiguous)"
  log "  dotenv inventory: 16 direct bare calls remain across 11 src/bin files, but this suite spawns none of those binaries. Environment values remain trusted operator configuration, not an authorization boundary."
  if [ "$ALLOW_NETWORK" -eq 1 ]; then
    log "  --allow-network passed: ALLOW_NETWORK_EMBED=1 would be exported, SURR_EMBED_PROVIDER left unset (surreal_mind.toml's \"openai\" applies) -- the 3 network-capable tests would exercise the real, intentionally-invalid-key, bound-to-degrade path"
  else
    log "  --allow-network NOT passed: ALLOW_NETWORK_EMBED stays unset, SURR_EMBED_PROVIDER=fake / SURR_EMBED_STRICT=1 / SURR_ALLOW_FAKE_EMBEDDER=1 would be exported -- the 3 network-capable tests run offline against the deterministic FakeEmbedder (fed-77afac). Without SURR_ALLOW_FAKE_EMBEDDER=1, create_embedder REFUSES provider \"fake\" at runtime"
  fi
  log "  would run: cargo test --features db_integration,test-probe,test-embedder --no-fail-fast $(cargo_args_display)"
  exit 0
fi

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/test_db.XXXXXX")"
log "scratch work dir: $WORK_DIR"

DB_PID=""
WE_STARTED_SURREAL=0
PORT=""

# --- the ONE stop path: every exit route (success, cargo failure, INT,
# TERM, failed-ready, owner-mismatch, port-exhaustion) calls this and only
# this to actually terminate a surreal child. TERM -> poll <=5s -> KILL ->
# wait/reap -> assert + log the outcome either way. Idempotent (safe to
# call with an empty pid, or a pid that's already dead).
stop_db() {
  local pid="$1"
  if [ -z "$pid" ]; then
    return 0
  fi
  if ! kill -0 "$pid" 2>/dev/null; then
    log "surreal pid $pid already not running"
    wait "$pid" 2>/dev/null || true
    return 0
  fi
  log "sending TERM to surreal pid $pid"
  kill -TERM "$pid" 2>/dev/null || true
  local i=0
  while kill -0 "$pid" 2>/dev/null && [ "$i" -lt 50 ]; do
    sleep 0.1
    i=$((i + 1))
  done
  if kill -0 "$pid" 2>/dev/null; then
    log "pid $pid still alive 5s after TERM, sending KILL"
    kill -KILL "$pid" 2>/dev/null || true
  fi
  wait "$pid" 2>/dev/null || true
  if kill -0 "$pid" 2>/dev/null; then
    log "WARNING: pid $pid still appears alive after KILL -- this should never happen"
    return 1
  fi
  log "confirmed: surreal pid $pid is not running"
  return 0
}

cleanup() {
  local rc=$?
  if [ -n "$DB_PID" ] && [ "$KEEP" -eq 1 ] && [ "$WE_STARTED_SURREAL" -eq 1 ]; then
    log "--keep set: leaving the SurrealDB instance this script started (pid $DB_PID) running on 127.0.0.1:$PORT."
    log "  connection env for manual poking:"
    log "    export SURR_DB_URL=127.0.0.1:$PORT SURR_DB_NS=$TEST_NS SURR_DB_DB=$TEST_DB SURR_DB_USER=root SURR_DB_PASS=root"
  else
    stop_db "$DB_PID"
    DB_PID=""
  fi
  rm -rf "$WORK_DIR"
  return "$rc"
}
trap cleanup EXIT
trap 'log "received SIGINT"; exit 130' INT
trap 'log "received SIGTERM"; exit 143' TERM

# --- fresh-instance-only launch loop: try up to MAX_PORT_ATTEMPTS candidate ports, never reuse ---
CANDIDATE="$START_PORT"
ATTEMPT=0
while :; do
  ATTEMPT=$((ATTEMPT + 1))
  if [ "$ATTEMPT" -gt "$MAX_PORT_ATTEMPTS" ]; then
    log "REFUSING: exhausted $MAX_PORT_ATTEMPTS candidate ports starting at $START_PORT without a usable one."
    exit 1
  fi
  if ! is_valid_port "$CANDIDATE"; then
    log "REFUSING: candidate port $CANDIDATE left the valid 1024-65535 range."
    exit 1
  fi
  refuse_if_prod_url "127.0.0.1:$CANDIDATE"

  if port_listening "$CANDIDATE"; then
    log "port $CANDIDATE is already in use by something -- SKIPPING it (never touching whatever owns it), trying next candidate."
    CANDIDATE=$((CANDIDATE + 1))
    continue
  fi

  log "launching fresh SurrealDB (memory) on 127.0.0.1:$CANDIDATE (attempt $ATTEMPT/$MAX_PORT_ATTEMPTS)..."
  log "+ $SURREAL_BIN start memory --bind 127.0.0.1:$CANDIDATE --user root --pass root"
  "$SURREAL_BIN" start memory --bind "127.0.0.1:$CANDIDATE" --user root --pass root \
    >"$WORK_DIR/surreal.$ATTEMPT.log" 2>&1 &
  DB_PID=$!
  log "spawned pid $DB_PID for candidate port $CANDIDATE"

  # Bounded readiness wait: 8 iterations x 0.3s sleep = 2.4s of SLEEP
  # BUDGET, not a wall-clock deadline -- each iteration also runs a
  # synchronous `surreal is-ready` subprocess whose own duration is not
  # bounded by this loop (it has no measured/enforced timeout of its own),
  # so actual wall-clock time is this sleep budget PLUS however long those
  # `is-ready` invocations take. Every observed real `surreal start memory`
  # instance in this task's proof runs became ready in well under 1s, so
  # this remains generous for the happy path while keeping the readiness
  # phase of the overall failed-ready-to-stopped budget small (this budget
  # + stop_db's own <=5s TERM-wait bound), per the stubborn-child control
  # below.
  READY=0
  for _ in $(seq 1 8); do
    if ! kill -0 "$DB_PID" 2>/dev/null; then
      log "pid $DB_PID exited early (before becoming ready) on candidate port $CANDIDATE"
      break
    fi
    if "$SURREAL_BIN" is-ready --endpoint "http://127.0.0.1:$CANDIDATE" >/dev/null 2>&1; then
      READY=1
      break
    fi
    sleep 0.3
  done

  if [ "$READY" -ne 1 ]; then
    log "candidate port $CANDIDATE unusable (process died or never became ready); log:"
    cat "$WORK_DIR/surreal.$ATTEMPT.log" 2>/dev/null || true
    stop_db "$DB_PID"
    DB_PID=""
    CANDIDATE=$((CANDIDATE + 1))
    continue
  fi

  # Bind-race check: confirm OUR pid is actually the LISTEN owner of the port
  # before trusting it (something else could have bound it in the gap
  # between our free-port check and our own bind, or `surreal` itself could
  # be a wrapper around a different real listener).
  OWNER_PIDS="$(lsof -nP -iTCP:"$CANDIDATE" -sTCP:LISTEN -t 2>/dev/null || true)"
  if ! printf '%s\n' "$OWNER_PIDS" | grep -qx "$DB_PID"; then
    log "bind race on port $CANDIDATE: pid $DB_PID is NOT the LISTEN owner (owner(s): ${OWNER_PIDS:-none}) -- discarding this attempt, trying next candidate."
    stop_db "$DB_PID"
    DB_PID=""
    CANDIDATE=$((CANDIDATE + 1))
    continue
  fi

  PORT="$CANDIDATE"
  WE_STARTED_SURREAL=1
  log "SurrealDB pid $DB_PID is ready and confirmed as the LISTEN owner of 127.0.0.1:$PORT."
  break
done

DB_URL="127.0.0.1:$PORT"

# --- apply the shared fixture (fed-734b8f's schema.surql, ALWAYS; seed.surql, opt-in via --seed) to the scratch ns/db ---
sql() {
  # `surreal sql` writes a readline history.txt into its CWD with no flag to
  # redirect it -- run it from the scratch work dir so it never dirties the
  # repo (same reason scripts/test_dryrun_contract.sh's sql() helper does this).
  (cd "$WORK_DIR" && "$SURREAL_BIN" sql --endpoint "http://127.0.0.1:$PORT" \
    --username root --password root \
    --namespace "$TEST_NS" --database "$TEST_DB" --pretty)
}

log "applying fixture: schema.surql"
sql <"$FIXTURE_DIR/schema.surql" >"$WORK_DIR/schema_apply.log" 2>&1
cat "$WORK_DIR/schema_apply.log"
if [ "$SEED" -eq 1 ]; then
  log "applying fixture: seed.surql (--seed passed)"
  sql <"$FIXTURE_DIR/seed.surql" >"$WORK_DIR/seed_apply.log" 2>&1
  cat "$WORK_DIR/seed_apply.log"
else
  log "skipping seed.surql (default -- pass --seed to apply it; scripts/test_dryrun_contract.sh and scripts/dryrun_contract/test_health_dryrun.sh seed themselves independently of this wrapper and are unaffected either way)"
fi

# --- fake Antigravity CLI stub (mirrors scripts/test_dryrun_contract.sh) ---
FAKE_AGY="$WORK_DIR/fake-agy"
cat >"$FAKE_AGY" <<'FAKE_AGY_EOF'
#!/bin/bash
# Fake Antigravity CLI stub for scripts/test_db.sh. Never calls a real provider.
cat <<'JSON'
{"extractions":[],"summary":"fake-agy canned response: fed-93bfee test_db.sh, no real provider called"}
JSON
exit 0
FAKE_AGY_EOF
chmod +x "$FAKE_AGY"

# --- empty scratch env file: SURR_ENV_FILE pin (see header comment) ---
EMPTY_ENV_FILE="$WORK_DIR/empty.env"
: >"$EMPTY_ENV_FILE"

# --- environment sanitization: explicitly UNSET every network-enabling gate ---
# this wrapper knows about, regardless of what the calling shell already
# exported. "Not setting a flag does not unset an inherited one" -- these
# must be removed, not merely left alone.
unset RUN_GEMINI_TESTS SURR_SMOKE_TEST REEMBED_TEST_CONFIRM_DISPOSABLE_NS ALLOW_NETWORK_EMBED GOOGLE_CLI_PROVIDER SURR_GOOGLE_CLI_PROVIDER SURR_EMBED_PROVIDER SURR_EMBED_STRICT SURR_ALLOW_FAKE_EMBEDDER 2>/dev/null || true

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
export SURR_ENV_FILE="$EMPTY_ENV_FILE"
export SURREAL_MIND_CONFIG="$REPO_ROOT/surreal_mind.toml"

if [ "$ALLOW_NETWORK" -eq 1 ]; then
  log "=================================================================="
  log "  --allow-network: OPTING IN TO REAL api.openai.com CALLS"
  log "  ALLOW_NETWORK_EMBED=1 -- 3 tests will attempt a real (fake-key,"
  log "  bound-to-degrade) embedding call against the live OpenAI API."
  log "  SURR_EMBED_PROVIDER left unset (surreal_mind.toml's \"openai\" applies)."
  log "=================================================================="
  export ALLOW_NETWORK_EMBED=1
else
  # fed-77afac: default path. Offline, deterministic, zero-network
  # FakeEmbedder -- requires this wrapper's `test-embedder` cargo feature
  # (added to the cargo test invocation below).
  export SURR_EMBED_PROVIDER=fake
  export SURR_EMBED_STRICT=1
  # fed-77afac review round 2: create_embedder()'s "fake" arm refuses to
  # construct the fake embedder unless this operational opt-in is set. This
  # wrapper sanitizes then sets it deterministically because the DB below is
  # a throwaway in-memory instance on a random loopback port, vetted by
  # refuse_if_prod_url. The deployed barrier is compile-time exclusion: a
  # default-feature release has no fake arm, while --features test-embedder
  # and --all-features releases do include it.
  export SURR_ALLOW_FAKE_EMBEDDER=1
fi

log "environment exported:"
log "  SURR_DB_URL=$SURR_DB_URL SURR_DB_NS=$SURR_DB_NS SURR_DB_DB=$SURR_DB_DB"
log "  SURR_DB_USER=$SURR_DB_USER SURR_DB_PASS=$SURR_DB_PASS"
log "  RUN_DB_TESTS=$RUN_DB_TESTS SURR_TEST_DB_URL=$SURR_TEST_DB_URL"
log "  OPENAI_API_KEY=$OPENAI_API_KEY GEMINI_API_KEY=$GEMINI_API_KEY"
log "  SM_AGENT_PROVIDER=$SM_AGENT_PROVIDER ANTIGRAVITY_CLI_BIN=$ANTIGRAVITY_CLI_BIN"
log "  SURR_ENV_FILE=$SURR_ENV_FILE (size: $(wc -c <"$SURR_ENV_FILE" | tr -d ' ') bytes)"
log "  SURREAL_MIND_CONFIG=$SURREAL_MIND_CONFIG"
log "  SURR_EMBED_PROVIDER=${SURR_EMBED_PROVIDER:-<unset, openai from toml>} SURR_EMBED_STRICT=${SURR_EMBED_STRICT:-<unset>}"
log "  SURR_ALLOW_FAKE_EMBEDDER=${SURR_ALLOW_FAKE_EMBEDDER:-<unset -- create_embedder will refuse provider \"fake\">}"
log "  sanitized (unset, not merely left alone): RUN_GEMINI_TESTS SURR_SMOKE_TEST REEMBED_TEST_CONFIRM_DISPOSABLE_NS GOOGLE_CLI_PROVIDER SURR_GOOGLE_CLI_PROVIDER (SURR_ALLOW_FAKE_EMBEDDER too, then re-set above on the offline path only)"
if [ -n "${ALLOW_NETWORK_EMBED:-}" ]; then
  log "  ALLOW_NETWORK_EMBED=$ALLOW_NETWORK_EMBED (--allow-network passed -- the 3 network-capable tests exercise the real, bound-to-degrade path)"
else
  log "  ALLOW_NETWORK_EMBED unset (default -- the 3 network-capable tests run offline against the FakeEmbedder)"
fi

# --no-fail-fast: this is a STANDING wrapper meant to report results across
# every db_integration test binary in one pass, not stop at the first
# failing one.
#
# NOTE: an earlier draft tried to redirect cargo test's working directory to
# starve bare `dotenvy::dotenv()` calls of a reachable ancestor `.env`.
# `cargo test --manifest-path X` was measured to anchor test binaries at the
# crate root, so that redirect bought nothing and was removed. The callers
# reached by this suite now honor the explicit empty SURR_ENV_FILE pin; the
# remaining direct bare calls belong to auxiliary binaries this suite does
# not spawn.
log "running: cargo test --features db_integration,test-probe,test-embedder --no-fail-fast $(cargo_args_display)"
cd "$REPO_ROOT"
set +e
if [ "${#CARGO_ARGS[@]}" -gt 0 ]; then
  cargo test --features db_integration,test-probe,test-embedder --no-fail-fast "${CARGO_ARGS[@]}"
else
  cargo test --features db_integration,test-probe,test-embedder --no-fail-fast
fi
CARGO_RC=$?
set -e
log "cargo test exit code: $CARGO_RC"
exit "$CARGO_RC"
