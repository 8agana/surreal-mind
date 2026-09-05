# Standing ephemeral test-DB wrapper for `db_integration` tests (fed-93bfee)

`scripts/test_db.sh` is a standing wrapper around `cargo test --features
db_integration`: it starts (or reuses) a throwaway in-memory SurrealDB
instance on a loopback port, exports exactly the environment the DB-gated
test suite reads, refuses outright if anything resolves to the production
endpoint, applies the shared fixture, runs the full `db_integration` test
suite (or a caller-selected subset), and tears down only the instance it
started itself. It exists so nobody has to hand-assemble that environment
(or risk pointing it at `127.0.0.1:8000`) to run this crate's DB-backed
tests.

**Never touches production** — `127.0.0.1:8000`, ns `surreal_mind` / db
`consciousness`. See the HARD REFUSAL section below for the mechanism.

## Usage

```
scripts/test_db.sh [--keep] [--dry-run] [cargo-test-args...]
```

- `--keep` — leave the ephemeral SurrealDB instance running after the test
  run and print its connection env for manual poking. Only meaningful when
  the script started the instance itself; a reused pre-existing instance is
  never killed regardless of `--keep`.
- `--dry-run` — print the plan (port, reuse/start decision, scratch ns/db,
  the env var names that would be exported, the cargo command that would
  run) and exit 0 without starting anything or running any test.
- Anything else is forwarded verbatim to
  `cargo test --features db_integration --no-fail-fast <args>` — e.g.
  `--test reembed_dry_run_contract` to run one integration test binary, or a
  substring filter to target one test function.

Every step is logged with the exact command run; nothing is silenced with
`2>/dev/null`.

## What it does

1. **Port selection.** Tries `127.0.0.1:8100` first (override via
   `TEST_DB_PORT`). If something is already listening there:
   - and it's a `surreal start memory` process (matched via `pgrep -f
     "surreal start memory --bind 127.0.0.1:$PORT "`) — **reuse it**, log
     that it's being reused, and never kill it on exit (regardless of
     `--keep`, and regardless of whether *this invocation* started it).
   - otherwise — it's occupied by something unrelated — pick the next free
     port instead (`lsof -nP -iTCP:<port> -sTCP:LISTEN`).
   Verified live against the pre-existing throwaway instance this task's
   brief called out (pid `42064`, `surreal start memory --bind
   127.0.0.1:8100 --user root --pass root`): the script detected it,
   reused it, and left it running and unmodified across every run below.
2. **HARD REFUSAL**, checked *before* anything else runs: if `$SURR_DB_URL`
   or `$SURR_TEST_DB_URL` is already set in the calling environment and
   contains `:8000`, exit 3 immediately. (A second, redundant check runs
   against the URL the script is about to construct itself, which — given
   the port-selection logic above never produces `:8000` — should be
   unreachable, but refuses anyway rather than trusting that invariant
   silently.)
3. **Start or reuse** the SurrealDB instance and poll `surreal is-ready
   --endpoint http://127.0.0.1:$PORT` until it answers (bounded to 20s when
   starting fresh). A freshly started instance is killed in an `EXIT` trap;
   a reused one never is.
4. **Apply the shared fixture** — `scripts/dryrun_contract/schema.surql`
   then `seed.surql` (the same fixture `scripts/test_dryrun_contract.sh` and
   `scripts/dryrun_contract/test_health_dryrun.sh` use) — to a freshly
   generated scratch namespace (`test_fed93bfee_<pid>_<epoch>`) and a fixed
   `scratch` database, via `surreal sql` run from a scratch work directory
   (so its `history.txt` readline log never lands in the repo).
5. **Export the env contract** (full file:line sourcing below) and run
   `cargo test --features db_integration --no-fail-fast <args>`.
6. **Clean up**: kill the SurrealDB instance only if this invocation started
   it; always remove the scratch work directory.

## Env contract (file:line)

