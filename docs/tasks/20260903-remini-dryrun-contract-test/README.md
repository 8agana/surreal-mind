# REMini DRY_RUN contract test (fed-734b8f step 0.5)

`scripts/test_dryrun_contract.sh` proves the DRY_RUN safety contract that
plan `fed-734b8f` (remove-call-tools) depends on before any deletion work
in step 2+ can proceed: `remini --all --dry-run` must make **zero provider
invocations**, cause **zero data/schema mutation**, terminate within a
bounded wall clock, and write **only** the explicitly selected report
artifact.

## What it does

1. Starts its own throwaway in-memory SurrealDB (`surreal start memory`,
   default `127.0.0.1:8100`) and tears it down on exit via a trap — it never
   touches the production database (`surreal_mind`/`consciousness`).
2. Applies `scripts/dryrun_contract/schema.surql` (a copy of the DDL in
   `src/server/schema.rs`'s `initialize_schema()`, plus a test-only
   `FLEXIBLE` override on `correction_events.new_state`/`previous_state` —
   see the comment at the bottom of that file for why: this SurrealDB
   version enforces nested-object keys on a plain `TYPE object` field,
   which the schema.rs comment did not anticipate) and seeds it via
   `scripts/dryrun_contract/seed.surql` — unextracted thoughts, entities
   missing embeddings, a `marked_for = 'gemini'` mark for `gem_rethink`, and
   a pending `correction_events` row for `kg_consolidate` to merge.
3. Installs a fake Antigravity CLI stub (`fake-agy`) that never calls a real
   provider: it appends one line per invocation to a calls file and prints a
   canned, schema-valid extraction response.
4. **POSITIVE control**: runs `remini --all --dry-run` and asserts bounded
   exit 0, zero lines in the calls file, an identical before/after DB
   snapshot digest (`scripts/dryrun_contract/snapshot_db.py`, a
   deterministic SHA-256 over every table remini's tasks can touch), that
   only the requested `--report-path` file was written (and the default
   `logs/remini_report.json` was not), and that the report lists all six
   tasks.
5. Reseeds, then runs the **NEGATIVE control**: the same binary *without*
   `--dry-run`, restricted to `--tasks populate,rethink,consolidate`, and
   asserts the fake provider *was* called and the DB snapshot *did* change.
   Without this half a green positive result proves nothing — a check that
   can't fail isn't a check.

## Provider-selection hardening and exact assertions (review finding #168 item 3)

Codex's review of the first cut of this harness (comment #168, item 3) found
it environment-dependent and under-asserting: it installed the fake
Antigravity stub but never forced provider selection, so an operator's
ambient shell could in principle steer a task to the real Gemini CLI; it
didn't instrument the OpenAI path at all; and the positive control checked
only that the report had six `task_details` entries, not that they were the
right six, each with `success: true`. The script now closes each of those:

- **Provider forced, not assumed.** `SM_AGENT_PROVIDER=antigravity` is
  exported before either control runs. Per the crate's precedence chain
  (`src/clients/google_cli.rs:11-17`: `SM_AGENT_PROVIDER` >
  `GOOGLE_CLI_PROVIDER` > `SURR_GOOGLE_CLI_PROVIDER` > config > default), this
  wins regardless of what the operator's environment or `Config` already
  holds — the harness no longer depends on the default happening to be
  Antigravity.
- **Gemini explicitly neutralized.** `kg_populate`/`kg_wander`'s Gemini
  client shells out via a bare `Command::new("gemini")` looked up on `PATH`
  (`src/clients/gemini.rs:291`), with no env var to redirect it the way
  `ANTIGRAVITY_CLI_BIN` redirects Antigravity. The harness now writes a fake
  `gemini` executable into a scratch `fakebin/` directory and prepends it to
  `PATH`, so any Gemini invocation — intended or a regression — hits a stub
  that logs the call and exits non-zero (17) instead of reaching a real
  install. Both controls assert its calls file stays empty; the negative
  control's live provider call is confirmed to land on the fake Antigravity
  stub instead.
- **OpenAI instrumented as far as the crate allows.** `create_embedder`'s
  HTTP endpoint (`src/embeddings.rs:155`) is hardcoded to
  `https://api.openai.com/v1/embeddings` with no `OPENAI_BASE_URL` or
  equivalent override, so redirecting it to a local counting stub isn't
  possible without touching crate code — out of scope here and folded into
  the existing "no offline embedder" deferral. The harness does assert
  `OPENAI_API_KEY` is an obviously-fake value, so if the `embed` task's
  `DRY_RUN` guard (which constructs the embedder but never calls `.embed()`)
  were ever bypassed by a regression, the resulting call would 401 rather
  than silently succeed against a real key.
- **Exact positive assertion.** The report-shape check no longer stops at
  "six entries" — it parses `task_details` down to `{name, success}`, sorts
  it, and compares it byte-for-byte against the six canonical task names
  from `src/bin/remini.rs:147-152` (`consolidate`, `embed`, `health`,
  `populate`, `rethink`, `wander`), each expected `success: true`. A wrong
  task name or a silent `success: false` now fails the run instead of
  passing because the count happened to still be six.

## Why `embed`, `wander`, and `health` are excluded from the negative control

- `embed`: `kg_embed`'s embedding provider is OpenAI, called directly with
  no fake-stub layer this repo owns. Running it live would hit a real
  third-party endpoint, which the operating constraints for this test
  explicitly forbid. Its `--dry-run` path (embedder constructed but
  `.embed()` never called) is still exercised and asserted in the positive
  control, where all six tasks including `embed` report `success: true`.
- `wander`: its live path needs a running `SurrealMindServer` and enough
  graph structure to jump around; that's real weight for no additional
  proof once `populate` has already demonstrated a live provider call and a
  live DB mutation.
- `health`: `scripts/sm_health.sh` shells out to `surreal sql` directly and
  defaults to the *production* endpoint/namespace if `SURR_DB_URL`/`SURR_DB_NS`/
  `SURR_DB_DB` aren't exported — the script exports them, so it would have
  been safe, but there's nothing this task needs from it that `consolidate`
  doesn't already prove (a real, non-dry-run DB write).

## Independently discovered while seeding

Seeding `correction_events` with a nested `new_state` object
(`{ mode: "merge_alias", winner_id: ... }`) against this SurrealDB version
(3.2.3) fails with `"Found field 'new_state.mode', but no such field exists
for table 'correction_events'"` unless the field is declared `FLEXIBLE`.
`schema.rs` declares it as a plain `TYPE object`. `gem_rethink.rs` and
`kg_consolidate.rs` write exactly this kind of nested object to
`new_state`/`previous_state` in production via bound query parameters. This
may or may not reproduce against whatever SurrealDB server version
production actually runs — not verified here, out of scope for this step,
and **not fixed** (this note applies the `FLEXIBLE` override only inside the
disposable test schema copy, never to `schema.rs` itself). Worth its own
ticket if production's SurrealDB version is 3.2.x or later.

## Running it

```
scripts/test_dryrun_contract.sh
```

Builds the release binaries first if `target/release/remini` doesn't exist.
Exits 0 iff every assertion passes; each assertion prints `PASS`/`FAIL`.
Requires `surreal` (checks `PATH`, falls back to `/opt/homebrew/bin/surreal`),
`python3`, `jq`, and `curl`.
