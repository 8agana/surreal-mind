# Standing ephemeral test-DB wrapper for `db_integration` tests (fed-93bfee)

`scripts/test_db.sh` is a standing wrapper around `cargo test --features
db_integration`: it starts (or reuses) a throwaway in-memory SurrealDB
instance on a loopback port, exports exactly the environment the DB-gated
test suite reads, refuses outright if anything resolves to the production
endpoint, applies the shared schema fixture, runs the full `db_integration`
test suite (or a caller-selected subset) with zero external network calls
by default, and tears down only the instance it started itself.

**Never touches production** — `127.0.0.1:8000`, ns `surreal_mind` / db
`consciousness`. See the HARD REFUSAL section below for the mechanism.

## Usage

```
scripts/test_db.sh [--keep] [--dry-run] [--seed] [cargo-test-args...]
```

- `--keep` — leave the ephemeral SurrealDB instance running after the test
  run and print its connection env for manual poking. Only meaningful when
  the script started the instance itself; a reused pre-existing instance is
  never killed regardless of `--keep`.
- `--dry-run` — print the plan (port, reuse/start decision, scratch ns/db,
  fixture plan, the env var names that would be exported, the cargo command
  that would run) and exit 0 without starting anything or running any test.
- `--seed` — ALSO apply `scripts/dryrun_contract/seed.surql` on top of
  `schema.surql` (schema is always applied; seed data is opt-in and NOT the
  default — see "Fixture: schema always, seed opt-in" below for why).
- Anything else is forwarded verbatim to
  `cargo test --features db_integration --no-fail-fast <args>` — e.g.
  `--test reembed_dry_run_contract` to run one integration test binary, or a
  substring filter to target one test function.

`ALLOW_NETWORK_EMBED` (env var, read from the calling environment, **never
set by this wrapper itself**) — three tests reach `api.openai.com` for a
real (bound-to-fail-open, fake-key) embedding call unless this is set. Left
unset by default so this wrapper's contract is genuinely zero external
calls; see "Network contract: zero external calls by default" below.

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
   Verified live throughout this task's two passes against the same
   pre-existing throwaway instance (pid `42064`, `surreal start memory
   --bind 127.0.0.1:8100 --user root --pass root`): the script detected it,
   reused it, and left it running and unmodified across every run.
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
4. **Apply `schema.surql` always; `seed.surql` only with `--seed`** — see
   the dedicated section below for why the split exists.
5. **Export the env contract** (full file:line sourcing below) and run
   `cargo test --features db_integration --no-fail-fast <args>`.
   `ALLOW_NETWORK_EMBED` is passed through only if the *caller* already had
   it set; this wrapper never sets it itself.
6. **Clean up**: kill the SurrealDB instance only if this invocation started
   it; always remove the scratch work directory.

## Fixture: schema always, seed opt-in

The first version of this wrapper applied both `schema.surql` and
`seed.surql` unconditionally. That broke a product test:
`tests/dimension_hygiene.rs::test_reembed_mismatch_reporting` runs its own
`SELECT embedding_dim as dim, count() as count FROM thoughts WHERE
embedding_dim != $expected GROUP BY embedding_dim` (no `ORDER BY`) and reads
`stats[0]` expecting the one row it deliberately inserted with a mismatched
`embedding_dim`. `seed.surql` creates `thoughts:t1`–`t4` with no
`embedding_dim` field at all, so with the seed present the query returns
**two** groups:
```
[{ count: 4, dim: NONE }, { count: 1, dim: 1636 }]
```
(reproduced directly against the scratch namespace with `surreal sql`).
`stats[0]` picks up the `NONE` group instead of the deliberate one, and
`.as_i64().unwrap()` (`tests/dimension_hygiene.rs:263`) panics. This is a
fixture/test **mismatch**, not a defect in the mismatch-reporting logic
itself (the query correctly finds both groups) — but a wrapper whose
default run breaks a product test on every invocation is not a usable
standing wrapper.

