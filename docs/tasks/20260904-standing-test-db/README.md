# Standing ephemeral test-DB wrapper for `db_integration` tests (fed-93bfee)

`scripts/test_db.sh` is a standing wrapper around `cargo test --features
db_integration`: every run launches and OWNS a fresh throwaway in-memory
SurrealDB instance on a loopback port (never reusing anything already
listening), exports exactly the environment the DB-gated test suite reads,
explicitly sanitizes every network-enabling test gate this wrapper knows
about, applies the shared schema fixture, runs the full `db_integration`
test suite (or a caller-selected subset) with zero external network calls
by default, and tears down the exact child process it started -- verified,
not assumed.

**Never touches production** (`127.0.0.1:8000`, ns `surreal_mind` / db
`consciousness`) and **never reuses an existing process** (including any
pre-existing `surreal start memory` instance already running on this
machine).

## Usage

```
scripts/test_db.sh [--keep] [--dry-run] [--seed] [--allow-network]
                    [--port N] [cargo-test-args...]
```

- `--keep` -- leave the ephemeral SurrealDB instance running after the test
  run and print its connection env for manual poking.
- `--dry-run` -- print the plan and exit 0 without starting anything.
- `--seed` -- ALSO apply `scripts/dryrun_contract/seed.surql` on top of
  `schema.surql` (schema is always applied; seed data is opt-in -- see
  "Fixture: schema always, seed opt-in" below).
- `--allow-network` -- explicit, loud opt-in to the 3 tests that reach
  `api.openai.com` for real. Sets `ALLOW_NETWORK_EMBED=1` and prints a
  banner. Without this flag, `ALLOW_NETWORK_EMBED` is explicitly unset
  regardless of what the calling environment had.
- `--port N` -- start the fresh-instance port scan at `N` instead of the
  default 8100 (or `$TEST_DB_PORT`). Must be numeric, 1024-65535.
- Anything else is forwarded verbatim to
  `cargo test --features db_integration --no-fail-fast <args>`.

Every step is logged with the exact command run; nothing is silenced with
`2>/dev/null`.

---

## Part 1: process ownership (this pass's fix)

**The bug this pass fixed:** the first version of this wrapper started
SurrealDB via a bash function (`run() { log "+ $*"; "$@"; }`) backgrounded
with `run "$SURREAL_BIN" start memory ... & SURREAL_PID=$!`. Backgrounding
a function call captures the PID of the intermediate subshell bash forks to
run that function -- NOT the PID of `surreal` itself, which runs as a
*grandchild* inside that subshell. Cleanup killed the subshell; the actual
`surreal` process was orphaned and kept running, holding the port.

**The fix:** launch the binary directly, with no wrapper function in the
backgrounding chain:
```
"$SURREAL_BIN" start memory --bind "127.0.0.1:$CANDIDATE" --user root --pass root \
  >"$WORK_DIR/surreal.$ATTEMPT.log" 2>&1 &
DB_PID=$!
```
This is a single fork+exec, so `$!` is the real `surreal` PID. Cleanup now:
send TERM, poll for exit up to 5s (0.1s increments), send KILL if still
alive, then assert `! kill -0 "$DB_PID"` and log the outcome explicitly
either way. `trap cleanup EXIT` plus explicit `trap ... INT` / `trap ...
TERM` handlers (each just `exit N`, which in turn fires the EXIT trap)
guarantee cleanup runs regardless of how the script's foreground work ends.

### Controls (2026-09-05, Studio, same worktree/branch, all against a
known-free port so this exercises the FRESH-LAUNCH branch, not a reuse):

**(a) fresh launch + confirmed ownership** -- `scripts/test_db.sh --port
8300 --test tool_roster_db_free`:
```
[test_db.sh] spawned pid 89810 for candidate port 8300
[test_db.sh] SurrealDB pid 89810 is ready and confirmed as the LISTEN owner of 127.0.0.1:8300.
```
`lsof -nP -iTCP:8300 -sTCP:LISTEN` while running showed pid 89810 as the
owner. After the run: `pgrep -fl "surreal start memory"` showed only the
pre-existing, untouched pid 42064; `lsof` on 8300 showed nothing.

