# rmcp 3.1.4 — post-deploy delta review (CC)

**Reviewer:** CC (Claude Code, Opus 5)
**Requested by:** Codex-MBP, 2026-08-24
**Scope:** `26d5990143c10242f738f1aa53e31400c6518636..e5c33ad5ec8c3475cbab9ef9d1382aaa7d04c310`
**Mode:** read-only. No edits, builds, installs, production exercise, evidence cleanup, or Task mutation were performed.

## Why this review exists

The independent review gate for the rmcp 3.1.4 upgrade was satisfied at `26d5990`
("docs: independent auditor corroboration pass for RUN-04/05, PROTO-04") via a
Sam-directed alternate path — a resumable Fable 5 Dynamic Workflow with independent
Sonnet 5 audit/closure workers, followed by Codex artifact grading. Live CC was
deliberately not used. **That gate is legitimate and is not reopened here.**

`26d5990` is an **ancestor of** the deployed commit `e5c33ad`. Three commits landed
between them:

| commit | subject | `src/` lines |
|---|---|---|
| `8f9a6d7` | docs: record Codex source acceptance for rmcp 3.1.4 | 0+ / 0- |
| `f4869de` | chore: reconcile pre-upgrade live changes | 72+ / 13- |
| `e5c33ad` | fix: populate SEP-2549 ttlMs/cacheScope on tools/list | 83+ / 8- |

```
git diff 26d5990..e5c33ad -- src/
  src/server/router.rs      | 91 ++++++-----
  src/tools/maintenance.rs  | 85 ++++++-----
  2 files changed, 155 insertions(+), 21 deletions(-)
```

So ~176 changed source lines entered the deployed build **after** the independent
auditors finished. They could not have seen it; it did not exist yet.

> "Independent gate satisfied at `26d5990`" and "all shipped source independently
> reviewed" are different propositions. The first remains true; the second was false
> until this review.

## Method note

**The first coverage pass was measured against the wrong artifact and would have
produced an inverted finding.** `grep -c '#\[test\]' src/server/router.rs` returned
**0**, which would have been reported as "the SEP-2549 fix shipped with no regression
coverage" — the opposite of the truth.

Cause: the working tree is checked out on **dirty `master`** (25 modified files, 2 in
`src/`), not on `codex/rmcp-3.1.4-deploy`. The grep read master's copy of the file.
It was caught only because the count contradicted a diff already read directly.

Everything below is anchored to `git show <commit>:<path>`, not to the checkout.
*A grep against a working tree testifies to the checkout, not to the commit.*

## Verdict

**No blockers. Production is correctly left running.** One correctness defect worth
fixing separately, two coverage gaps, one provenance gap that is not established by
existing artifacts.

---

## 1. Source correctness / regression

### `src/server/router.rs` @ `e5c33ad` — CLEAN, no findings

The fix is correct and the doc comment states the real causal chain rather than a
plausible-sounding one: rmcp's `skip_serializing_if = "Option::is_none"` drops the
`None` defaults from the wire; the server does not narrow
`supported_protocol_versions()`, so it negotiates `2026-07-28` with any client
offering it; Claude Code 2.1.241's schema for that version treats `ttlMs`/`cacheScope`
as **required**, stricter than rmcp's own backward-compat leniency. rmcp 0.16.0
predates SEP-2549 entirely, which is why this never appeared before the upgrade.

**OBSERVATION — not a defect.** `TOOLS_LIST_TTL_MS = 300_000` combined with
`tools.listChanged = false` means a client may serve a cached roster for up to five
minutes **with no invalidation path**, because the server never notifies. Roster
changes now propagate on cache expiry only. This is a **new operational property**:
pre-upgrade, every `tools/list` was live. Correct as designed; simply no longer true
that a restart immediately reaches clients.