Fix: `schema.surql` is still applied on every run (nothing in the
`cargo test` suite needs it pre-applied either — `SurrealMindServer::new()`
calls its own `initialize_schema()`, src/server/schema.rs:19 — but applying
it is harmless and matches the original design). `seed.surql` is applied
**only** with `--seed`. Nothing in the `cargo test` suite needs seed data:
`scripts/test_dryrun_contract.sh` and `scripts/dryrun_contract/
test_health_dryrun.sh` (the fed-734b8f harnesses this fixture was written
for) seed themselves independently via their own `reseed()`/`apply_sql_file`
calls and are unaffected either way. `--seed` exists for manually poking at
seeded data (typically paired with `--keep`).

**Verified both ways** (2026-09-04/05, same instance, pid 42064):
- Default (no `--seed`): `scripts/test_db.sh --test dimension_hygiene` →
  **8 passed, 0 failed**.
- `--seed`: `scripts/test_db.sh --seed --test dimension_hygiene` →
  **7 passed, 1 FAILED** — `test_reembed_mismatch_reporting` panics at
  `tests/dimension_hygiene.rs:263:57`, the exact same NONE-group mismatch
  described above. This is the *expected* result of asking for seed data in
  the same run as this particular test; it is a fixture/test mismatch to
  note (done, here and in the originating clu ticket), not to patch in
  `tests/dimension_hygiene.rs` — out of scope for this wrapper.

## Network contract: zero external calls by default

Grepped the whole `tests/` tree for every path that constructs the real
`OpenAIEmbedder` and calls `.embed(` without an injected mock (`grep -rn
'\.embed(' tests/` finds no direct call outside a non-Rust data file;
the actual paths are all *indirect*, through handler functions). Under this
wrapper's env (`RUN_DB_TESTS=1` set, everything else left at whatever
gates each test individually adds), the full audit is:

| Test | Reaches network under this wrapper? | Why |
|---|---|---|
| `tests/mcp_integration.rs::test_think_handler` | **Yes, unless gated** | calls `handle_legacymind_think` -> `src/tools/thinking.rs:236` `server.embedder.embed(&self.content)` |
| `tests/mcp_integration.rs::test_think_with_continuity` | **Yes, unless gated** | same path |
| `tests/mcp_protocol.rs::test_call_tool_continuity_fallback_protocol` | **Yes, unless gated** | drives a live "think" CallTool through the full protocol harness -> same `embed()` call |
| `tests/dimension_hygiene.rs::test_reembed_mismatch_reporting` | No | calls `create_embedder(&config).await?` for `.dimensions()` only (metadata, no network); never calls `.embed(` |
| `tests/dimension_hygiene.rs::test_run_reembed_actually_updates_matched_row`, `test_kg_embed_actually_updates_matched_row`, `test_reembed_kg_actually_updates_matched_row` | No (already skips) | each gated behind a **second**, explicit `REEMBED_TEST_CONFIRM_DISPOSABLE_NS` env var this wrapper does not set (whole-table-scan safety gate, unrelated to network) — they call the real `run_reembed`/`run_kg_embed`/`run_reembed_kg` which WOULD embed for real, but never run under this wrapper |
| `tests/dimension_hygiene.rs::test_schema_dimension_mismatch_requires_or_honors_emergency_bypass` | No (already skips) | same `REEMBED_TEST_CONFIRM_DISPOSABLE_NS` gate |
| `tests/relationship_smoke.rs::relationship_flow_smoke` | No (already skips) | gated behind `SURR_SMOKE_TEST=1`, unset by this wrapper; calls `handle_knowledgegraph_create`/`handle_unified_search`, both of which do call the real embedder for real when this test runs at all |
| `tests/gemini_client_integration.rs::test_gemini_client_call` | No (already skips) | gated behind `RUN_GEMINI_TESTS=1`, unset by this wrapper; different provider (Gemini CLI, not the OpenAI embedder) anyway |
| everything else in `tests/` | No | no embedder path reached (schema/roster/protocol-metadata/db-CRUD checks only) |