**(b) cleanup on success** -- same run: `cargo test exit code: 0` ->
`sending TERM to surreal pid 89810` -> `confirmed: surreal pid 89810 is
not running`.

**(c) cleanup on cargo failure** -- `scripts/test_db.sh --port 8301 --test
nonexistent_binary`: cargo exits 101 (no such test binary); wrapper still
ran `sending TERM to surreal pid 90327` -> `confirmed: surreal pid 90327
is not running`. `pgrep` after: only pid 42064.

**(d) cleanup on SIGINT** -- delivering `SIGINT` to a background `&` job
from within the SAME non-interactive script had NO EFFECT at all in this
SSH/heredoc environment (isolated and confirmed with a trivial `bash -c
'trap ... INT; sleep 10' &`; `sleep 10; NATURAL_END` ran to completion,
`rc=0`, elapsed 10s -- POSIX's "asynchronous list" rule sets SIGINT/SIGQUIT
to ignored for `&`-backgrounded children of a non-interactive shell, and
that ignored disposition survives across `exec` into a new bash). Retested
correctly by running the wrapper as the FOREGROUND command of its own SSH
session (`bash -c 'echo $$ >pidfile; exec bash scripts/test_db.sh --port
8305 --test dimension_hygiene'`) and sending `SIGINT` from a SEPARATE SSH
session to the exact pid captured via the pidfile (the `exec` means no
extra process layer, so the pidfile pid IS the running script). Result:
outer command exited 130 (128+SIGINT); log showed `[test_db.sh] received
SIGINT` -> `sending TERM to surreal pid 90925` -> `confirmed: surreal pid
90925 is not running`. `pgrep` after: only pid 42064. (Honest caveat: the
signal in this run landed just as `cargo test` was finishing rather than
mid-build -- bash defers a trapped signal's handler until the shell
regains control between foreground commands, so a fast test run doesn't
prove interruption of a slow one, only that cleanup fires once the trap
runs. The KILL-after-cargo-failure control (c) and the process-ownership
mechanics (a/b) are the stronger proof that cleanup is unconditional; this
control additionally proves the INT trap itself fires and does the right
thing when it does.)

**(e) cleanup on SIGTERM** -- same method, port 8306: outer command exited
143 (128+SIGTERM); log showed `[test_db.sh] received SIGTERM` -> `sending
TERM to surreal pid 91093` -> `confirmed: surreal pid 91093 is not
running`. `pgrep` after: only pid 42064.

### Follow-up (#216): one bounded `stop_db()` on every exit path

The failed-ready and owner-mismatch branches in the launch loop previously
had their OWN inline `kill -TERM` + unbounded `wait`, duplicating (and not
matching) the bounded TERM->poll->KILL->assert logic that only `cleanup()`
had. Factored into a single function, `stop_db(pid)`, and every exit path
now calls only that: `cleanup()` (success, cargo failure, INT, TERM all
route through the EXIT trap -> `cleanup()` -> `stop_db`), the failed-ready
branch, and the owner-mismatch branch. Port-exhaustion needs no separate
call: by the time `MAX_PORT_ATTEMPTS` is exhausted, the prior iteration's
own `stop_db` call has already left `DB_PID` empty, so the EXIT trap's
`stop_db ""` is a no-op.

`grep -n 'kill\|wait ' scripts/test_db.sh` after the refactor -- every
actual TERM/KILL/wait is inside `stop_db()` (lines 294-311 at the time of
this grep); the one `kill -0` outside it (line 371) is a pure liveness
check in the readiness loop, not a stop action:
```
294:  if ! kill -0 "$pid" 2>/dev/null; then
296:    wait "$pid" 2>/dev/null || true
300:  kill -TERM "$pid" 2>/dev/null || true
302:  while kill -0 "$pid" 2>/dev/null && [ "$i" -lt 50 ]; do
306:  if kill -0 "$pid" 2>/dev/null; then
308:    kill -KILL "$pid" 2>/dev/null || true
310:  wait "$pid" 2>/dev/null || true
311:  if kill -0 "$pid" 2>/dev/null; then
371:    if ! kill -0 "$DB_PID" 2>/dev/null; then   # readiness-loop liveness check, not a stop
```

