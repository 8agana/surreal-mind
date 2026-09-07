## [Unreleased] - fed-6d00e5 decision-runner controls

## [Unreleased] - fed-11a1a0 gardener state advancement

- `kg_wander` now records each successful relationship/entity/observation
  mutation outcome truthfully (`kind`, `id`, `created`), including idempotent
  `created:false` responses. It then performs one semantic traversal from the
  current node, with random fallback, before the next decision prompt. The
  prompt carries a bounded recent-action record; this is state feedback, not a
  claim of semantic novelty or global deduplication.
- Dead-end/exhausted traversal falls back to random; an empty graph ends the
  runner cleanly with an explicit message. Transport/DB failure remains an
  error if its random fallback also fails, preventing further mutations against
  stale context. No live model or database calls were made for this change.

- Normalized the Python decision runner's model forwarding to match the Rust
  Antigravity client: empty or case-insensitive `auto` omits `--model`; an
  explicit trimmed model is forwarded. Offline fake-CLI argv coverage proves
  both paths. The recorded standalone acceptance attempt failed before this
  correction because it forwarded `--model auto` alongside `--effort low`.

- Forward the selected KG model through the opt-in Rust/Python runner to
  Antigravity's `--model`; offline fixtures assert both argument boundaries.

- Added an opt-in Rust adapter through `KG_WANDER_DECISION_RUNNER` (absolute
  script path). Sends context over stdin to the Python runner, consumes exact
  JSON, and propagates runner failure instead of falling back to random wander.
  Existing runtime selection is unchanged unless explicitly enabled. Pending
  review and deployment; this does not yet repair nightly execution.

- Added independent Rust/Python process-group supervision for the opt-in
  runner. Rust bounds the adapter deadline and combined output while it waits,
  then kills/reaps its shared Python/`agy` group on every result path; standalone
  Python retains its existing ownership model.

- Added offline subprocess controls for the candidate subscription decision runner:
  valid terminal output, nonzero child exit, oversized stdout, and timeout of
  a TERM-ignoring child. Each verifies child reaping and disposable workspace
  removal, including descendant cleanup. No provider or KG call.
  Runner remains unintegrated and undeployed; nightly wander is not fixed yet.

## [Unreleased] - remove call_status/call_jobs/call_cancel/call_cc/call_vibe/call_gem tools (branch `fed-734b8f/remove-call-tools`)

Removed the six delegation/job-management MCP tools that duplicated
federation delegation already covered elsewhere (`comm`, direct CLI
invocation): `call_status`, `call_jobs`, `call_cancel` (with
`registry.rs`, the job semaphore, and the `agent_jobs` DDL/dashmap),
`call_cc` (and `ClaudeClient`), `call_vibe` (and `VibeClient`), and
`call_gem` (the wrapper only — `AntigravityClient`, `GeminiClient`,
`GoogleCliProvider`, and their shared `CognitiveAgent` trait survive,
since `kg_populate` and `kg_wander` still depend on them directly;
`src/clients/antigravity.rs` itself lost the `call_gem`-only
`AntigravityPermissionMode::for_call_gem()` constructor and its
`ANTIGRAVITY_CALL_GEM_PERMISSION_MODE` env var, plus a doc-comment
update — a 1-insertion/9-deletion trim, not an untouched file). Also
removed the now-unreferenced `workspace.rs` /
`WorkspaceMap` / `WORKSPACE_*` config, whose only production consumers
were the three deleted delegation tools.

**Public tool count: 16 → 10.** Surviving roster, in registration order:
`think`, `wander`, `maintain`, `journal`, `rethink`, `corrections`,
`test_notification`, `remember`, `howto`, `search`. All six removed
names now return `METHOD_NOT_FOUND` at the router boundary.

**Unaffected:** REMini (`remini`), `kg_populate`, `kg_wander`, and the
Antigravity/Gemini/GoogleCli client stack — none of these ever routed
through the removed MCP tool handlers; they call the clients directly.

Also added, ahead of the removal itself, as their own preceding
commits: a genuine bounded `DRY_RUN` no-op path for `kg_wander`,
`kg_populate`, `kg_embed`/`reembed`, and REMini's health task (zero
provider/DB-write calls under dry-run, verified by a fake-provider
contract test asserting zero calls into
`ANTIGRAVITY_CLI_BIN`), and a `--report-path`/`REMINI_REPORT_PATH`
option on `remini` for capturing dry-run baselines before a
change like this one.

### Repair pass — Codex review #168 (2026-09-03)

Independent review of the deploy at `fba5ac2` returned BLOCK. Items 1–5 are
repaired in this tree and are described below. Item 7 (the deploy backup's
`ROLLBACK.md`, which prescribed a destructive `git reset --hard` against a
production checkout that had uncommitted work) was rewritten in place under
`surreal-mind-backups/20260903-fed734b8f/`, outside git, and so leaves no
commit here. No other finding was addressed in this pass.