The three "Yes, unless gated" tests are now gated identically: at the top of
each, `if std::env::var("ALLOW_NETWORK_EMBED").is_err() { eprintln!(
"skipped: reaches api.openai.com; needs the offline embedder (clu
fed-77afac)"); return; }`. This wrapper never sets `ALLOW_NETWORK_EMBED`
itself, so under its default env these three tests print the skip message
and pass trivially, and the wrapper's own contract (zero external calls) is
enforced by construction, not by luck.

Before this fix, all three "passed" anyway — not because they avoided the
network, but because `embed_strict` defaults to `false`
(`src/config.rs:149`), so the `think` handler swallows an embedding failure
(logs a `tracing::warn!` and returns a `"pending"` status) rather than
propagating it, and the tests only assert `result.is_ok()` / a successful
protocol response. A fake key made a real, bound-to-fail HTTP request to
`api.openai.com` on every run of this wrapper before the gate existed.

**Proof the underlying call is real, not tcpdump but decisive:** with
`ALLOW_NETWORK_EMBED=1` set (opting back into the gated path) and no other
change, `scripts/test_db.sh --test mcp_integration -- --test-threads=1
test_think_handler` takes ~2.6–3.0s (three retries' fixed exponential
backoff — `embed_retries=3` from `surreal_mind.toml` — plus real round
trips to `api.openai.com` that come back 401 fairly quickly). Re-running
with `HTTPS_PROXY=http://10.255.255.1:1` also exported (a non-routable
black-hole address, so any real outbound TCP connect attempt blocks instead
of failing instantly) — same test, same `ALLOW_NETWORK_EMBED=1` — jumped to
**62.60s**: almost exactly three attempts each hitting the reqwest client's
own 20s request timeout (`src/embeddings.rs:90`) instead of getting a fast
401. (An initial attempt to also set `SURR_EMBED_RETRIES=1` to bound this
experiment tighter had no effect — that env var is not read on this code
path; retries come from `config.system.embed_retries`, sourced from
`surreal_mind.toml`, not an env override. Noted for accuracy, not acted on.)
A ~24x slowdown that lands almost exactly on `retries × client_timeout`
when the proxy target is unroutable, versus normal fast-401 timing when it
isn't, is the network attempt actually happening — `reqwest::Client::builder()`
in `src/embeddings.rs` never calls `.no_proxy()`, so it honors
`HTTPS_PROXY`/`HTTP_PROXY` by default, and this experiment is exactly that
default behavior firing.

## Env contract (file:line)