**Related, in-scope tightening:** to keep the overall failed-ready-to-
stopped budget small, the readiness poll bound was reduced from 40x0.5s
(20s) to 8x0.3s (2.4s) -- every real `surreal start memory` instance in
every proof run in this doc became ready in well under 1s, so this remains
generous for the happy path.

**Stubborn-child control:** `SURREAL_BIN` pointed at a stub that (1) always
fails `is-ready`, (2) on its first `start` invocation, `trap '' TERM`s
itself then `exec`s directly into a small Python one-liner that binds the
requested port and sleeps 60 (`exec`, not a backgrounded child, so there is
no separate grandchild left behind when the stub process itself is
killed -- an earlier draft of this stub backgrounded the Python listener
and left it orphaned after SIGKILL, exactly the Part-1 bug shape recurring
in the TEST HARNESS; fixed by using `exec` instead), and (3) on subsequent
invocations exits immediately (so the wrapper's 20-candidate port-scan
doesn't repeat the same 60s-stubborn behavior 20 times over). Result for
the actual stubborn attempt (candidate 1, pid 94727):
```
[test_db.sh] spawned pid 94727 for candidate port 8380
[test_db.sh] candidate port 8380 unusable (process died or never became ready); log:
stub attempt 1: binding port 8380, ignoring TERM (exec'd python, no separate child), sleeping 60
[test_db.sh] sending TERM to surreal pid 94727
[test_db.sh] pid 94727 still alive 5s after TERM, sending KILL
[test_db.sh] confirmed: surreal pid 94727 is not running
```
-- readiness detection (2.4s bound) plus the TERM-wait-then-KILL cycle
(5s bound) resolves this ONE stubborn attempt in ~7s, not an unbounded
hang. (The wrapper's total exit time was 16s because it then had to
exhaust the remaining 19 candidate ports, each of which the stub fails
fast on by design (per point 3 above) -- that 16s reflects the port-scan
policy from Part 2, not the stop mechanism; the stop mechanism itself,
isolated to the one attempt that actually needed it, is the ~7s above.)
`lsof` on the port range and `pgrep` after: nothing listening, no leftover
process, only pid 42064 remained.

---

## Part 2: fresh instance only (no reuse)

The `pgrep -f "surreal start memory ..."` reuse path from the previous pass
is **removed entirely**. Every run launches a NEW instance. Port selection:
validate (`is_valid_port`: numeric, 1024-65535; `--port N` or
`$TEST_DB_PORT` to start elsewhere), then loop up to `MAX_PORT_ATTEMPTS=20`
candidates starting there:
1. If the candidate is already listening (`lsof -nP -iTCP:$P
   -sTCP:LISTEN`), for ANY reason -- **skip it, never signal it, never
   touch it** -- and try the next port.
2. Otherwise launch `surreal start memory` on it and wait (bounded, 40 x
   0.5s) for either readiness (`surreal is-ready`) or early exit.
3. **Bind-race check**: before trusting the instance, confirm OUR pid is
   the actual LISTEN owner (`lsof -nP -iTCP:$P -sTCP:LISTEN -t` must
   include our `$DB_PID`) -- if something else won the bind race, or the
   process died before/at this check, discard the attempt (kill it if
   still alive) and try the next candidate.
4. Exhausting `MAX_PORT_ATTEMPTS` refuses with a clear message and exit 1.

### Controls

**(a) occupied port -- skipped, never touched:** started a dummy listener
(`python3 -m http.server 8307`, pid 91241) then ran `scripts/test_db.sh
--port 8307 --test tool_roster_db_free`. Log: `port 8307 is already in use
by something -- SKIPPING it (never touching whatever owns it), trying next
candidate` -> launched on 8308 instead, ran clean, cleaned up its OWN
8308 instance. `kill -0 91241` after the wrapper finished: still alive
(the wrapper never signaled it). I killed the dummy listener myself as
test cleanup, not the wrapper.

**(b) early child exit -- fails closed:** pointed `SURREAL_BIN` at a stub
(`#!/bin/bash; echo "fake surreal..." >&2; exit 1`) and ran
`SURREAL_BIN=<stub> scripts/test_db.sh --port 8309 --test
tool_roster_db_free`. The wrapper tried all 20 candidate ports (8309-8328),
detected the early exit on each (`pid N exited early (before becoming
ready) on candidate port P`), then: `REFUSING: exhausted 20 candidate
ports starting at 8309 without a usable one.`, exit 1. `grep -c "applying
fixture" <log>` = 0 -- schema was never applied. `pgrep` after: only pid
42064 (untouched throughout; this control never went near it since it
started at 8309).

`pid 42064` (the pre-existing instance this task's brief called out) was
never reused, signaled, or touched by ANY control above.

---

## Part 3: environment sanitization

**"Not setting a flag does not unset an inherited one."** Every run,
before invoking cargo test, this wrapper explicitly `unset`s (not merely
"doesn't set"):
```
RUN_GEMINI_TESTS SURR_SMOKE_TEST REEMBED_TEST_CONFIRM_DISPOSABLE_NS \
  ALLOW_NETWORK_EMBED GOOGLE_CLI_PROVIDER SURR_GOOGLE_CLI_PROVIDER
```
`ALLOW_NETWORK_EMBED` is re-exported as exactly `"1"` only when
`--allow-network` is passed (with a loud banner). The 3 tests that read it
(see "Network contract" below) now require an EXACT `"1"` match, not mere
presence:
```
if std::env::var("ALLOW_NETWORK_EMBED").ok().as_deref() != Some("1") { ... skip ... }
```
(`tests/mcp_integration.rs:114,155`; `tests/mcp_protocol.rs:413`).

`SURR_ENV_FILE` is pinned to an empty scratch file this wrapper creates
(`: >"$WORK_DIR/empty.env"`), which protects `Config::load`'s own dotenv
step (`src/config.rs:229-231`: `if let Ok(env_path) =
env::var("SURR_ENV_FILE") { dotenvy::from_path(env_path) }` -- an explicit
path read, no upward search, so pinning it fully neutralizes this branch).
`SURREAL_MIND_CONFIG` is also pinned to `$REPO_ROOT/surreal_mind.toml`
(absolute path) as a small, unrelated belt-and-suspenders addition so
config-file resolution is never ambiguous.

### Full `env::var(` inventory (grepped across `tests/` and `src/` this
pass) -- network/test-scope-relevant gates only; the exhaustive raw grep
output (all ~150 matches, including ordinary config knobs like
`SURR_CACHE_MAX`, `SURR_HTTP_*`, etc. that carry no network-reach
implication) is in the task's working notes, not reproduced here in full:

| Var | File:line | What it gates | This wrapper's stance |
|---|---|---|---|
| `RUN_DB_TESTS` | 11 test files + `src/tools/knowledge_graph.rs:523`, `src/maintenance/reembed.rs:1918`, `src/http.rs:593` | master DB-test skip gate | explicitly SET to `1` (intentional master switch) |
| `ALLOW_NETWORK_EMBED` | `tests/mcp_integration.rs:114,155`, `tests/mcp_protocol.rs:413` | 3 tests that reach `api.openai.com` for real | explicitly UNSET; `--allow-network` sets exactly `"1"` |
| `RUN_GEMINI_TESTS` | `tests/gemini_client_integration.rs:12` | real Gemini CLI call (`GeminiClient::new().call()`) | explicitly UNSET |
| `SURR_SMOKE_TEST` | `tests/relationship_smoke.rs:7` (already exact `"1"` match) | test that makes real embed calls via `handle_knowledgegraph_create`/`handle_unified_search` | explicitly UNSET |
| `REEMBED_TEST_CONFIRM_DISPOSABLE_NS` | `tests/dimension_hygiene.rs:106,397,503,588`, also gates `src/maintenance/reembed.rs:1919`, `src/tools/knowledge_graph.rs:524` | whole-table-scan tests that call the REAL `run_reembed`/`run_kg_embed`/`run_reembed_kg` (real embed calls) | explicitly UNSET |
| `SM_AGENT_PROVIDER` / `GOOGLE_CLI_PROVIDER` / `SURR_GOOGLE_CLI_PROVIDER` | `src/clients/google_cli.rs:11-17` (precedence chain, that order) | which Google CLI (Antigravity vs Gemini) provider selection resolves to | `SM_AGENT_PROVIDER=antigravity` SET (top precedence, wins regardless); the two lower-precedence names explicitly UNSET too (defense in depth, even though moot given precedence) |
| `ANTIGRAVITY_CLI_BIN` / `AGY_CLI_BIN` | `src/clients/antigravity.rs:79-80` | which binary `AntigravityClient` shells out to | `ANTIGRAVITY_CLI_BIN` SET to a fake stub (first in the `.or_else` chain, so `AGY_CLI_BIN` never matters) |
| `OPENAI_API_KEY` / `GEMINI_API_KEY` | `src/config.rs:427`; not read anywhere for the latter (verified empty grep) | embedder auth / defensive | both explicitly SET to obviously-fake values |

### Dotenv gap: CLOSED (#216)

The previous pass in this task documented three bare, unconditional
`dotenvy::dotenv()` calls that did not honor `SURR_ENV_FILE` and disclosed
the gap rather than shipping a broken fix (an attempted cwd-redirect had
been tried and empirically disproven -- see the git history of this file
for that account). Codex ruled closing it in-scope. Fixed properly this
pass:

**One shared resolution rule**, `pub fn load_env_file()` in `src/config.rs`:
```rust
pub fn load_env_file() {
    if let Ok(env_path) = std::env::var("SURR_ENV_FILE") {
        let _ = dotenvy::from_path(env_path);
    } else {
        let _ = dotenvy::dotenv();
    }
}
```
If `SURR_ENV_FILE` is set, load ONLY that path -- no fallback, ever. If
unset, ordinary `dotenvy::dotenv()` discovery (unchanged default
behavior). Every dotenv-loading call site reachable by the `db_integration`
test suite now routes through it:
- `src/config.rs`'s own `Config::load()` (replaced its previous two-branch
  `.env`/`../.env` inline logic -- a strict generalization for this
  deployment, since `dotenvy::dotenv()`'s unbounded upward walk finds
  everything that 2-hop check found and more).