| Var | Source | Purpose |
|---|---|---|
| `SURR_DB_URL` | `src/config.rs:254` (env override in `Config::load`) | DB endpoint `SurrealMindServer::new` (`src/server/db.rs:29`) connects to |
| `SURR_DB_NS` | `src/config.rs:257` | namespace |
| `SURR_DB_DB` | `src/config.rs:260` | database |
| `SURR_DB_USER` | `src/config.rs:425` (default `root`) | auth |
| `SURR_DB_PASS` | `src/config.rs:426` (default `root`) | auth |
| `RUN_DB_TESTS=1` | runtime skip-gate checked by every DB-backed test: `tests/mcp_protocol.rs:223,336,398,477,768`, `tests/mcp_integration.rs:20,50,64,89,116,170`, `tests/test_wander.rs:16,47`, `tests/stdio_smoke.rs:66`, `tests/dimension_hygiene.rs:39,105,186,292,384,498,583`, `tests/reembed_dry_run_contract.rs:72`, `tests/reembed_bin_dry_run.rs:67`, `src/tools/knowledge_graph.rs:523`, `src/maintenance/reembed.rs:1918`, `src/http.rs:593` | without it every one of these prints a skip notice and returns `Ok(())`/passes trivially — the crate's default `cargo test` stays green and never touches a socket |
| `SURR_TEST_DB_URL` | `tests/reembed_dry_run_contract.rs:76`, `tests/reembed_bin_dry_run.rs:71` | these two files build their **own** scratch namespace internally (`disposable_db()` / `test_db_url()`) rather than using `SURR_DB_NS`/`SURR_DB_DB`, and independently refuse any URL containing `:8000` |
| `OPENAI_API_KEY` | `src/config.rs:427`, consumed by `src/embeddings.rs:246-257` (`create_embedder`/`is_placeholder`) | set to `sk-fake-testdb` — never empty, never the literal `changeme` (which `is_placeholder` treats as *accepted*, not rejected) |
| `GEMINI_API_KEY` | not read anywhere in `src/` (verified: `grep -rn '"GEMINI_API_KEY"' src/` is empty) | set defensively (`fake-testdb`) per the task brief in case a future test path starts reading it; currently a no-op |
| `SM_AGENT_PROVIDER=antigravity` | `src/clients/google_cli.rs:11-17` (top of precedence chain) | forces Google CLI provider selection away from the operator's ambient shell, mirroring `scripts/test_dryrun_contract.sh` |
| `ANTIGRAVITY_CLI_BIN` | `src/clients/antigravity.rs:79` | points at a fake stub this wrapper writes into its scratch work dir (same shape as `scripts/test_dryrun_contract.sh`'s `fake-agy`) — a canned JSON response, never a real CLI |

`db_integration` feature: `Cargo.toml:71` (`db_integration = []`); most files
above gate on `#![cfg(feature = "db_integration")]` at the top (verified
per-file), the two exceptions being `tests/stdio_smoke.rs` (ungated at
compile time — always compiles, skips at runtime on `RUN_DB_TESTS`) and
`tests/howto_schema_matches_roster.rs` (not DB-gated at all; runs either
way).

## How this relates to fed-734b8f's harness

`scripts/test_dryrun_contract.sh` and `scripts/dryrun_contract/
test_health_dryrun.sh` are narrow, single-purpose bash harnesses: each
starts its own throwaway SurrealDB, seeds the same fixture, and asserts one
specific dry-run safety contract (remini's six tasks; `sm_health.sh`'s
`DRY_RUN` gate) by spawning a compiled binary and grepping its output. This
wrapper is the general-purpose counterpart for the **Rust test suite**
(`cargo test --features db_integration`) rather than one hand-built
contract — it reuses the same fixture files and the same fake-Antigravity-
stub pattern those two scripts established, so a `surreal start memory`
instance and a `fake-agy` stub are never invented twice.

## Proof run (2026-09-04/05, Studio, worktree `surreal-mind-wt-fed93bfee` @
branch `fed-93bfee/test-db-wrapper`)

All four commands below were run against the live pre-existing instance
(`pid 42064`, port 8100) — the script detected and reused it every time,
per the port-selection logic in §1 above.

1. `scripts/test_db.sh --dry-run` → exit 0, plan printed, nothing started.
2. `scripts/test_db.sh` (full run, `cargo test --features db_integration
   --no-fail-fast`) → **exit 101** (2 of 17 test binaries had 1 failing
   test each; every other binary fully green). Full breakdown:

   | Test binary | Result | Note |
   |---|---|---|
   | `surreal_mind` (lib, 72 tests) | ok | |
   | `admin`, `kg_apply_from_plan`, `kg_debug_tool`, `kg_dedupe_plan`, `kg_populate`, `migration`, `remini` (bin unit tests) | ok (0 tests each) | no unit tests defined |
   | `gem_rethink` (2), `kg_consolidate` (4), `kg_embed` (1), `kg_wander` (1), `reembed` (3), `reembed_kg` (1) (bin unit tests) | ok | |
   | `surreal_mind` (`src/main.rs`, 1 test) | ok | |
   | **`dimension_hygiene`** | **7 passed, 1 FAILED** | `test_reembed_mismatch_reporting` — see "Known friction" below |
   | `gemini_client_integration` | ok (1) | `RUN_GEMINI_TESTS` unset → skips before any real call |
   | `howto_schema_matches_roster` | ok (3) | not DB-gated |
   | **`mcp_integration`** | **5 passed, 1 FAILED** | `test_server_initialization` — see "Known friction" below |
   | `mcp_protocol` | ok (5) | |
   | `reembed_bin_dry_run` | ok (2) | |
   | `reembed_dry_run_contract` | ok (4) | |
   | `relationship_smoke` | ok (1) | |
   | `stdio_smoke` | ok (1) | |
   | `test_gemini_call` | ok (0 run, 1 ignored) | |
   | `test_wander` | ok (2) | |
   | `tool_roster_db_free` | ok (1) | |
   | `tool_schemas` | ok (7) | |

   `cargo test`'s default fail-fast would have stopped at the first failing
   binary (`dimension_hygiene`) and never run the other 15 — this wrapper
   passes `--no-fail-fast` specifically so a single failure doesn't hide
   the rest of the suite's result.
