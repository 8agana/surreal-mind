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

### Acknowledged residual gap (not fully closed, disclosed rather than
hidden)

This crate has **three bare, unconditional `dotenvy::dotenv()` calls** that
do **not** honor `SURR_ENV_FILE` and are **not** blocked by anything this
wrapper does:
- `src/embeddings.rs:238` -- inside `create_embedder`, reached by every
  test that builds a real `SurrealMindServer`.
- `src/lib.rs:24` -- `pub fn load_env()`; not currently called by any test
  in this suite, but exported for external use.
- `tests/gemini_client_integration.rs:9` -- called **unconditionally**,
  before that test's own `RUN_GEMINI_TESTS` check.

`dotenvy::dotenv()` walks upward from the test binary's own current
directory looking for a file literally named `.env`, and (per dotenv/
dotenvy's universal convention) never overrides a variable that is already
present in the process environment -- only genuinely absent ones.

**An attempt to redirect cargo test's own working directory to a scratch
dir with no reachable ancestor `.env` was tried in this pass and
EMPIRICALLY DISPROVEN, then reverted.** Direct test: with a decoy `.env`
(`RUN_GEMINI_TESTS=1`) placed at the crate root and `RUN_GEMINI_TESTS`
genuinely absent from the shell, `cargo test --manifest-path
$REPO_ROOT/Cargo.toml --features db_integration --test
gemini_client_integration` run from an unrelated `mktemp -d` scratch
directory **still** picked up the decoy and attempted a real call
(`Error: cli executable not found` -- the gate was bypassed and it tried).
The same decoy placed ONLY inside that scratch directory (none at the
crate root) was **not** found -- the test correctly skipped. Conclusion:
`cargo test`'s test binaries run with their current directory anchored at
the crate root regardless of where `cargo` itself was invoked from, so
there is no cwd lever available to this wrapper for these three call
sites. (This also means the earlier draft's `SURREAL_MIND_CONFIG`-plus-
cwd-redirect combination in this same pass provided no actual protection
for the claimed purpose; the cwd redirect has been removed, `cargo test`
runs from `$REPO_ROOT` as before this pass.)

**Is this exploitable today?** This machine has a real ancestor `.env` one
directory above every worktree (`~/Projects/LegacyMind/.env`), which
`dotenvy::dotenv()`'s upward walk from the crate root (which has no `.env`
of its own, verified) would reach. Checked its variable **names only**
(never read a value): it defines `SURR_DB_URL`, `SURR_DB_NS`, `SURR_DB_DB`,
`SURR_DB_USER`, `SURR_DB_PASS`, `OPENAI_API_KEY`, `GEMINI_API_KEY` (among
others unrelated to this crate) -- every one of which this wrapper already
explicitly exports itself before cargo test runs, so dotenvy's
"never-override-an-already-set-var" rule protects them regardless of this
gap. It does **not** define `RUN_GEMINI_TESTS`, `SURR_SMOKE_TEST`,
`REEMBED_TEST_CONFIRM_DISPOSABLE_NS`, or `ALLOW_NETWORK_EMBED`. **So this
gap is not currently exploitable on this machine** -- but it is a
structural gap, not a closed one: if that file (or any other ancestor
`.env` on a different machine) ever defines one of those four names, it
will silently reappear despite this wrapper's explicit `unset`. The
correct full fix is out of scope here (editing `src/embeddings.rs`,
`src/lib.rs`, and `tests/gemini_client_integration.rs` themselves to honor
`SURR_ENV_FILE` or skip dotenv under `RUN_DB_TESTS`) and is left as a
follow-up, not silently patched around.

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
`fed-93bfee/test-db-wrapper`, this commit)

- `cargo fmt --check` -- clean.
- `cargo clippy --all-targets --features db_integration -- -D warnings` --
  clean.
- `scripts/test_db.sh --port 8340` (full default run, fresh instance,
  sanitized env, no seed, no network) -- **exit 0**, every one of 17 test
  binaries + lib + doctests green or explicitly skipped by its own gate
  (`gemini_client_integration`, `relationship_smoke` skip on pre-existing
  gates unrelated to this pass; `mcp_integration`/`mcp_protocol` skip the 3
  `ALLOW_NETWORK_EMBED`-gated tests). Cleanup confirmed: `sending TERM to
  surreal pid 92349` -> `confirmed: surreal pid 92349 is not running`.
  `pgrep` after: only pid 42064.
- Refusal, re-proven after all changes: `SURR_TEST_DB_URL=127.0.0.1:8000
  scripts/test_db.sh` -> exit 3, `pgrep -fl "surreal start memory"`
  identical (only pid 42064) before and after.

`pid 42064` was never reused, signaled, or touched across any run in
either the previous or this pass.