- `src/embeddings.rs:238` (`create_embedder`, reached by every test that
  builds a real `SurrealMindServer`).
- `src/lib.rs:24` (`pub fn load_env()`).
- `tests/gemini_client_integration.rs:9` (was unconditional, before that
  test's own `RUN_GEMINI_TESTS` check).
- `src/bin/reembed.rs:25` -- reachable because `tests/
  reembed_bin_dry_run.rs` spawns the compiled `reembed` binary as a
  subprocess, which inherits this process's env (including a pinned
  `SURR_ENV_FILE`) but has its own `main()` entry point with its own
  independent dotenv call.

**Deliberately left unconverted** (grepped, flagged, not silently
skipped): 11 other `src/bin/*.rs` binaries (`admin` x5, `kg_populate`,
`kg_wander`, `kg_embed`, `kg_consolidate`, `kg_dedupe_plan`,
`kg_apply_from_plan`, `kg_debug_tool`, `gem_rethink`, `migration`,
`reembed_kg`) also call bare `dotenvy::dotenv()` at their own `main()`.
None are spawned by anything in the `db_integration` test suite (verified:
`grep -rn 'CARGO_BIN_EXE_' tests/` finds only `CARGO_BIN_EXE_reembed` and
`CARGO_BIN_EXE_surreal-mind`), so converting them was an unrelated,
unbounded blast-radius change to operational CLI tools (`admin` in
particular runs against live production per AGENTS.md) that this task did
not ask for and did not touch.