**OBSERVATION — invariant worth recording.** `CacheScope::Public` is justified in the
comment as "identical for every caller of this single-tenant server," which holds
today. Per SEP-2549, `Public` permits any intermediary to cache and serve the response
to any user. It becomes wrong the moment tool exposure varies by identity. Given an
OAuth layer sits in front of this server, that is a property the scope **depends on**,
not one that merely happens to hold — worth stating as an invariant so a future change
to per-identity tool exposure trips over it.

### `src/tools/maintenance.rs` @ `f4869de` — one finding

**CONFIRMED CORRECT (stated because the diff looks alarming in isolation):** changing
`"remaining": remaining.saturating_sub(succeeded)` to `"remaining": remaining` is a
**bug fix, not a regression.** Verified at `e5c33ad`: the count query is at line 958,
**after** the loop at line 865, and filters `WHERE embedding_status IN ['pending','failed']`.
Rows just set to `'complete'` already drop out of that fresh count, so the old code
subtracted `succeeded` a second time and under-reported. The new value is right.

Also genuinely improved: replacing `RETURN NONE` with a read-back of
`embedding_status` and `array::len(embedding)` turns a blind write into a verified one.

#### FINDING M-1 — retry path can be permanently poisoned (correctness)

**Anchor:** `src/tools/maintenance.rs` @ `f4869de`, the update-and-verify block
(`UPDATE type::record('thoughts', $id) SET ... embedding_status = 'complete' RETURN ...`).

The `UPDATE` sets `embedding_status = 'complete'` **unconditionally** as part of `SET`.
Verification then reads the row back and requires
`status_is_complete && embedding_len_matches`. When the persisted embedding length does
not equal `dim`, the row is counted **failed** — but the database row is already marked
`'complete'`.

**Consequence:** the selection query picks up only `'pending' | 'failed'`. Such a row is
therefore **invisible to `embed_pending` forever** — a wrong-dimension embedding,
permanently marked complete, reported as failed, and unreachable by the retry path that
exists to heal exactly this condition.

**Why it is a real finding either way:** if an off-dimension vector is genuinely
unreachable, the check is dead code and its `failed` branch is misleading; if it is
reachable, the recovery path is poisoned. The two halves disagree about whether the
case can happen.

**Cheapest fix:** on mismatch, set `embedding_status = 'failed'` rather than leaving
`'complete'`, so the retry path can still see the row.

#### FINDING M-2 — defense-in-depth incomplete (minor)

**Anchor:** `normalize_thought_record_key`, `src/tools/maintenance.rs` @ `f4869de`.

The helper strips a `thoughts:` prefix and trims backticks, but does not handle
SurrealDB's **other** record-id escaping form (angle brackets). A key arriving in that
form normalizes to a still-escaped string, `type::record()` misses, and the row silently
counts as failed.

Low severity, because the query now selects `meta::id(id)` and the raw form should not
arrive. But the helper exists precisely as the belt to that suspenders, and it covers
only one of the two escape syntaxes.

---

## 2. Does recorded acceptance exercise each changed behavior?

### `router.rs` — PARTIAL

The regression test is **not vacuous**: it asserts the *serialized* form
(`wire["ttlMs"]` as a JSON number, `wire["cacheScope"] == "public"`), which is the
actual failure mode, rather than only the struct fields. That is the right assertion.

**GAP:** the test calls `list_tools_result()` **directly**; it does not exercise
`ServerHandler::list_tools`. Reverting the handler line from `Ok(list_tools_result(tools))`
back to `Ok(ListToolsResult { tools, ..Default::default() })` reintroduces the **exact
outage** and the test still passes. The regression test does not cover the regression's
wiring — it is one line outside its reach.

**Mitigated by:** live acceptance through a real Claude MCP config that invoked
`mcp__surreal-prod__search` exercised the handler end-to-end. That is the stronger
witness here, and it is recorded.

### `maintenance.rs` — NO, and structurally so

At `e5c33ad` the file carries **2 tests**, both on the pure string helper
`normalize_thought_record_key`. There is **zero** coverage of:

- the `SELECT meta::id(id) AS id` query change
- the three newly persisted fields (`embedding_provider`, `embedding_model`, `embedding_dim`)
- the write-verification logic
- the `remaining` fix

All four are **database round-trip behaviors**, and `db_integration` is an opt-in Cargo
feature (`Cargo.toml:72`, `db_integration = []`). The recorded gate list states
**"db no-run."** Therefore the 74/74 lib run **cannot** have exercised any of them —
not "did not," *cannot*.

No evidence was found that live acceptance invoked `maintain embed_pending`; acceptance
covered `tools/list` and search. If it was not invoked against production, then
`f4869de`'s entire behavioral surface shipped **unexercised by any witness** — unit,
integration, or live.

**This is the largest gap in the delta, and it is in the commit labeled `chore`.**
A `chore` label on 85 lines of source movement is the shape that passes a review sweep
unexamined.

---

## 3. Provenance — live-binary-to-`e5c33ad` identity is ASSUMED, not established

### Established by measurement

`target/release/surreal-mind` hashes to
`3fd8e01afaa2841bc2678e769e8a8c5e68e95743482704098b2ad5e5ccb29a67`, matching the
reported live SHA-256 exactly. **Which file is running is proven**, and the preserved
rollback binary (`0f7fbccb…5911c5c`) is a distinct artifact.

### Not established

- **No `build.rs`** in the repository, so no commit hash is embedded. The binary cannot
  self-attest.
- **The build artifact predates the commit it is attributed to.** Artifact mtime
  `2026-08-24 19:03:45`; `e5c33ad` committed `19:04:26` — 41 seconds later. This is
  ordinary build-then-commit ordering and is **not** evidence of wrongdoing, but it
  means the binary was built from an **uncommitted working tree**, and no artifact
  attests that the tree was byte-identical to what landed 41 seconds afterward.
- **Rust release builds are not bit-reproducible by default**, so rebuild-and-compare
  cannot settle it retroactively.
- The repository is currently on dirty `master` with `src/tools/maintenance.rs` and
  `src/bin/kg_debug_tool.rs` modified, so the present tree does not reproduce `e5c33ad`
  either.
- `rmcp-3.1.4-upgrade-impl.md:17` — "Record the live binary SHA-256…" — is an
  **unchecked** box, and the SHA was not found recorded in a durable artifact.
  *Caveat:* that document's mtime predates the 2026-08-24 execution, so unchecked boxes
  reflect a document not updated, not necessarily steps not run.

**This is not a claim that the wrong binary is deployed.** Every indicator is consistent
with correct deployment. It is a claim that the chain is *consistent with* rather than
*establishes* — and "consistent with" is not "excludes."

### Cheapest permanent fix

Add a `build.rs` embedding the git hash plus a dirty flag, so `--version` self-attests.
**`comm` already does exactly this in this federation** (`comm 0.1.0+0bc53d0`), which is
why a version report and an independent measurement could be cross-checked in seconds on
2026-08-24. `surreal-mind` has no equivalent, and that asymmetry is the entire reason
this question is open.

---

## Out of scope — flagged, not reviewed

- The working tree carries an **uncommitted** change to `src/tools/maintenance.rs`
  replacing `CallToolRequestParams::new("corrections").with_arguments(map)` with a raw
  struct literal including `meta: None` and `task: None`. That moves **away** from the
  3.1.4 builder idiom and toward the hand-construction pattern `#[non_exhaustive]`
  exists to prevent. Not in the delta, not deployed, not reviewed — but it would regress
  the idiom if committed.
- `rmcp-3.1.4-upgrade-impl.md:100` reads "Request CC implementation review before
  production build/install," unchecked. **Not re-litigated here** — the alternate review
  path was Sam-directed and is accepted. Noted only so the record shows that gate was
  **substituted**, not that it did not exist.

## Disposition

Nothing in this review warrants rollback or emergency change. Per Codex, M-1, database
behavioral coverage, router handler-wiring regression coverage, and build provenance are
routed as **follow-up work** rather than reopening the completed upgrade.
