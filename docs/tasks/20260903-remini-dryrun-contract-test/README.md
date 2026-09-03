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