**Control (b) -- normal behavior preserved**, permanent unit tests in
`src/config.rs`'s `mod tests` (both pass under plain `cargo test`,
5-run-stable):
- `load_env_file_without_surr_env_file_falls_back_to_default_dotenv_discovery`
  -- with `SURR_ENV_FILE` unset, writes a decoy `.env` directly at
  `env!("CARGO_MANIFEST_DIR")` (a compile-time constant, so **chdir-
  independent** -- no `std::env::set_current_dir` call anywhere in this
  test), calls `load_env_file()`, asserts the decoy's variable appears,
  then removes the file (cleanup happens **before** the assertion so a
  failing test can never leave it behind).
- `load_env_file_with_surr_env_file_set_ignores_crate_root_decoy_and_keeps_present_key`
  -- control (a) at the Rust level: with `OPENAI_API_KEY` already set to a
  fake wrapper-style value (mirrors the wrapper's own export) and
  `SURR_ENV_FILE` pinned to an empty scratch file, a decoy `.env` at the
  crate root defining `ALLOW_NETWORK_EMBED=1` and a different fake
  `OPENAI_API_KEY=sk-decoy-...` is proven completely ignored: the decoy's
  probe variable never appears, `ALLOW_NETWORK_EMBED` never appears, and
  `OPENAI_API_KEY` stays exactly `"sk-fake-testdb"`. Prints the resolved
  key's first 8 characters via `eprintln!` (synthetic test values only,
  never a real key) as the "which key did it resolve to" evidence.

  **Both tests mutate process-wide env vars and a shared crate-root file,
  so they're serialized against each other with a `static
  LOAD_ENV_FILE_TEST_LOCK: Mutex<()>`.** This was NOT optional: the first
  version of these tests (without also guarding the pre-existing
  `test_config_loading`, which calls `Config::load()` unconditionally with
  no env control of its own) was OBSERVED to fail exactly this way --
  `test_config_loading`, running concurrently on another thread in the
  same `cargo test --lib` process, read the decoy `.env` sitting at the
  crate root via its own unguarded `Config::load()` call before the decoy
  test had pinned `SURR_ENV_FILE`, leaking the probe variable into the
  shared process environment. Fixed by adding the same lock to
  `test_config_loading`; verified stable across 5 consecutive
  `cargo test --lib` runs afterward. (Other `Config::load()`/
  `create_embedder` call sites in this crate's own `#[cfg(test)]` modules
  -- `src/http.rs:597`, `src/maintenance/reembed.rs:1924` -- are gated
  behind `RUN_DB_TESTS` and/or `#[cfg(feature = "db_integration")]`, so
  they don't run and can't race under a bare `cargo test`; under the
  wrapper's `--features db_integration` invocation they run in entirely
  separate OS processes -- each `tests/*.rs` file and the lib are
  independent binaries with no shared mutable env -- so they were never a
  race risk regardless.)