3. `scripts/test_db.sh --test reembed_dry_run_contract` → **exit 0**, all 4
   tests pass in isolation (confirms the DB-integration contract test this
   task's brief highlighted works standalone, independent of the two
   failures above).
4. Refusal proof: `SURR_TEST_DB_URL=127.0.0.1:8000 scripts/test_db.sh` →
   **exit 3**, `pgrep -fl "surreal start memory"` identical before and
   after (only pid 42064, untouched) — confirmed nothing was started.

## Known friction (2 pre-existing test failures, neither caused by the
wrapper's core function, both reproduced and root-caused)

**`dimension_hygiene::test_reembed_mismatch_reporting`** — caused by this
wrapper's own step 4 (fixture seeding), not a product bug. `seed.surql`
creates `thoughts:t1`–`t4` with no `embedding_dim` field. The test's own
query (`SELECT embedding_dim as dim, count() as count FROM thoughts WHERE
embedding_dim != $expected GROUP BY embedding_dim`, `tests/
dimension_hygiene.rs:251`) has no `ORDER BY`, so with the seed data present
it returns **two** groups instead of the one the test assumes:
```
[{ count: 4, dim: NONE }, { count: 1, dim: 1636 }]
```
(reproduced directly against the exact scratch namespace from the failing
run: `surreal sql --namespace test_fed93bfee_82738_1788574350 --database
scratch` with that same query). The test reads `stats[0]` and calls
`.as_i64().unwrap()` (`tests/dimension_hygiene.rs:263`) expecting the
deliberately-mismatched row; it gets the `NONE` group instead and panics.
This is a **fixture/test coupling issue introduced by seeding `thoughts`
into the same scratch db this test also uses**, not a defect in the
re-embed mismatch-reporting logic itself — the query correctly identifies
both groups. Not fixed here (out of scope: fixing `tests/
dimension_hygiene.rs` was not part of this task; this wrapper is asked to
apply the shared fixture, which is what actually causes this).

**`mcp_integration::test_server_initialization`** — pre-existing, unrelated
to this wrapper. `tests/mcp_integration.rs:35` asserts
`version.starts_with("0.1")`; `Cargo.toml:3` has `version = "0.8.2"`. This
is a stale version-drift assertion that would fail identically under any
correctly configured `RUN_DB_TESTS=1` run, wrapper or not.

## Live-OpenAI-embedder tests: not skipped — degrade gracefully instead

The task brief anticipated tests needing a live embedder would
skip/fail-closed. In practice **none did either**: `mcp_integration.rs`'s
`test_think_handler` and `test_think_with_continuity` both call
`handle_legacymind_think`, which does attempt a real embedding call against
`api.openai.com` with the fake key (no test double stubs this path —
`create_embedder` performs no network I/O at construction, only the actual
`.embed()` call reaches the network) and gets a real failure back from the
real endpoint. But `Config::default()`'s `embed_strict` is `false`
(`src/config.rs:149`), so the `think` handler tolerates an embedding
failure rather than propagating it, and both tests only assert
`result.is_ok()` — so they pass regardless of whether the embed call
actually succeeded. The one test that *does* avoid a real call entirely is
`reembed_dry_run_contract.rs`, which injects a `CountingEmbedder` mock
(zero network calls, by construction) rather than the real `OpenAIEmbedder`.
This mirrors `scripts/test_dryrun_contract.sh`'s own documented tradeoff
(same file, its `OPENAI_API_KEY` comment): the real OpenAI endpoint has no
override/stub in this crate, so a fake key means "a real network call that
would fail loudly if it succeeded," not "no network call at all."