| Var | Source | Purpose |
|---|---|---|
| `SURR_DB_URL` | `src/config.rs:254` (env override in `Config::load`) | DB endpoint `SurrealMindServer::new` (`src/server/db.rs:29`) connects to |
| `SURR_DB_NS` | `src/config.rs:257` | namespace |
| `SURR_DB_DB` | `src/config.rs:260` | database |
| `SURR_DB_USER` | `src/config.rs:425` (default `root`) | auth |
| `SURR_DB_PASS` | `src/config.rs:426` (default `root`) | auth |
| `RUN_DB_TESTS=1` | runtime skip-gate checked by every DB-backed test: `tests/mcp_protocol.rs:223,336,398,492,783`, `tests/mcp_integration.rs:20,61,75,100,141,209`, `tests/test_wander.rs:16,47`, `tests/stdio_smoke.rs:66`, `tests/dimension_hygiene.rs:39,105,186,292,384,498,583`, `tests/reembed_dry_run_contract.rs:72`, `tests/reembed_bin_dry_run.rs:67`, `src/tools/knowledge_graph.rs:523`, `src/maintenance/reembed.rs:1918`, `src/http.rs:593` (line numbers current as of this pass — the mcp_protocol.rs/mcp_integration.rs numbers moved slightly from the first pass's README because the new ALLOW_NETWORK_EMBED gate blocks were inserted above some of them) | without it every one of these prints a skip notice and returns `Ok(())`/passes trivially — the crate's default `cargo test` stays green and never touches a socket |
| `SURR_TEST_DB_URL` | `tests/reembed_dry_run_contract.rs:76`, `tests/reembed_bin_dry_run.rs:71` | these two files build their **own** scratch namespace internally (`disposable_db()` / `test_db_url()`) rather than using `SURR_DB_NS`/`SURR_DB_DB`, and independently refuse any URL containing `:8000` |
| `OPENAI_API_KEY` | `src/config.rs:427`, consumed by `src/embeddings.rs:246-257` (`create_embedder`/`is_placeholder`) | set to `sk-fake-testdb` — never empty, never the literal `changeme` (which `is_placeholder` treats as *accepted*, not rejected) |
| `GEMINI_API_KEY` | not read anywhere in `src/` (verified: `grep -rn '"GEMINI_API_KEY"' src/` is empty) | set defensively (`fake-testdb`) per the task brief in case a future test path starts reading it; currently a no-op |
| `SM_AGENT_PROVIDER=antigravity` | `src/clients/google_cli.rs:11-17` (top of precedence chain) | forces Google CLI provider selection away from the operator's ambient shell, mirroring `scripts/test_dryrun_contract.sh` |
| `ANTIGRAVITY_CLI_BIN` | `src/clients/antigravity.rs:79` | points at a fake stub this wrapper writes into its scratch work dir (same shape as `scripts/test_dryrun_contract.sh`'s `fake-agy`) — a canned JSON response, never a real CLI |
| `ALLOW_NETWORK_EMBED` | new gates added this pass in `tests/mcp_integration.rs` (`test_think_handler`, `test_think_with_continuity`) and `tests/mcp_protocol.rs` (`test_call_tool_continuity_fallback_protocol`) | **never set by this wrapper**; passed through only if the caller already had it set. See "Network contract" above |

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
contract — it reuses the same schema fixture and the same fake-Antigravity-
stub pattern those two scripts established, so a `surreal start memory`
instance and a `fake-agy` stub are never invented twice, but seeds data
only on request (`--seed`) since the two bash harnesses already seed
themselves and a shared, always-on seed step is what broke
`dimension_hygiene.rs` (see above).

## Proof run (2026-09-04/05, Studio, worktree `surreal-mind-wt-fed93bfee` @
branch `fed-93bfee/test-db-wrapper`, commit on top of `e94ecd2`)

All runs below were against the live pre-existing instance (`pid 42064`,
port 8100) — the script detected and reused it every time, per the
port-selection logic above, and it was still running, untouched, at the
end.

**Rust-change gates:**
- `cargo fmt --check` → clean after `cargo fmt` (3 files reformatted:
  `scripts/test_db.sh` is bash and untouched by this; `tests/
  mcp_integration.rs` and `tests/mcp_protocol.rs`'s new gate blocks needed
  one `eprintln!` line each reflowed).
- `cargo clippy --all-targets --features db_integration -- -D warnings` →
  clean, exit 0.

**Fixture opt-in (item 1):**
- `scripts/test_db.sh --test dimension_hygiene` (default, no `--seed`) →
  exit 0, **8 passed, 0 failed**.
- `scripts/test_db.sh --seed --test dimension_hygiene` → exit 101, **7
  passed, 1 FAILED** (`test_reembed_mismatch_reporting`, same panic as
  before at `tests/dimension_hygiene.rs:263:57` — expected, a fixture/test
  mismatch, not patched here).

**Stale assertion (item 2):** `tests/mcp_integration.rs:35`'s
`assert!(version.starts_with("0.1"))` replaced with `assert_eq!(version,
env!("CARGO_PKG_VERSION"), ...)` — `src/server/router.rs:157` sets
`server_info.version` to exactly `env!("CARGO_PKG_VERSION")`, so an exact
match is correct (not merely `starts_with`).

**Network gate (item 3):** see the dedicated section above for the full
audit and the proxy-based proof. Summary: `mcp_integration` now shows **6
passed** (was 5 passed + 1 stale-version FAILED); `mcp_protocol` still shows
**5 passed** (unchanged count, but `test_call_tool_continuity_fallback_
protocol` now skips instead of silently reaching the network). Confirmed
with `--nocapture` that both `mcp_integration`'s two gated tests and
`mcp_protocol`'s one gated test print `skipped: reaches api.openai.com;
needs the offline embedder (clu fed-77afac)` and that `ALLOW_NETWORK_EMBED=1`
correctly re-enables them (no skip line, same pass/fail outcome, ~3s
elapsed instead of ~0.2s).

**Full wrapper run** (`scripts/test_db.sh`, default: no `--seed`, no
`ALLOW_NETWORK_EMBED`) — per-binary results, all green:

| Test binary | Result |
|---|---|
| `surreal_mind` (lib, 72 tests) | ok |
| `admin`, `kg_apply_from_plan`, `kg_debug_tool`, `kg_dedupe_plan`, `kg_populate`, `migration`, `remini` (bin unit tests) | ok (0 tests each) |
| `gem_rethink` (2), `kg_consolidate` (4), `kg_embed` (1), `kg_wander` (1), `reembed` (3), `reembed_kg` (1) (bin unit tests) | ok |
| `surreal_mind` (`src/main.rs`, 1 test) | ok |
| `dimension_hygiene` | ok (8) |
| `gemini_client_integration` | ok (1, skips before any real call — `RUN_GEMINI_TESTS` unset) |
| `howto_schema_matches_roster` | ok (3) |
| `mcp_integration` | ok (6) |
| `mcp_protocol` | ok (5) |
| `reembed_bin_dry_run` | ok (2) |
| `reembed_dry_run_contract` | ok (4) — **see flake note below** |
| `relationship_smoke` | ok (1, skips — `SURR_SMOKE_TEST` unset) |
| `stdio_smoke` | ok (1) |
| `test_gemini_call` | ok (0 run, 1 ignored) |
| `test_wander` | ok (2) |
| `tool_roster_db_free` | ok (1) |
| `tool_schemas` | ok (7) |
| doctests | ok |

Overall: `cargo test` exit 0, every binary green or explicitly skipped by
its own pre-existing gate.

**Known pre-existing flake, unrelated to this task (`reembed_dry_run_
contract.rs`):** one clean run of the sequence above hit `reembed_kg_dry_
run_makes_no_provider_calls_and_no_writes ... FAILED` with `Error: There
was a problem with the key-value store: Transaction conflict, retry the
transaction`. Reproduced 1/4 more times on immediate retry (`--test
reembed_dry_run_contract` alone), then passed clean the following 2
attempts; running the same file with `-- --test-threads=1` passed clean 4/4
in a row. This is `cargo test`'s default intra-binary test parallelism
racing multiple transactions against the single shared in-memory
`surreal start memory` instance — a pre-existing SurrealDB-engine-level
race in this test file's concurrency assumptions, not something this
wrapper's fixture-opt-in or gating changes introduced (confirmed: the
failure and its single-threaded absence were both observed using the exact
same wrapper invocation, varying only `--test-threads`). Not fixed here —
out of scope; noted for the ticket. Workaround if it recurs:
`scripts/test_db.sh --test reembed_dry_run_contract -- --test-threads=1`.

**Refusal, re-proven after all changes:** `SURR_TEST_DB_URL=127.0.0.1:8000
scripts/test_db.sh` → exit 3, `pgrep -fl "surreal start memory"` identical
(only pid 42064) before and after.