**Control (a) -- script level, using the actual wrapper:** created a real
decoy `.env` at the crate root (`ALLOW_NETWORK_EMBED=1` /
`OPENAI_API_KEY=sk-decoy`), ran `scripts/test_db.sh --test mcp_integration
-- --nocapture`. Log confirmed `SURR_ENV_FILE=<scratch>/empty.env (size: 0
bytes)`, `ALLOW_NETWORK_EMBED unset`, both gated tests printed `skipped:
reaches api.openai.com...`, 6/6 passed. Removed the decoy immediately
after (`git status --short` showed only the expected source files
modified, decoy gone). `~/Projects/LegacyMind/.env` (the one real ancestor
`.env` this machine has) was never touched, read, or written by anything
in this pass.

### Control: hostile inherited environment

```
RUN_GEMINI_TESTS=1 SURR_SMOKE_TEST=1 ALLOW_NETWORK_EMBED=1 \
  SURR_ENV_FILE=~/Projects/LegacyMind/.env \
  scripts/test_db.sh --test mcp_integration -- --nocapture
```
Result: exit 0, 6/6 passed. Log shows the wrapper's own resolved values won
regardless of the hostile calling environment:
```
SURR_ENV_FILE=/var/.../test_db.XXXXXX/empty.env (size: 0 bytes)
ALLOW_NETWORK_EMBED unset (default -- the 3 network-reaching tests will skip)
```
and, with `--nocapture`, both `test_think_handler` and
`test_think_with_continuity` printed `skipped: reaches api.openai.com;
needs the offline embedder (clu fed-77afac)` and passed trivially. `pgrep`
after: only pid 42064.