- **Item 1 — `scripts/sm_health.sh` ignored `DRY_RUN`.** `maintain(subcommand=health,
  dry_run=true)` spawns the script with `DRY_RUN=1`, but the script issued its
  stale-entity `UPDATE` regardless. Added a truthiness-matched guard
  (`1|true|TRUE|True|yes|YES|on|ON`, matching the crate's `bool_env` helpers)
  that exits 0 before any `surreal sql` call, plus
  `scripts/dryrun_contract/test_health_dryrun.sh` with a positive and a
  negative control. REMini's own health shortcut in `src/bin/remini.rs` was
  already safe — it intercepts `dry_run` in Rust and never invokes the script.
- **Item 2 — both re-embed paths called the provider under dry-run.**
  `run_reembed_kg` embedded every candidate row and only guarded the write, and
  `src/bin/reembed.rs` read no dry-run flag at all. Split `run_reembed_kg` into
  a thin env-resolving wrapper plus an injectable `run_reembed_kg_with` core,
  and added a dry-run branch immediately before each of the three
  `embedder.embed()` calls; the pre-existing `if !dry_run` write guards are kept
  as a second line of defence. `src/bin/reembed.rs` now honours both
  `--dry-run` and `DRY_RUN` via a shared pure classifier. Covered by
  `tests/reembed_dry_run_contract.rs` (counting embedder, before/after snapshot,
  live negative control), which fails against the pre-fix code.
- **Item 3 — the contract harness relied on the provider default rather than
  forcing it.** `scripts/test_dryrun_contract.sh` now forces
  `SM_AGENT_PROVIDER=antigravity` at the top of the crate's precedence chain,
  PATH-shadows a fake `gemini` stub that logs and exits non-zero (Gemini has no
  env redirect the way `ANTIGRAVITY_CLI_BIN` does), asserts both fake-CLI call
  logs are empty, asserts `OPENAI_API_KEY` is an obviously-fake value, and
  replaces the count-only check with an exact comparison against the six
  canonical task names each with `success:true`.
- **Item 4 — the dry-run fixture resurrected a removed table.**
  `scripts/dryrun_contract/schema.surql` carried a 28-line `agent_jobs` DDL
  block for a table this branch deleted. Removed it and dropped `agent_jobs`
  from `snapshot_db.py`'s `TABLES`; a statement-by-statement diff against
  `initialize_schema()` confirmed it was the only table-level discrepancy. The
  task README's "a copy of the DDL in schema.rs" claim is now anchored to
  `fba5ac2` and names the two deliberate differences that remain.
- **Item 5 — documentation left stale by the removal.** `README.md`'s
  "Tool Surface (16)" heading, `docs/AGENTS/arch.md`'s Agent Jobs line, and the
  never-implemented `docs/tasks/workspace-alias-resolution.md` (now banner-
  fenced, body preserved) were corrected, along with this entry's own
  overstated claim that the client stack survived "untouched" —
  `src/clients/antigravity.rs` lost 9 lines. `src/tools/howto.rs` gained the
  missing `test_notification` help arm.

**Deferrals carried forward, as labelled by the repairing workers:**

- No offline embedding provider exists, so nothing that requires constructing
  `SurrealMindServer` was exercised end to end; item 1 was verified by invoking
  the script directly under the identical env contract `handle_spawn_script`
  uses. `src/embeddings.rs` hardcodes the OpenAI endpoint with no base-URL
  override, so item 3's OpenAI leg is a fake-key assertion rather than a
  counting stub.
- No whole-suite `RUN_DB_TESTS=1 --features db_integration` run: the worktree
  `.env` points at production (`127.0.0.1:8000`, `surreal_mind`/`consciousness`),
  so DB runs were scoped to the new contract test against the throwaway
  `127.0.0.1:8100` instance, which additionally refuses any URL containing
  `:8000`.
- `run_reembed` (thoughts, HTTP-client based) and `run_kg_embed` were already
  correct and were left untouched; they have no counting-embedder test because
  they build their own embedder from config. `src/bin/reembed.rs`'s core loop
  now runs through an injectable library function,
  `run_reembed_standalone_with` (`src/maintenance/reembed.rs`), the same
  `_with` pattern `run_reembed_kg_with` established — covered by a
  counting-mock-embedder + disposable-DB dry-run/live-run pair in
  `tests/reembed_dry_run_contract.rs`. The binary itself keeps two subprocess
  spawn tests in `tests/reembed_bin_dry_run.rs`, covering only the
  `--dry-run`/`DRY_RUN` guard end to end (arg parsing, config load, exit code,
  stdout contract). No test in this suite contacts OpenAI or any other
  external provider — the subprocess-level negative control that used to
  reach `api.openai.com` with a fake key was replaced by the in-process one
  above, run against the counting mock instead of live production
  infrastructure.
- Pre-existing and out of scope: `scripts/sm_health.sh`'s `duration::days(90)`
  type mismatch against SurrealDB 3.2.x (reproduced against the pre-fix script),
  and `run_kg_embed`'s `&text[..min(60)]` log truncation, which can panic on a
  UTF-8 boundary.

## [Unreleased] - REMini scheduler deduplication

- **Single 01:00 owner:** `dev.legacymind.nightly-shift` remains the canonical
  scheduler for `remini --all`; the duplicate standalone
  `dev.legacymind.remini` LaunchAgent was unloaded and its installed plist was
  preserved as `.plist.disabled` under `fed-74ee30`.
- The repository's standalone plist remains available for explicit rollback,
  with an inline warning against installing both jobs concurrently. Immediate
  topology checks passed; the next 01:00 fire is the functional single-run
  witness. Evidence and rollback: `docs/tasks/20260903-remini-scheduler-dedupe/`.

## [Unreleased] - rmcp 3.1.4 postdeploy follow-up (branch `codex/rmcp-3.1.4-followup`)

Isolated worktree `surreal-mind-rmcp-3.1.4-followup`, based on the deployed branch commit `13943cb` with the corrected review imported from `f1dbf2f`. Implements the CONFIRMED findings from `docs/tasks/20260823-rmcp-3.1.4-upgrade/rmcp-3.1.4-postdeploy-delta-review-cc.md` Revision 2 (M-1, N-1, N-2, N-5), plus extensions this pass derived directly from that same document's own text (deterministic selection from the N-2 discussion; `src/bin/reembed.rs` false-success accounting from the N-5 "same false-success class" note; `src/bin/admin.rs`'s sibling N-1 site from the N-1 finding's own dual-site anchor), the N-4 protocol test-coverage gap, and this pass's own CLI `--version` provenance design (no separate design document — see `build.rs`'s inline doc comments). All work done over `ssh studio` against this worktree only; the live Studio worktree and production binary/launchd/port 8787/tunnel/DB were never touched. No production install performed — this is a source candidate with recorded evidence, per this task's mandate.

**Master integration reconciliation (2026-09-01):** the isolated integration candidate merges committed master review history `4c32b3c` and `f1dbf2f` into the followup lineage, then ports four still-true documentation corrections from the preserved pre-cleanup evidence: the public `think` name, current 16-tool/job names, current embedding configuration ownership, and valid maintenance/smoke commands. Stale line numbers and obsolete whole-file snapshots were deliberately not carried forward.

**Provenance note (added on rerun, correcting this pass's own citation errors):** two earlier commits in this branch cited a "maintenance analysis" document with a numbered "CONFIRMED #7" finding and a `do_not_change` section, and `build.rs`'s doc comment cited `docs/tasks/20260823-rmcp-3.1.4-upgrade/rmcp-3.1.4-version-provenance-design.md`. **Neither document exists anywhere in this repository's history** (confirmed via `git ls-tree -r` against this branch's base commits and a grep across every file in `docs/tasks/20260823-rmcp-3.1.4-upgrade/`). Every decision those citations described was in fact reasoned directly from `rmcp-3.1.4-postdeploy-delta-review-cc.md` Revision 2's own text — see the corrected citations below and in `build.rs`.

**Sol closure correction (2026-08-25):** the initial provenance implementation still treated a tracked-file edit as invisible unless a Git ref changed, accepted an ancestor repository when a source archive was nested inside one, and serialized unknown commit metadata as a bare package version. Those claims are superseded: `build.rs` now watches every tracked file, requires canonical Git top-level equality with `CARGO_MANIFEST_DIR`, and emits explicit `+unknown` / `-dirty-unknown` identities. This remains build metadata only; it is neither reproducible binary hashing nor a deployment assertion. A measured artifact SHA-256 plus an external deployment receipt is the binding witness. See `rmcp-3.1.4-followup-sol-review.md` and `scripts/verify-build-provenance-fixture.sh`.

**N-4 CI wiring (2026-08-25):** CI now runs Clippy with the same locked,
all-features, warnings-as-errors boundary used for the production candidate and
adds a separate disposable SurrealDB 3.2.3 job for the deterministic
database/protocol slice. That job executes the real handler/wire
`tools/list`/SEP-2549 assertion, protocol negotiation, invalid-parameter and
notification paths, stdio smoke, `GROUP ALL` count regression, and stale-index
dimension/bypass regression under `RUN_DB_TESTS=1`. The slice was first run
locally against an in-memory SurrealDB on port 18000: all seven tests passed,
`Cargo.lock` stayed unchanged, and the worktree remained clean. Tests that
actually invoke the external embedding API are intentionally not mislabeled as
CI-covered; they retain their independently recorded disposable-Studio
evidence until a dedicated CI credential is provisioned.

The malformed duplicate `.github/workflows/rust.yml` was removed in the same
closure. Its broken `on:` indentation produced an immediate zero-job failure on
every branch push, while its jobs duplicated `ci.yml`; its rmcp check also ran
`cargo update -p rmcp` immediately before a `--locked` check. `ci.yml` is now the
single workflow owner for this repository's Rust gates.

The first real GitHub run also exposed that `tests/test_agent_job_status.rs`
created/deleted SurrealDB rows while living in the default, database-free test
suite. It is now feature-gated with `db_integration`, skips unless
`RUN_DB_TESTS=1`, and executes as its own named step in the disposable database
job. The default suite no longer depends on an undeclared server, and the three
deserialization regressions remain executable CI coverage rather than being
silently removed.

That run also activated the repository's previously dormant `cargo audit` step
against the production-identical lockfile and reported 15 pre-existing RustSec
advisories. Dependency remediation is tracked separately from this rmcp CI
repair; the audit remains visible on every branch run as a non-blocking warning
instead of either hiding the findings or making unrelated baseline debt report
the rmcp regression suite as failed.

### Fixed (Sol closure)

- **Protocol-shaped list responses:** production tracing proved that rmcp's
  default `resources/list`, `resources/templates/list`, and `prompts/list`
  handlers omitted draft-required cache metadata under `2026-07-28`. The first
  correction narrowed the advertised ceiling to `2025-11-25`; it was rolled
  back after a real Claude Code client proved its inline 2026 requests then
  failed with `-32022`. The replacement keeps both eras supported and uses the
  request context as the type marker: 2026 clients receive `resultType`,
  `ttlMs`, and `cacheScope` on all four list methods, while older session
  clients receive none of those draft-only fields.
- **Generated-vector and update-result correctness:** one shared `ensure_generated_embedding_dimension` guard now runs before every active thought/KG/admin/re-embed vector write. The `think`, `ensure_kg_embedding`, admin, and re-embed paths inspect `RETURN` rows before reporting success; a wrong-length vector, statement error, or zero-row update cannot be represented as completion.
- **N-2 recurrence in the six KG batch writers:** `run_reembed_kg` and `run_kg_embed` now classify transport failure, statement error, zero match, and success independently. Per-record failures continue the batch and are returned separately from `*_no_match` counters.
- **Emergency dimension bypass at the real startup boundary:** `SURR_SKIP_DIM_CHECK` now bypasses only the schema index-dimension check inside `SurrealMindServer::new()`, which occurs before `main`'s existing preflight. Normal startup remains strict. The db-integration test for the actual stale-index/bypass sequence is intentionally gated on a disposable namespace.
- **KG `edges` MCP reporting correction:** a later Sol repair added the complete `edges` block to `maintain(action:"reembed_kg")`, including `updated`, `skipped`, `missing`, `mismatched`, `no_match`, and `failed`. Any earlier statement in this changelog that says the MCP result omits edges is historical and superseded.

### Deferred (Sol closure)

- N-3 real-client trace, N-4 CI hard assertion/wiring, M-2 angle-bracket record-key normalization, and the total-order tie-breaker remain follow-ups. No protocol change, CI push, or production install was made.

### Fixed

- **M-1 (embed_pending never verified embedding dimension before marking `complete`)** — `src/tools/maintenance.rs::handle_embed_pending` now delegates to the shared `ensure_generated_embedding_dimension` guard before attempting its UPDATE; a wrong-length vector is logged and counted as `failed` while the row remains retry-selectable. The later Sol closure applies that same guard to every materially equivalent active writer. **Correction to Revision 2's own "CORRECT FIX" framing** (`rmcp-3.1.4-postdeploy-delta-review-cc.md`, the M-1 finding, "upstream, not post-hoc" section), **discovered live during this pass**: `thoughts_embedding_idx` (`HNSW DIMENSION {dim}`) rejects any write to `embedding` whose array length doesn't match the index's configured dimension outright ("Incorrect vector dimension") — SurrealDB itself was already a hard backstop against this exact write, which is *why* Revision 2 measured 0 wrong-dimension rows live despite the application-level gap. This fix remains correct and worth keeping (it avoids a wasted UPDATE round-trip, produces a clear log line instead of a generic transport error, and doesn't rely on assuming the HNSW index will always exist in exactly this shape), but it is defense-in-depth on top of an existing DB-level constraint, not the sole barrier the original framing implied.
- **N-1 (missing `GROUP ALL` collapses `count()` to 1 per matched row)** — SurrealDB 3.1's zero-arg `count()` returns a hardcoded 1 per matched row unless the query carries `GROUP ALL`; both call sites read `.first()` off the result array, silently truncating any true count &gt; 1 to 1. Fixed at both sites named in the review: `src/tools/maintenance.rs`'s embed_pending remaining-count query (now `... GROUP ALL`) and `src/bin/admin.rs:672`'s dimension-fix verification query (now `... GROUP ALL`). New db_integration regression test `tests/dimension_hygiene.rs::test_group_all_required_for_accurate_count` reproduces the exact predicate shape against 3 synthetic scratch rows and asserts both the pre-fix asymmetry (3× `{cnt:1}`) and the post-fix collapse (1× `{cnt:3}`) directly — **executed live** against a disposable namespace (see Testing evidence below), not merely compiled.
- **N-2 (a per-row statement error aborted the whole embed_pending call via `?`)** — `src/tools/maintenance.rs::handle_embed_pending`: `let updated: Vec<serde_json::Value> = response.take(0)?;` (which propagates a per-statement error, e.g. a SCHEMAFULL type violation, out of the entire tool call) replaced with an explicit `match response.take::<Vec<serde_json::Value>>(0)`, reusing the same log-and-`failed+=1` shape already used for the sibling transport-error arm three lines below. Paired with a new `ORDER BY created_at ASC` on the pending/failed SELECT (`created_at` was already in the SELECT list, satisfying the code's own noted SurrealDB 2.4+ constraint) so a persistently-erroring row sorts to the same position every run instead of reshuffling — bounding worst-case damage to "always reselected in the same spot" rather than "can wedge the whole backlog at zero progress."
- **N-5 (reembed's per-row UPDATE never matched any row)** — `src/maintenance/reembed.rs::run_reembed`: the per-row UPDATE's `WHERE id = '{}'` (a bare string compared against a typed Thing, measured live pre-fix as 0-of-1 rows affected) changed to `WHERE id = type::record('thoughts', '{}')`, and `RETURN NONE` changed to `RETURN meta::id(id) AS id` so the match is observable. `updated` is now only incremented when the UPDATE's own result block is a non-empty array AND the statement's status is `"OK"` (SurrealDB's HTTP `/sql` endpoint can return HTTP 200 with a per-statement error body, previously misread as success); a non-match increments a new `ReembedStats::no_match` counter (surfaced through the `maintain(action:"reembed")` tool's JSON) instead of silently inflating `updated`. New db_integration regression test `tests/dimension_hygiene.rs::test_run_reembed_actually_updates_matched_row` calls the real `run_reembed()` end-to-end against a scratch row and asserts its `embedding_model` actually changed — **verified live to discriminate**: run against the pre-fix bare-string WHERE clause (temporarily reverted, everything else held constant) it fails exactly as expected (`updated: 0, no_match: 1`); run against the fix it passes.
- **`src/bin/reembed.rs` (in-scope per the review's own reembed.rs carve-out)**: the same false-success class as N-5 — `Ok(response) => { success_count += 1; ... }` counted success on any `Ok` from `.query().await`, regardless of whether the UPDATE (already using the correct `type::record('thoughts', $id)` identity) matched a row. Now calls `response.take::<Vec<serde_json::Value>>(0)` and only increments `success_count` on a non-empty result; a 0-row match or a statement-level error increments `error_count` with a distinct log line.
- **Pre-existing broken db_integration test, discovered while adding N-1/N-5 coverage in this pass** — `tests/dimension_hygiene.rs::test_reembed_mismatch_reporting` had been silently no-op-ing on every run via the same N-2 pattern (`server.db.query(query).await?;` never called `.take()`/`.check()`, so a failing CREATE was never surfaced) for three independent reasons, all fixed: (1) `array::range(start, count, step)` (3 args) does not exist in SurrealDB 3.1.2 — the live 2-arg form is `array::range(a, b)`, and (measured live) it returns `b - a` elements, not `b`; (2) `thoughts` is SCHEMAFULL with several non-`option` fields carrying no `DEFAULT` (`injected_memories`, `injection_scale`, `significance`, `access_count`, `created_at`) that a CREATE must now supply explicitly; (3) `thoughts_embedding_idx`'s live HNSW index (see the M-1 note above) rejects a wrong-length `embedding` array outright, so the test's premise was rewritten to set a correct-length `embedding` array with a deliberately wrong `embedding_dim` *metadata* field instead — which is what the test's own `SELECT ... WHERE embedding_dim != $expected` was actually checking for all along.
- **Schema DDL error visibility — implemented differently than Revision 2's "STRONGEST SURVIVING OBJECTION" note proposed (`rmcp-3.1.4-postdeploy-delta-review-cc.md`, the M-1 finding, `schema.rs:60`/`:235` discussion), after a measured regression it did not anticipate.** That note identified that chaining `.check()` onto `initialize_schema()`'s bulk `schema_sql` query would surface a per-statement DDL error (e.g. `thoughts_embedding_idx` redefined at a different DIMENSION), which otherwise rides back inside an `Ok(Response)` and is silently discarded. **Measured live**: none of this schema's DDL statements carry `OVERWRITE`/`IF NOT EXISTS`, so SurrealDB 3.1.2 errors on *every* redefinition — even byte-identical ones — with e.g. `"The table 'thoughts' already exists"`; `initialize_schema()` runs on every `SurrealMindServer::new()`, i.e. every server restart against an already-initialized namespace, so blind `.check()` broke that restart (verified live: adding it caused a second `SurrealMindServer::new()` call in the same disposable namespace to fail hard). Reverted that approach; implemented instead as a narrow, additive post-init read-back: new `SurrealMindServer::verify_embedding_index_dimension(expected_dim)` runs `INFO FOR TABLE thoughts` after schema init, parses `thoughts_embedding_idx`'s live `DIMENSION` out of its DDL text, and returns an explicit error only when it disagrees with the currently configured embedder dimension. This achieves the review's actual stated goal (a stale-dimension index no longer goes unnoticed) without touching DDL error-swallowing semantics that production currently depends on for idempotent restarts, and without adding `OVERWRITE`/`IF NOT EXISTS` to the index DDL (a separate rebuild-cost design decision this pass explicitly deferred — see below).

### Fixed (maintenance/scope-audit rerun — three independent reviews of the above)

Three independent read-only audits (maintenance, protocol, scope/provenance) were run against this branch after the initial implementation pass. Findings were verified against source before any edit; false positives are rejected in this same rerun where identified. All fixes below are the SAME false-success/undercounting bug classes as N-1/N-5 above, recurring at sites those fixes didn't reach.

- **N-5-class false-success in `run_reembed_kg` and `run_kg_embed` (`src/maintenance/reembed.rs`)**: the 9289c72 fix that corrected `run_reembed`'s thoughts-table UPDATE (see N-5 above) was never extended to this same file's KG-side functions, even though the review had already named the bug class. `run_reembed_kg`'s three per-table UPDATE blocks (`kg_entities`, `kg_observations`, `kg_edges`) called `.await?` with no `.take()`/verification and incremented `*_updated` unconditionally; `run_kg_embed`'s idempotent UPDATE variant was strictly worse — it used `RETURN NONE`, giving nothing to verify a zero-row match against even in principle. Both are reachable from live tooling: `run_reembed_kg` via the `maintain(action:"reembed_kg")` MCP tool and the `reembed_kg` CLI binary; `run_kg_embed` via the `kg_embed` CLI binary (Sam's `kgembed` shortcut). Fixed by adding `RETURN meta::id(id) AS id` (replacing `RETURN NONE` where applicable) to all six UPDATE blocks, checking the result is non-empty before incrementing the corresponding `*_updated` counter, and adding a parallel `entities_no_match`/`observations_no_match`/`edges_no_match` counter to `ReembedKgStats` and `KgEmbedStats` (surfaced through the `maintain(action:"reembed_kg")` tool's JSON `entities`/`observations` blocks and both CLI binaries' summary output — `edges` was already absent from the MCP tool's JSON output before this pass and remains so; that gap is pre-existing and out of this pass's scope). New db_integration regression tests `tests/dimension_hygiene.rs::test_kg_embed_actually_updates_matched_row` and `::test_reembed_kg_actually_updates_matched_row` each create one synthetic `kg_entities` scratch row, call the real function end-to-end, and assert both that the embedding was actually written (not just that the counter incremented) and that `entities_no_match == 0` for the freshly-matching row — **executed live** against a disposable namespace (see Testing evidence below).
- **N-1 recurrence in `src/http.rs`'s `get_db_counts_params`** (the `SURR_DB_STATS=1` `/health` endpoint's count path) — flagged by Revision 2 (`src/http.rs:525,530`) and left unfixed in the initial pass as out of the named M-1/N-1/N-2/N-5 scope. Confirmed still broken and now fixed as part of this rerun: the query lacked `GROUP ALL`, and this call site was worse than the sibling sites fixed above — it deserialized straight into `Option<u64>` instead of reading `.first()` off a `Vec<Value>`, so on SurrealDB 3.x it failed outright for ANY table with ≥1 matching row (an explicit "Tried to take only a single result" error for >1 rows, or a failed object-to-u64 cast for exactly 1 row) rather than merely truncating to 1 — `thoughts_count`/`recalls_count` on `/health` silently reported `null` regardless of actual table size. Fixed by adding `GROUP ALL` and switching to the same `Vec<Value>`-then-`.get("c")` pattern used elsewhere in this codebase. **Bug found and fixed live while adding the regression test**: the first attempt wrote `SELECT count() AS c FROM thoughts LIMIT 100000 GROUP ALL`, keeping the pre-fix query's `LIMIT 100000`, but SurrealQL's clause order is `... GROUP ... ORDER BY ... LIMIT ...` — `LIMIT` always applies AFTER aggregation, so `GROUP ALL ... LIMIT n` would be a no-op (only ever 1 output row) and `... LIMIT n GROUP ALL` is a parse error (confirmed live: `"Unexpected token \`GROUP\`, expected Eof"`). Live test run against the first attempt failed exactly this way before the ordering bug was caught; the shipped fix drops `LIMIT` entirely (matching maintenance.rs's/admin.rs's GROUP ALL sites, none of which carry one) and relies on this function's existing 500ms `tokio::time::timeout` as the actual bound on slow-query cost. New db_integration regression test `tests/http.rs`-adjacent `src/http.rs::tests::test_get_db_counts_reports_true_total` (inline, since `get_db_counts_params` is a private function of the `main.rs`-only `http` module and has no `tests/`-crate-visible path) creates 3 synthetic scratch thoughts and asserts the real function returns `Some(n ≥ 3)` — **executed live**, and was the run that caught the `LIMIT`/`GROUP ALL` ordering bug above (see Testing evidence below).
- **Non-blocking findings from the three-audit rerun reviewed and left unfixed, on purpose**: (1) `handle_embed_pending`'s N-2 fix increments a local `failed` counter but never writes `embedding_status = 'failed'` to a persistently-erroring row, so it stays reselected every run rather than being permanently skipped — true, but changes production write behavior (an UPDATE this pass didn't otherwise touch) for a case the ORDER BY fix already bounds to "no worse than reselected in the same spot," so left as a named follow-up rather than an in-pass fix. (2) The N-1 regression test (`test_group_all_required_for_accurate_count`) and this rerun's new tests exercise the real SQL semantics and, for the KG fixes and the http.rs fix, the real functions directly; `admin.rs::fix_dims`'s own GROUP ALL fix still has no test coverage of any kind — flagged, not fixed, since `fix_dims` is an interactive admin CLI path this task's scope did not otherwise touch. (3) The protocol-audit's N-3/PROTO/CI findings (missing `2026-07-28` version-negotiation test coverage, the branch never having reached `origin` so CI has never run against it) are correctly non-blocking and explicitly out of this task's scope, which forbids speculative N-3 changes; none were made.

### Added

- **CLI `--version`/`-V` provenance** (`build.rs`, `src/version.rs`, wired into `src/main.rs` before `Config::load()`): emits explicit clean (`{pkg}+{commit}`), dirty (`-dirty`), dirty-unknown (`-dirty-unknown`), and unknown-commit (`+unknown[-dirty[-unknown]]`) identities. Git metadata is accepted only for a repository rooted exactly at `CARGO_MANIFEST_DIR`; every tracked file is watched because the dirty observation covers every tracked file. A source archive, nested ancestor repository, or unavailable `git` therefore cannot masquerade as a clean bare package version. The bounded fixture verifies clean/dirty/cleared source and non-source edits, unavailable Git, source archives, and nested archives. This is build metadata, not reproducible binary hashing or a deployment receipt.
- **N-4 protocol test coverage** (`tests/mcp_protocol.rs::test_list_tools_protocol`): added assertions that the live JSON-RPC `tools/list` response carries `ttlMs: 300000` and `cacheScope: "public"` on the wire. Closes the specific gap the protocol-analysis N-4 finding named: the router.rs unit test (`list_tools_result_serializes_required_sep2549_fields`) checks these fields but never drives the real `ServerHandler::list_tools` → JSON-RPC serialization path, while this test drives that real path but, until now, never checked these fields — so a revert of `router.rs`'s `list_tools_result()` helper back to `ListToolsResult { tools, ..Default::default() }` would have passed both tests. It now fails this one, reproducing the exact shape of the Claude Code 2.1.241 "tools fetch failed" regression this upgrade originally fixed.

### Deferred / explicitly not implemented

- **N-3 (unadvertised `resources/list`/`resources/templates/list`/`prompts/list` returning empty success with no SEP-2549 cache metadata, instead of `method_not_found`)** — left unimplemented per this task's own gate: the protocol analysis's live reproduction confirmed the *mechanism* (what the server sends back if asked) but explicitly could not establish the *trigger* (whether Claude Code's or Codex's actual MCP client ever calls these methods against a tools-only server) from this sandbox — no client source was available to inspect. Preserved explicitly as unimplemented; **next discriminator**: have a real Claude Code or Codex client connect to an isolated candidate server and check request logs/tracing for whether any of the three methods are ever sent during a normal connect-and-use session. If confirmed, the lowest-blast-radius fix is populating the same `ttlMs`/`cacheScope` metadata on the three empty-default results (mirroring `list_tools_result()`), not narrowing `supported_protocol_versions()` or overriding the methods to `method_not_found` (both carry larger blast radius per the protocol analysis).
- **N-4 CI wiring** (running `tests/mcp_protocol.rs` under `--features db_integration` with `RUN_DB_TESTS=1` against a disposable SurrealDB instance in GitHub Actions) — not implemented in this pass. The protocol analysis's proposed spec (install the `surreal` CLI, background a `memory`-backed instance, point `SURR_DB_*` at it, tear down after) is a real CI-infrastructure change that needs its own verification against an actual Actions run before being trusted; shipping it unverified here would violate this task's "provenance discipline" instruction not to claim a CI pass without an actual run URL. Left as a named follow-up.
- **`thoughts_embedding_idx` `OVERWRITE`/`IF NOT EXISTS`** — explicitly left unchanged this pass; confirmed during this pass to be the correct call (see the schema DDL note above — adding it changes real rebuild-cost/idempotency semantics, not just visibility).
- ~~**`src/http.rs:525,530`** (same missing-`GROUP ALL` shape as N-1, in the health-report path) — flagged, not fixed; out of this task's named scope (M-1, N-1, N-2, N-5 + maintenance/reembed.rs).~~ **FIXED in this branch's maintenance/scope-audit rerun** — see "Fixed (maintenance/scope-audit rerun)" above. Struck rather than removed per this document's own provenance-discipline norm elsewhere in this section.

### Testing evidence

**Provenance note (rerun): no binary hash in this document is canonical.** The durable artifact is the command `git checkout <commit> && cargo build --release --locked && shasum -a 256 target/release/surreal-mind`. Because `build.rs` embeds the current Git commit, every commit—including documentation-only commits—changes binary identity; this document does not claim a hash for the commit that contains any particular sentence about it. Historical hashes below are data points for their named commits only.

- `cargo fmt --all -- --check`: clean.
- `cargo check --workspace --all-targets --locked`: clean.
- `cargo check --all-targets --features db_integration`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings` and `--all-features` variant: clean, zero warnings.
- `cargo test --workspace --locked` (default features, matching CI's exact invocation): 80 lib tests + all non-db_integration integration tests pass; `tests/mcp_protocol.rs` and `tests/dimension_hygiene.rs` compile to 0 tests under this invocation (feature-gated behind `db_integration`).
- **Live db_integration run against a genuinely disposable namespace** (`SURR_DB_NS=followup_disposable_20260824230629`, `SURR_DB_DB=scratch`, same local SurrealDB instance, `RUN_DB_TESTS=1`, `REEMBED_TEST_CONFIRM_DISPOSABLE_NS=1`): all 7 tests in `tests/dimension_hygiene.rs` pass (the original 5 plus this rerun's `test_kg_embed_actually_updates_matched_row` and `test_reembed_kg_actually_updates_matched_row`), plus the 1 inline test in `src/http.rs::tests` (run separately via `cargo test --features db_integration --bin surreal-mind`, since it lives in the `main.rs`-only `http` module). The N-5 test was previously verified to *discriminate* against a temporary revert (unchanged this pass, not re-verified). This pass's new http.rs test caught a real bug live before passing: the first attempt's query (`... LIMIT 100000 GROUP ALL`) failed with SurrealDB's `"Unexpected token \`GROUP\`, expected Eof"` parse error, because SurrealQL's clause order requires `GROUP` before `LIMIT`; fixed by dropping `LIMIT` (see the http.rs Fixed entry above), after which the test passed. Disposable namespace fully removed after via a temporary, uncommitted test file that issued `REMOVE NAMESPACE` and was deleted before this commit (never part of the tracked test suite); confirmed via `INFO FOR ROOT` before/after showing the disposable namespace present then absent, with every other namespace (`default, legacymind, main, photography, surreal-mind, surreal_mind, test`) unchanged. Production namespaces/DB/port 8787 untouched throughout.
- `cargo build --release --locked`: clean before and after the prior pass's source changes. Historical data point: commit `fe9e408` produced SHA-256 `9701ded096977392d3c3fc89d57c844b6317f36fbc7c170c3e6cc85572503423` and `surreal-mind 0.8.2+fe9e408` from a clean checkout. Later commits necessarily have different embedded commit metadata and must be built and measured independently.
- `git diff --stat Cargo.lock`: empty across this pass — Cargo.lock unchanged.
- **No production contact**: no binary replacement, no launchctl action, no port 8787 process action, no tunnel change, no live database write, no Task mutation, no peer message. Live Studio worktree (`/Users/samuelatagana/Projects/LegacyMind/surreal-mind`) never touched.

## [Unreleased] - rmcp 3.1.4 upgrade (deployed 2026-08-24)

Branch `codex/rmcp-3.1.4`, isolated worktree cut from `874d229c8a4cd7494d452b9d44fa6d56c2e85ffb`. Full design, decisions (D1-D11), and risk register: `docs/tasks/20260823-rmcp-3.1.4-upgrade/`. **Package version intentionally left at `0.8.2`** — a dependency-only major bump is not, on its own, a recorded reason to change the crate version (see the upgrade doc's Constraint 9); a version bump remains available as its own decision at the deployment gate.

Production deploy HEAD `e5c33ad` serves binary SHA-256 `3fd8e01afaa2841bc2678e769e8a8c5e68e95743482704098b2ad5e5ccb29a67` with `SURR_HTTP_ALLOWED_HOSTS=mcp.samataganaphotography.com`. Local/public/Codex/Claude acceptance passed. An earlier `f4869de` install was rolled back cleanly after Claude Code 2.1.241 rejected missing SEP-2549 list metadata; the fixed deployment retains the original `0f7fbccb...5911c5c` rollback artifact.

### Changed

- **rmcp `0.16.0` → `3.1.4`** (exact pin, all four existing feature flags — `macros`, `transport-io`, `transport-streamable-http-server`, `transport-worker` — preserved unchanged). Transitive lockfile deltas: `rmcp-macros` matched to `3.1.4`; `sse-stream` `0.2.1` → `0.2.5`; `darling`/`darling_core`/`darling_macro` `0.23.0` → `0.24.1` (rmcp-macros' proc-macro dependency); a second `syn` major (`3.0.4`, alongside the existing `2.0.114` used elsewhere in the tree) and `indexmap` pulled in as new direct/transitive dependencies of rmcp; a second `base64` version (`0.23.1`, alongside the existing `0.22.1`) because rmcp 3.1.4 moved to a newer `base64` major while every other consumer in the tree is still on `0.22`. No other dependency changed.
- **`Tool` construction**: all 16 `list_tools` entries now use `Tool::new(name, description, input_schema).with_title(title)` instead of the removed 0.16 struct-literal form (`execution` field no longer exists; `Tool` became `#[non_exhaustive]`). Names, titles, descriptions, input schemas, and registration order are unchanged.
- **`CallToolRequestParams` construction**: every call site (router self-calls in `maintenance.rs`, `kg_wander.rs`, and the test suite) now uses `CallToolRequestParams::new(name).with_arguments(args)` instead of the removed struct-literal form (the `task` field no longer exists on this type).
- **`get_info()`**: rebuilt with `ServerCapabilities::builder().enable_tools_with(...)` and `Implementation::new(...).with_title(...).with_description(...).with_website_url(...)`, since `ServerCapabilities`, `ToolsCapability`, `Implementation`, and `InitializeResult` (aka `ServerInfo`) are all `#[non_exhaustive]` in 3.1.4. `tools.listChanged` is still explicitly serialized as `false` (built via `ToolsCapability::default()` then field-mutated, since non-exhaustive structs still permit field assignment — only literal construction is banned).
- **Protocol negotiation**: removed the custom `initialize` override in `src/server/router.rs` that echoed back whatever `protocol_version` the client requested verbatim. rmcp 3.1.4's default `initialize` now negotiates the response version against `supported_protocol_versions()` (left at rmcp's own default, `ProtocolVersion::KNOWN_VERSIONS`) instead. An unsupported/future version can no longer be falsely accepted.
- **`call_tool` return type**: `ServerHandler::call_tool` now returns `CallToolResponse` (a new `Complete`/`InputRequired`/`Task` enum introduced for the SEP-2663 tasks extension and MRTR). Every one of the 15 tool-handler modules keeps returning `CallToolResult` unchanged; the router converts once, at the boundary, via `CallToolResult`'s `Into<CallToolResponse>`.
- **Content extraction**: `kg_wander`'s `execute_wander` and the equivalent test-suite assertions now match `rmcp::model::ContentBlock::Text(text)` directly instead of the removed `RawContent::Text` behind a `.raw` field — 3.1.4 dropped the `Content { raw, annotations }` wrapper in favor of `ContentBlock` as the direct enum in `CallToolResult::content`.
- **Streamable HTTP config** (`src/http.rs`): `StreamableHttpServerConfig` is `#[non_exhaustive]` in 3.1.4 and the old `stateful_mode: bool` field is gone, replaced by `legacy_session_mode: bool` (rmcp's closest equivalent, and also its own default). The service is now built via `StreamableHttpServerConfig::default().with_legacy_session_mode(true).with_sse_keep_alive(...).with_json_response(false).with_allowed_hosts(...).with_stateless_protocol_metadata_required(false)`, preserving prior behavior (`stateful_mode: true` → `legacy_session_mode: true`) while adding the new explicit Host allowlist. `allowed_origins` (Origin validation) is intentionally left at rmcp's empty/disabled default — see the new `SURR_HTTP_ALLOWED_HOSTS` entry below and the security note under Deferred.
- **`test_notification` / SEP-2577 logging deprecation bridge**: rmcp 3.1.4 deprecates the entire logging-notification surface (`LoggingLevel`, `LoggingMessageNotificationParam`, `notify_logging_message`) per SEP-2577, but keeps it functional. `test_notification` is a public tool this upgrade does not remove; `src/tools/test_notification.rs` carries the sole `#[allow(deprecated)]` bridge in the codebase, scoped to the `use` import and the handler function body only — the rest of the workspace remains warning-clean under `-D warnings`. Retiring or replacing this tool is a separate follow-up (not part of this upgrade — see upgrade doc D8).
- **Stale tool-count log**: `main.rs`'s startup log said "Loaded 15 MCP tools" and omitted `journal` from the list; corrected to 16 and the full name list. `howto.rs`'s hand-maintained overview-mode roster (`src/tools/howto.rs`, the `tools` vec used when `howto` is called with no `tool` argument) was separately missing `test_notification`; added. `tests/tool_schemas.rs`'s synthetic 14-tool expectation list was also stale (missing `journal` and `call_vibe`); corrected to the real 16.

### Fixed

- **Claude Code 2.1.241 `tools/list` compatibility (SEP-2549 `ttlMs`/`cacheScope`)**: production was briefly upgraded to this branch (`f4869de`) and Claude Code 2.1.241 reported `tools fetch failed — Invalid result for tools/list: expected number at path ttlMs (received undefined)` / `cacheScope expected public|private`; rollback to rmcp 0.16.0 restored `Connected`. Root cause: `src/server/router.rs`'s `list_tools` built `ListToolsResult { tools, ..Default::default() }`, leaving the new SEP-2549 `ttl_ms`/`cache_scope` fields `None`; rmcp's `paginated_result!` macro marks both `#[serde(skip_serializing_if = "Option::is_none")]`, so they were silently omitted from the wire — valid under rmcp's own backward-compat contract for peers below protocol `2026-07-28`, but this server doesn't narrow `supported_protocol_versions()` (still rmcp's default `KNOWN_VERSIONS`, which includes `2026-07-28`), so it negotiates `2026-07-28` with any client that offers it. Claude Code 2.1.241's `2026-07-28` response schema treats both fields as *required*, stricter than rmcp's own leniency; rmcp 0.16.0 predates SEP-2549 and protocol `2026-07-28` entirely, so it never offered that version and never hit this. Fix: `list_tools` now routes through a new `list_tools_result()` helper (`src/server/router.rs`) that explicitly calls `.with_ttl_ms(300_000)` and `.with_cache_scope(CacheScope::Public)` — 5-minute TTL (the roster is static; `tools.listChanged` is already `false`) and `Public` scope (the roster is identical for every caller of this single-tenant server). Tool names/order/descriptions/input schemas are unchanged; only the two new metadata fields are now always present on the wire. Reproduced pre-fix and verified fixed against an isolated candidate (Studio, port 18792, ephemeral token) using Claude Code 2.1.241 with a strict `--mcp-config`/`--strict-mcp-config` temp config; new regression test `server::router::list_tools_result_tests::list_tools_result_serializes_required_sep2549_fields` asserts both fields serialize correctly. 2026-08-24.

### Added

- **`RuntimeConfig::http_allowed_hosts`** (`src/config.rs`) and `SURR_HTTP_ALLOWED_HOSTS`: rmcp 3.1.4 added `Host`-header validation to Streamable HTTP as a DNS-rebinding defense, defaulting to loopback-only. The secure loopback set (`localhost`, `127.0.0.1`, `::1`) is always retained; an unset env var uses that set alone; a valid comma-separated configured value **extends and deduplicates** the loopback set (never replaces it); a present-but-empty, whitespace-only, or malformed (stray/leading/trailing comma) value fails startup loudly rather than silently degrading to loopback-only or allow-all. 9 new focused unit tests cover absent/default, extension, loopback retention, whitespace, duplicates, and every malformed-input shape (10 `#[test]` functions total in `src/config.rs`, including the pre-existing `test_config_loading`). The production value (expected: loopback plus `mcp.samataganaphotography.com`, evidenced out-of-band via the Cloudflare `Host`-forwarding measurement in the upgrade doc) is deployment configuration (launchd/env file), not a Rust constant, and is not embedded in this branch.
- **Protocol/tool-contract test coverage** (`tests/mcp_protocol.rs`): `test_list_tools_protocol` now asserts the exact 16-name, ordered tool contract plus a title/description/schema spot-check (TOOL-01/02), instead of only checking that `search` is present. New `test_initialize_protocol_negotiation` drives real `initialize` handshakes for a supported legacy version, the current latest version, and a synthesized unsupported future version, asserting negotiation behavior (never echoing an unsupported version as accepted) and that `tools.listChanged` still serializes as explicit `false` in every case (PROTO-01/02/03/05). New `test_notification_protocol_bridge` drives a real `test_notification` tool call through the protocol harness and asserts both the response and the client-observable notification arrive (TOOL-06).
- **`tests/stdio_smoke.rs`** (STDIO-01): stdio is `SURR_TRANSPORT`'s default and `main.rs`'s default transport path, so it gets a runtime witness rather than compile-only coverage. Spawns the built binary as a subprocess over piped stdio with a disposable config, sends newline-delimited JSON-RPC `initialize` + `notifications/initialized` + `tools/list`, and asserts a negotiated protocol version, `tools.listChanged: false`, and the exact 16-tool list — with no extraneous bytes on stdout before the JSON.
- All new tests requiring a live server (`mcp_protocol.rs` additions, `test_wander.rs`/`mcp_integration.rs` content-extraction fixes, `stdio_smoke.rs`) follow the existing `RUN_DB_TESTS=1`-gated pattern and were compiled and lint-checked (`cargo check --all-targets --features db_integration`, `cargo clippy ... -D warnings`) but **not executed** in this implementation pass — no verified disposable database/namespace was available in this session, and the upgrade's constraints forbid running database-writing tests against the production namespace. Execution against a real disposable namespace is a Testing-phase gate (see `docs/tasks/20260823-rmcp-3.1.4-upgrade/rmcp-3.1.4-upgrade-testing.md`).

### Deferred

- **Origin validation** (`allowed_origins`) remains at rmcp's empty/disabled default for this migration (upgrade doc D7) — the current accepted client-`Origin` set has not been measured. Bearer auth and the new Host allowlist still constrain the endpoint. Measuring real client `Origin` headers and enabling this sibling DNS-rebinding defense is a separate follow-up task.
- **`test_notification` / SEP-2577 retirement or replacement** is a separate follow-up task (upgrade doc D8); this upgrade only ports it forward without normalizing the deprecation.
- **Pre-existing dirty-worktree reconciliation** (`src/tools/maintenance.rs` and several documentation files modified in the source Studio worktree before this branch was cut) is a separate, explicitly-scoped pre-merge task (upgrade doc D11 / impl doc Phase 8) and is **not** included in this branch.

## [0.8.2] - 2026-03-12

### Fixed

- **`maintain embed_pending` persistence accounting**: Fixed pending-thought retries so they select plain `meta::id(id)` record keys before `type::record('thoughts', $id)` updates, verify that each update actually persisted a complete embedding with the expected dimension before counting success, refresh embedding metadata on retry, and report the real post-run pending count instead of subtracting successful attempts from an already-updated count.
- **Codex federation identity support**: Added `codex` as a valid `journal` author and `wander`/`rethink` attention-routing target so Codex-authored KG work preserves its own provenance instead of defaulting to `cc`.
- **REMini KG consolidation planning**: `gem_rethink` now writes structured pending merge state (`mode`, `loser_id`, `winner_id`) when it can identify a merge target, and `kg_consolidate` consumes that structure before falling back to legacy reasoning-text parsing. `kg_consolidate` also ignores unresolved non-merge correction history instead of reporting it as failed dedup work.
- **KG dedupe planner alias awareness**: `kg_dedupe_plan` now excludes entities already marked as aliases/canonicalized from candidate queries, so post-apply duplicate-group counts reflect remaining real work instead of re-counting already-merged losers.
- **KG dedupe planner datetime decoding**: `kg_dedupe_plan` now string-casts `created_at` in its entity query (`type::string(created_at)`) so the planner can read SurrealDB 3.x datetime results without failing with `Expected any, got datetime`.
- **Registry test stability**: Removed brittle global-size assertions in registry/cancel tests and switched to UUID-scoped assertions to avoid cross-test interference from shared global registry state.
- **Server version consistency**: `server_info.version` now reads from `env!("CARGO_PKG_VERSION")` so MCP runtime metadata stays in sync with crate version.
- **agent_job_status robustness**: Optional `started_at`/`completed_at` values now emit `null` (not stringified `NONE`) and integration tests serialize shared DB/server access to eliminate parallel cross-test flakiness.

## [0.8.1] - 2026-03-07

### Removed

- **call_warp Tool**: Removed `call_warp` delegation tool from MCP surface. The `WarpClient` remains available in the codebase for potential future use. Federation now uses three delegation paths: `call_gem`, `call_cc`, and `call_vibe`.

## [0.8.0] - 2026-03-05

### SurrealDB 2.x → 3.x Migration

Emergency migration after `brew upgrade` installed SurrealDB 3.0.1, which could not read the v2 SurrealKV manifest format (`Unsupported manifest format version: 0`). All data (2,393 thoughts, full KG, photography namespace) preserved and restored.

### Changed

- **surrealdb Crate 2.0 → 3.0**: Updated Rust client crate across 28 source files. API changes include query response handling, `WsClient` type usage, and record ID deserialization.
- **`type::thing` → `type::record`**: Renamed across 49 occurrences in 15+ files. SurrealDB 3.x renamed this function; old name produces parse errors.
- **Schema `FLEXIBLE` Keyword**: Moved from before `TYPE` to after (e.g., `TYPE option<object> FLEXIBLE`). SurrealDB 3.x reversed the keyword order.
- **Schema `DEFINE FIELD id` Removal**: Removed `DEFINE FIELD id ON TABLE ... TYPE record<...>` from `correction_events` and `agent_exchanges` tables. SurrealDB 3.x rejects explicit `record<table>` type on `id` fields.
- **SurrealDB Plist**: Added `--user root --pass root` to `com.legacymind.surrealdb.plist` ProgramArguments for fresh instance authentication after data directory wipe and reimport.
- **Memory Injection Retrieval Path**: Switched `inject_memories` from Rust-side cosine scoring over fetched raw embeddings to DB-side scoring using `vector::similarity::cosine(...)` with scalar result fields only (`id`, `name`, `entity_type/description`, `similarity`). This reduces payload size, avoids SurrealDB 3.x WS decode edge cases, and keeps threshold/floor behavior unchanged.
- **think Debug Telemetry**: Added step-level timing logs around `think` execution (`continuity`, `CREATE`, embedding call, `UPDATE`, framework update, candidate fetch, injection persist) to make future runtime stalls diagnosable without invasive tracing.

### Fixed

- **REMini launchd Environment**: Added explicit PATH environment variable to `dev.legacymind.remini.plist` to ensure homebrew binaries (`/opt/homebrew/bin`) are accessible to scheduled maintenance tasks. Fixed failing `wander` (gemini CLI not found) and `health` (surreal CLI not found) tasks. Also corrected typo in `SURR_ENV_FILE` path.
- **think Tool Hang on SurrealDB 3.x**: Resolved silent `think` stalls after migration to SurrealDB 3.x. Root cause was websocket deserialization failure when memory injection fetched raw embedding arrays from KG tables (`Failed to decode fb value`). `think` now completes and returns MCP responses reliably in stdio and HTTP flows.
- **Test Compatibility with rmcp 0.16**: Updated integration/smoke tests to use `CallToolRequestParams` shape with required `meta` and `task` fields. This resolves `cargo clippy --all-targets` build failures caused by outdated request initializers.
- **Git Push Divergence Resolved**: Fixed local `master` branch divergence from `origin/master` by fetching remote changes and rebasing local commits. Successfully pushed 3 local commits integrating 1 remote commit without conflicts, resolving non-fast-forward rejection.
- **SurrealDB 3.x Query/Type Follow-ups (`search`, `wander`, `corrections`)**: Fixed missed migration issues by replacing legacy record checks in `unified_search` (`type::is::record(...)` → `meta::tb(...) IS NOT NONE`), removing `SELECT *` in `wander` in favor of explicit field projections, and string-casting datetime outputs (`created_at`, `marked_at`, `timestamp`) with `type::string(...)` to resolve runtime decode errors like `Expected any, got datetime`.
- **`search` Chain-ID Hang**: Fixed `unified_search` stalling when `chain_id` was provided. Root cause was repeated inline subqueries (`SELECT ... FROM thoughts WHERE chain_id = $cid`) inside entity/relationship/observation filters. Search now resolves chain thought IDs once per request and reuses a bound `$chain_ids` list, eliminating the pathological query plan and returning promptly.

### Migration Process

1. Downloaded SurrealDB 2.6.3 binary to export existing data with `--v3` compatibility flag
2. Exported all 6 databases across 4 namespaces (surreal_mind, photography, legacymind, test)
3. Backed up v2 data directory, started fresh 3.0.1 instance
4. Imported smaller databases directly, fixed consciousness export (removed `DEFINE FIELD id TYPE record<>` lines, changed `SCHEMAFULL` → `SCHEMALESS` for tables with extra fields)
5. Updated surrealdb Rust crate, fixed all compile errors, replaced `type::thing` → `type::record`
6. Fixed `FLEXIBLE` keyword positioning in schema.rs
7. Resolved think tool hang caused by WS deserialization of raw embedding arrays

### Removed

- **call_codex Tool**: Removed `call_codex` delegation tool from MCP surface. The `CodexClient` remains available in the codebase for potential future use. Federation now uses three delegation paths: `call_gem`, `call_cc`, and `call_warp`.
- **Dead Directories**: Removed `models/` (260MB BGE model weights - Candle/local embedding support was removed), `.idea/` (JetBrains), `.aiassistant/` (JetBrains AI), `.agent/` (Gemini rules), `.venv-convert/` (46MB one-off Python venv).
- **Stale Files**: Removed `.rc-prep` (September 2024 RC marker), `docs/QUICKSTART.md` (referenced old tool names).
- **One-off Scripts**: Cleaned `scripts/` - removed `check_chain_id_usage.py`, `diagnose_entity_data.py`, `test_chain_id.py`, `test_kg.py`, `test-sleep-gemini.sh`, `package.json`, and `migration/` subproject (1.3GB target dir). Photography scripts (`backup_database.py`, `cleanup_duplicates.py`, `investigate_duplicates.py`) moved to photography-mind.
- **TUI Binary**: Removed `smtop` dashboard; rely on `/metrics` or external observability.

### Removed

- **Photography Scripts**: Deleted `scripts/import_skater_requests.py` and `scripts/validate_contacts.py` - these belong in photography-mind, not surreal-mind.
- **Deprecated Shell Tests**: Removed 8 shell test scripts that referenced deprecated tools (`think_search`, `think_convo`): `simple_test.sh`, `test_with_data.sh`, `debug_search_low_thresh.sh`, `debug_search.sh`, `test_search.sh`, `test_mcp_comprehensive.sh`, `test_detailed_mcp.sh`, `test_simplified_output.sh`. Kept 4 valid scripts: `test_simple.sh`, `test_mcp.sh`, `test_stdio_persistence.sh`, `check_version.sh`.

### Changed

- **Tool File Naming**: Renamed `delegate_gemini.rs` → `call_gem.rs` and `detailed_help.rs` → `howto.rs` for consistency with tool names. Handler methods also renamed (`handle_delegate_gemini` → `handle_call_gem`, `handle_detailed_help` → `handle_howto`).
- **Memory Injection Retrieval Path**: Switched `inject_memories` from Rust-side cosine scoring over fetched raw embeddings to DB-side scoring using `vector::similarity::cosine(...)` with scalar result fields only (`id`, `name`, `entity_type/description`, `similarity`). This reduces payload size, avoids SurrealDB 3.x WS decode edge cases, and keeps threshold/floor behavior unchanged.
- **think Debug Telemetry**: Added step-level timing logs around `think` execution (`continuity`, `CREATE`, embedding call, `UPDATE`, framework update, candidate fetch, injection persist) to make future runtime stalls diagnosable without invasive tracing.
- **Repository Hygiene Pass**: Completed `cargo fmt` and `cargo clippy --all-targets` with clean results after migration fixes. Also removed temporary debug/test artifacts created during incident triage.
- **Documentation Sync**: Updated `README.md`, `docs/AGENTS/{arch,setup,connections}.md`, and `docs/DEPENDENCIES.md` to reflect SurrealDB 3.x baseline, current tool naming (`call_gem`), and current runtime environment variable expectations.
- **call_codex Tool**: Refactored to synchronous execution - returns response directly in MCP call instead of async job queue. Removed worker polling pattern for simpler, more reliable operation.
- **CodexClient**: Added `--skip-git-repo-check` flag for execution in any directory. Fixed NDJSON parser to handle Codex's `item.aggregated_output` format and `thread_id` extraction.
- **Codex Model Configuration**: Default model and available models dropdown now read from environment variables (`CODEX_MODEL` and `CODEX_MODELS`) instead of hardcoded - no rebuild required to change model list.
- **call_gem Native Resume**: Added `resume_session_id` and `continue_latest` parameters. Gemini CLI auto-saves all sessions - use `continue_latest: true` for `--resume` (latest) or `resume_session_id` for specific session.

### Added

- **test_notification Tool**: New tool for testing MCP notification capabilities (`peer.notify_logging_message`). Sends a logging message with a specified level to the client.
- **call_cc Tool**: New tool for delegating tasks to Claude Code CLI. Synchronous execution with `--output-format stream-json`. Model selection via `ANTHROPIC_MODEL`/`ANTHROPIC_MODELS` env vars. Supports `--resume <id>` and `-c` (continue latest) for session management.
- **call_warp Tool**: New tool for delegating tasks to Warp CLI. Multi-model access through single interface: Claude (haiku/sonnet/opus), GPT-5/Codex (with reasoning levels: -low/-medium/-high/-xhigh/-max), and auto modes (auto/auto-efficient/auto-genius). One-shot executor—no resume/session support. Required: `prompt`, `cwd`. Optional: `model`, `timeout_ms`, `max_response_chars`, `task_name`, `mode`.
- **Observe Mode**: All `call_*` tools support a `mode` parameter with values `"execute"` (default) or `"observe"`. In observe mode, the delegated agent is instructed to analyze and report only—no file modifications. (Note: `call_codex` was later removed.)
- **Response Truncation**: Added `max_response_chars` parameter to all `call_*` tools (default 100KB). Prevents oversized responses from overwhelming clients. Set to `0` for no limit.
- **Federation Context**: All `call_*` tools now prepend a `[FEDERATION CONTEXT]` header to prompts, informing the delegated agent it's being invoked as a subagent by surreal-mind MCP.

### Fixed

- **delegate_gemini Worker**: Fixed job stealing bug - worker now filters by `tool_name = 'delegate_gemini'` to prevent claiming jobs from other tools like call_codex.
- **CodexClient Session Resume**: Fixed CLI argument ordering per v0.79.0+ docs. Resume is a subcommand of exec with strict ordering: `codex exec resume <id> "prompt" [flags]`. Prompt now placed before flags.
- **Search NULL vs NONE**: Fixed `unified_search.rs` to use `IS NOT NONE` instead of `IS NOT NULL` for SurrealDB 2.x compatibility. Thoughts with uninitialized embeddings were causing `vector::similarity::cosine()` errors.
- **REMini Timeout**: Added `--timeout` flag (default 3600s = 1 hour per task). Uses spawn + polling instead of blocking `.output()` to prevent runaway tasks from hanging indefinitely.
- **wander ID normalization**: `wander` now accepts `entity:` / `observation:` / `thought:` aliases and validates record existence before querying, preventing `meta::id()` type errors when starting from entity IDs.
- **wander meta::id() serialization**: Fixed critical bug where `wander` tool failed with "invalid type: enum" serialization error. Updated all SQL queries to properly use `meta::id(id) as id` to convert Thing objects to strings, ensuring JSON serialization compatibility. This affects 12 query statements across all wander modes (random, semantic, meta, marks).

### Removed

- **PersistedAgent Wrapper**: Removed fake memory/statefulness layer that concatenated previous exchanges into prompts. The `persisted.rs` module and related `agent_exchanges`/`tool_sessions` DB writes are removed.
- **call_codex Async Worker**: Removed background job queue pattern in favor of synchronous execution.
- **call_gem Async Worker**: Removed background job queue pattern in favor of synchronous execution. Tool now returns response directly.

---

### Added

- **call_codex Tool**: Added Codex CLI delegation with async job tracking, resume options, and stream metadata capture.
- **Graceful Embedding Degradation**: Thoughts are now saved before embedding, preventing data loss when the OpenAI embedding API is unavailable. Failed embeddings can be retried later via `maintain embed_pending`. Adds `embedding_status` field to thoughts table (values: `pending`, `complete`, `failed`).
- **Phase 1: Schema & Data Model**: Implemented the initial schema for the REMini & Correction System, adding Mark fields (`marked_for`, `mark_type`, `mark_note`, `marked_at`, `marked_by`) to thoughts, kg_entities, and kg_observations tables, and creating the CorrectionEvent table with fields for provenance tracking.
- **Phase 2: rethink Tool - Mark Mode**: Implemented the `rethink` MCP tool with mark creation capability.
- **Phase 3: wander --mode marks**: Added capability to surface and filter marks in the `wander` tool.
- **Phase 4: rethink Tool - Correct Mode**: Implemented full correction provenance with CorrectionEvent tracking and derivative cascading.
- **Phase 5: gem_rethink Process**: Created a specialized binary for autonomous background correction processing by Gemini.
- **Phase 6: REMini Wrapper**: Implemented a unified maintenance orchestrator (`remini` CLI) to manage background tasks.
- **Phase 7: Forensic Queries**: Added `--forensic` flag to the `search` tool to expose correction chains and provenance data.
- **Phase 8: Confidence Decay**: (Foundation) Added confidence fields and decay tracking logic to the core schemas.
- **Phase 9: Corrections Tool**: Integrated the standalone `corrections` tool and mapped it into the `maintain` surface.

### Removed

- **Scalpel Tool**: Fully removed the scalpel tool and local delegation infrastructure to free port 8111 and improve reliability. Scalpel was unreliable on the 32GB Studio; use remote `call_gem` for delegation instead.
- **Scalpel Environment Variables**: Removed all scalpel-related environment variables (`SURR_SCALPEL_MODEL`, `SURR_SCALPEL_ENDPOINT`, `SURR_SCALPEL_MAX_TOKENS`, `SURR_SCALPEL_TIMEOUT_MS`) from `.env` and `.env.example` files.

### Changed

- **Thought Persistence**: Avoid writing empty embeddings during initial thought creation so HNSW indexing doesn't reject the record; embedding is only set after a valid vector is produced.
- **Thought Schema**: Set `thoughts.embedding` to `option<array<float>>` with `DEFINE FIELD OVERWRITE` so the migration applies on startup; initial create uses `embedding: NONE` to pass schema validation before embedding is computed.
- **Thought Create Validation**: Thought creation now returns `meta::id` and checks the response to surface DB errors instead of failing silently.
- **Scalpel Configuration**: Removed hardcoded default model from `src/clients/local.rs`. The `SURR_SCALPEL_MODEL` environment variable is now **mandatory**. This prevents silent failures/mismatches by forcing explicit configuration in `.env`.
- **Documentation**: Added Scalpel configuration section to `.env.example`.