### Do NOT claim: fake key / proxy slowdown proves zero egress

An earlier version of this README used the `ALLOW_NETWORK_EMBED=1` +
`HTTPS_PROXY` slowdown experiment (2.6-3.0s normally vs 62.6s with a
black-holed proxy target) as evidence about network behavior *when the
network path IS exercised* (`--allow-network`). That is a true statement
about the opt-in path, but it says nothing about the DEFAULT (offline)
path's guarantee, and using it to imply "zero egress, proven" would be
overclaiming what was actually tested. **The honest statement for the
default path is:** the 3 tests that reach `api.openai.com` are gated behind
an exact `ALLOW_NETWORK_EMBED=1` check and print a skip line and return
before calling the embedder when that var isn't exactly `"1"` (verified
directly above, hostile-env included) -- this wrapper does not run any
other mechanism (proxy checks, `tcpdump`, network namespaces) to verify no
other code path in the 17 test binaries independently reaches the network;
the tests/+src/ audit in the previous pass's README section (grep for every
`.embed(` call site) is what stands behind "no other coverage of real
embedding," not a runtime network trace. There is currently no coverage of
the real (online) embedding path at all (clu fed-77afac tracks building an
offline/mock embedder for that).

---

## Fixture: schema always, seed opt-in

(Unchanged from the prior pass, carried forward.) `schema.surql` is applied
every run; `seed.surql` only with `--seed`. Unconditional seeding broke
`tests/dimension_hygiene.rs::test_reembed_mismatch_reporting` (seeded
`thoughts` rows with no `embedding_dim` polluted an ungrouped `GROUP BY`
query). Verified again this pass: default run of `dimension_hygiene` is
8/8 green; `--seed` reproduces the same `NONE`-group panic at
`tests/dimension_hygiene.rs:263:57` -- a fixture/test mismatch to note, not
patch here.

## How this relates to fed-734b8f's harness

`scripts/test_dryrun_contract.sh` and `scripts/dryrun_contract/
test_health_dryrun.sh` remain narrow, single-purpose bash harnesses that
seed themselves independently of this wrapper (see the file:line contract
above) -- unaffected by anything in this pass.

## Final proof run (2026-09-05, Studio, `surreal-mind-wt-fed93bfee` @
`fed-93bfee/test-db-wrapper`, this commit, #216)

- `cargo fmt --check` -- clean.
- `cargo clippy --all-targets --features db_integration -- -D warnings` --
  clean.
- `cargo test` (DB-free, no features) -- clean across every target; lib
  unit tests **74 passed, 0 failed** (72 + the 2 new `load_env_file` tests),
  stable across 5 consecutive runs.
- `scripts/test_db.sh --port 8420` (full default run, fresh instance,
  sanitized env, no seed, no network) -- **exit 0**, every one of 17 test
  binaries + lib (74) + doctests green or explicitly skipped by its own gate.
  Cleanup confirmed: `sending TERM to surreal pid 96083` -> `confirmed:
  surreal pid 96083 is not running`. `pgrep` after: only pid 42064.
- Refusal, re-proven after all changes: `SURR_TEST_DB_URL=127.0.0.1:8000
  scripts/test_db.sh` -> exit 3, `pgrep -fl "surreal start memory"`
  identical (only pid 42064) before and after.

`pid 42064` was never reused, signaled, or touched across any run in any
pass of this task, including every stray process spawned and cleaned up
while iterating on the stubborn-child control stub itself.
