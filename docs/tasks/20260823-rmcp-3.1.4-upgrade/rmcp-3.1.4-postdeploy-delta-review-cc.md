# rmcp 3.1.4 — post-deploy delta review (CC)

**Reviewer:** CC (Claude Code, Opus 5)
**Requested by:** Codex-MBP, 2026-08-24
**Scope:** `26d5990143c10242f738f1aa53e31400c6518636..e5c33ad5ec8c3475cbab9ef9d1382aaa7d04c310`
**Mode:** read-only. No edits, builds, installs, production exercise, evidence cleanup, or Task mutation were performed.

---

## ⚠️ REVISION 2 — 2026-08-24. TWO CLAIMS BELOW ARE RETRACTED. READ THIS FIRST.

Revision 1 (commit `4c32b3c`, sha256 `fd691bc3…`) was produced by a **single-pass inline review**.
It was then attacked by a 16-agent adversarial Dynamic Workflow (4 dimension readers + 3 skeptics
instructed to *refute* M-1, every significant finding independently verified), and Codex executed
live read-only discriminators against Studio SurrealDB 3.2.3.

**Result: one finding downgraded, one verdict retracted outright, four new defects found, two of the
adversarial pass's own findings refuted in turn.**

| # | Revision-1 claim | Status now |
|---|---|---|
| M-1 | wrong-dimension row is *"permanently poisoned… unreachable forever"* | 🔴 **REFUTED (2 of 3 skeptics).** Named the wrong healer. Downgraded to a verification-integrity defect. |
| `remaining` | *"CONFIRMED CORRECT"* | 🔴 **RETRACTED.** The query is missing `GROUP ALL` and can only report 0 or 1. Empirically confirmed live. |
| "74/74 lib tests" | stated as fact | 🟡 **UNSOURCED.** Relayed from a peer's receipt, never measured. Repo contains 71/71 pre-delta. |

**Nothing below is deleted.** Retracted text is struck through in place, per the standing doctrine
that erasing the past robs the next reader of the actual story — and because *how* a single pass
produced these errors is the more durable finding.

🔴 **THE PATTERN ACROSS ALL THREE ERRORS IS ONE ERROR: verifying a neighbouring proposition and
concluding the target one.** "The count runs after the loop" ≠ "the count aggregates."
"`embed_pending` cannot reach the row" ≠ "no healer can reach the row." "Codex reported 74/74" ≠
"74/74 is true." Each check was *performed correctly* and answered a question adjacent to the one
that mattered.

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

🔴 **RETRACTED — SEE FINDING N-1 BELOW. THE VERDICT IN THIS PARAGRAPH IS WRONG.**

~~**CONFIRMED CORRECT (stated because the diff looks alarming in isolation):** changing
`"remaining": remaining.saturating_sub(succeeded)` to `"remaining": remaining` is a
**bug fix, not a regression.** Verified at `e5c33ad`: the count query is at line 958,
**after** the loop at line 865, and filters `WHERE embedding_status IN ['pending','failed']`.
Rows just set to `'complete'` already drop out of that fresh count, so the old code
subtracted `succeeded` a second time and under-reported. The new value is right.~~

⛔ **What Revision 1 actually verified was that the count runs AFTER the loop — which is true — and
never whether the query AGGREGATES, which it does not.** The removal of `saturating_sub` is still
correct in isolation; the value it now reports is broken for an unrelated reason that the same read
should have caught. See **N-1**.

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

🔴 **THE CONSEQUENCE CLAIM ABOVE IS REFUTED — 2 of 3 independent skeptics, confirmed by live
measurement. The row is NOT unreachable. Revision 1 named the wrong healer.**

~~invisible to `embed_pending` forever… unreachable by the retry path~~ — the healing path is **not**
`embed_pending`. It is `reembed`, and it is **status-blind by design**, keying on measured vector
length rather than status:

- `e5c33ad:src/maintenance/reembed.rs:96-98` — `SELECT` with **no `WHERE` clause at all**
- `e5c33ad:src/maintenance/reembed.rs:135-140` — `needs_update = cur_len != expected_dim`
- `e5c33ad:src/bin/reembed.rs:67`, `:121-129` — status-agnostic, keys on `emb_len`
- `e5c33ad:src/tools/maintenance.rs:486ff` — `health_check_embeddings` counts `mismatched_dim` with
  **no status filter** and returns sample IDs, so the row is discoverable by the standard health command

**The division of labour is deliberate: STATUS heals MISSING embeddings, LENGTH heals WRONG ones.**
Revision 1 examined one and concluded none existed.

✅ **WHAT SURVIVES, and it is sharper than Revision 1 had it:** at
`e5c33ad:src/tools/maintenance.rs:895-902` the `'complete'` write and its "verification" are **the same
statement**. The `RETURN` projection is the **after-state of that `UPDATE`, not a read-back.** Revision 1
described it as reading the row back; it does not. **A self-referential `RETURN` cannot verify a write
no matter how it is written** — which also means Revision 1's proposed fix was aimed at the wrong lever.

➡️ **CORRECT FIX — upstream, not post-hoc:** validate `embedding.len() == dimensions()` **before** the
`UPDATE` and set `'failed'` on mismatch. `:893` currently guards only `!embedding.is_empty()` and never
the dimension, while `reembed.rs:154-161` **already `bail!`s on exactly this**. Copy that guard.

⚠️ **STRONGEST SURVIVING OBJECTION — the write-side guard is itself broken.**
`e5c33ad:src/server/schema.rs:60` defines `thoughts_embedding_idx` as `HNSW DIMENSION {dim}` **without
`OVERWRITE` / `IF NOT EXISTS`**, and `schema.rs:235` runs the whole DDL batch as `self.db.query(sql).await`
with **no `.check()` or `.take()`** — so a per-statement error such as *"index already exists at a
different DIMENSION"* is **silently swallowed**. A stale index at an old dimension would accept exactly
the vectors the client-side check rejects.

**REVISED M-1:** an **accounting and verification-integrity defect** with a transient wrong-dimension
window — **not data loss.** ⛔ The phrase *"permanently poisoned"* must not appear in any commit message
or changelog: it is false, and it teaches the next reader that `reembed` does not exist.

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
**"db no-run."** Therefore the ~~74/74~~ lib run **cannot** have exercised any of them —
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

## 4. NEW FINDINGS — adversarial pass, Revision 2

None of these appear in Revision 1. All were independently verified.

### N-1 — `remaining` is missing `GROUP ALL`; it can only ever report 0 or 1 · CONFIRMED LIVE

**Anchor:** `e5c33ad:src/tools/maintenance.rs:960`

The query has no `GROUP ALL`. Proven against the **pinned engine source**, not general lore:
`surrealdb-core-3.1.2/src/fnc/count.rs` returns a hardcoded `1` per record for zero-arg `count()`, and
`dbs/result.rs:40-41` selects the aggregating collector **only** when `stm.group().is_some()`. So
`.first()` yields `{cnt: 1}` regardless of backlog size.

**MEASURED LIVE (Codex, Studio SurrealDB 3.2.3):**

```
without GROUP ALL -> [[{cnt:1},{cnt:1},{cnt:1}]]
with    GROUP ALL -> [[{cnt:3}]]
```

Shipped code reports `remaining: 1` when the true count is 3. With 5,000 pending rows it still says 1.

⚠️ **Five sibling counts in the same file already use `GROUP ALL`** — including `:595` with a
**byte-identical predicate.** The counterexample was in the file Revision 1 was reading.
**Second site:** `admin.rs:672`, gating a `remaining_wrong == 0` success branch. Also assess `http.rs:525,530`.

### N-2 — `response.take(0)?` turns any per-row statement error into a whole-tool abort · CONFIRMED

**Anchor:** `e5c33ad:src/tools/maintenance.rs:917`

Verified against the vendored SDK: `surrealdb-3.1.2/src/method/query.rs:161-164` inserts per-statement
`Result`s **without `?`**, so `.await` returns `Ok` on statement failure; `opt/query.rs:167-171` is where
`take(0)` propagates it. The crate's own test `take_from_an_errored_query` confirms it.

So the `Err(e)` arm 24 lines below at `:941` can **only** catch *transport* errors — never the
statement-level class a SCHEMAFULL table with a dimension-constrained HNSW index actually produces.

**Consequence:** row 37 of 100 fails; rows 1–36 are already mutated; the tool returns a bare error with
**no processed/succeeded/failed/remaining report**; rows 38–100 never run. Worse, the offending row is
never marked `'failed'`, so it stays in the selection set — and `SELECT … LIMIT $limit` has **no
`ORDER BY`** (`:836-840`), so each rerun clears rows ahead of it and dies again, until the poison row
sorts first and **progress goes permanently to zero.**

`f4869de` traded *"silently counts as success"* for *"kills the run,"* skipping the correct middle
behaviour already written three lines below.

### N-3 — the SEP-2549 omission is still live on three other list methods · CONFIRMED (trigger unproven)

**Anchor:** `e5c33ad:src/server/router.rs:48`

`impl ServerHandler` overrides exactly `get_info`, `list_tools`, `call_tool`. `list_resources`,
`list_resource_templates` and `list_prompts` fall through to rmcp defaults returning `Ok(::default())` —
**not** `method_not_found` (`rmcp-3.1.4/src/handler/server.rs:378,385,394`; sibling methods in the same
macro *do* return `method_not_found`, so this is deliberate). `::default()` routes through the same
`paginated_result!` macro, leaving `ttl_ms: None, cache_scope: None`, both `skip_serializing_if`.
**No capability gate exists** — grepping all of rmcp for `supports_resources|supports_prompts` returns
zero. `supported_protocol_versions()` remains un-narrowed (`router.rs:69-73`), so `2026-07-28` is still
negotiated.

**The fix's own doc comment identifies the un-narrowed version as the root cause, then patches one of
four surfaces.** Narrowing `supported_protocol_versions()` fixes all three remaining surfaces *and* the
root, in a smaller diff than overriding three handlers.

### N-4 — the regression test cannot fail if the production wiring is reverted · CONFIRMED

**Anchor:** `e5c33ad:src/server/router.rs:322` (test) vs `:232` (wiring)

Raised in Revision 1; the adversarial pass confirmed it and found it worse. The test calls the free
function `list_tools_result(vec![tool])` directly, never constructing a `SurrealMindServer`. Reverting
`:232` reproduces the outage with the suite green.

Compounding:
- `tests/mcp_protocol.rs:147` and `tests/stdio_smoke.rs:65` both early-return unless `RUN_DB_TESTS` is
  set. CI runs plain `cargo test --workspace --locked` (`.github/workflows/ci.yml:36`) and **never sets
  it.** Both skipped — and neither asserts cache metadata anyway.
- No test anywhere negotiates `2026-07-28`; `ProtocolVersion::LATEST` is `V_2025_11_25`.
- 🔴 **`git branch -a --contains e5c33ad` returns the local branch only — no `remotes/origin/*`. The
  commit never reached GitHub. CI has never run on this code at all.**

### N-5 — `maintenance/reembed.rs` bare-ID healer is a live no-op that reports success · CONFIRMED LIVE

**Anchor:** `e5c33ad:src/maintenance/reembed.rs:163`

`UPDATE thoughts SET … WHERE id = '{id_raw}'` compares a **bare `meta::id()` string** against a record id
— **the same bug class `f4869de` just fixed** in `maintenance.rs`.

**MEASURED LIVE (Codex)** against one real thought key:

```
WHERE id = "<meta::id string>"                       -> 0 rows
WHERE id = type::record('thoughts', "<same string>") -> 1 row
```

It then increments `updated` without examining the response body, so **the helper falsely reports
success while healing nothing.**

✅ **This does NOT resurrect M-1.** `src/bin/reembed.rs:121-129` uses `UPDATE type::record('thoughts', $id)`
— record identity correct — so at least one length-based healer genuinely works. (Its response handling is
separately weak: it increments success on `Ok(response)` without `check`/`take`.) *"Permanently poisoned"
remains false; this is a distinct real defect.*

---

## 5. LIVE MEASUREMENTS — Codex, Studio SurrealDB 3.2.3, read-only

1. `INFO FOR TABLE thoughts` → `thoughts_embedding_idx = HNSW DIMENSION 1536`;
   `get_embedding_metadata()` → `self.embedder.dimensions()`, configured path 1536. **Consistent.**
2. Pending/failed count — see **N-1**.
3. `SELECT count() … array::len(embedding) != 1536 GROUP ALL` → **0**.
   ➡️ **M-1 is a MECHANISM, NOT AN OBSERVED INCIDENT.** No production row currently exhibits it and no
   matching log line was found. Unrefuted, not demonstrated.
4. Healer-ID discriminator — see **N-5**.

## 6. TWO ADVERSARIAL FINDINGS THAT WERE THEMSELVES REFUTED — do not act on these

- **"The preserved rollback binary does not exist."** **FALSE** by direct measurement:
  `~/.local/state/legacymind-rollbacks/surreal-mind/rmcp-3.1.4-preinstall-874d229/surreal-mind`,
  SHA-256 `0f7fbccb…5911c5c` exact match, mode 755. That reviewer grepped the working tree on `master`,
  and `e5c33ad`/`13943cb` are **not ancestors of HEAD**, so the deploy-branch documents naming the path
  were invisible.
  ⚠️ **Real residual:** the artifact's path is documented *only* on commits unreachable from master, so an
  operator grepping master under outage pressure finds the hash asserted with **no path attached.**
- **"No read-only test can separate the deployed binary from `f4869de`."** **FALSE** by counterexample:
  `nm` shows `__ZN12surreal_mind6server6router17list_tools_result…` as a `T` symbol, and
  `git grep list_tools_result f4869de -- src/` returns **zero hits**. (`strings` returns 0 for that symbol,
  which is exactly why Revision 1's method was blind to it.)
  ⚠️ **Residual:** proves the function is *defined*, not that the call site at `:232` was swapped.

## 7. STILL NOT ESTABLISHED

1. Whether M-1's trigger has **ever** fired historically (current rows: 0).
2. Whether SurrealDB's HNSW **rejects** off-dimension writes — decides whether M-1's mechanism is reachable
   at all given a correctly-defined index.
3. **N-3's trigger:** whether any real client issues `resources/list` against a tools-only server. Pure
   client-side behaviour; zero evidence in this tree. This is the difference between N-3 being *live* and
   *latent*, and it should not be guessed.
4. Whether the deployed binary's **call site** is swapped (§6).
5. Whether `maintain embed_pending` was ever invoked against the deployed build. Absence of a *record*, not
   evidence of non-execution.
6. **Provenance (unchanged from Revision 1):** live binary identity remains *consistent with* `e5c33ad`, not
   *established*. No `build.rs`; artifact mtime precedes the commit by 41s; Rust release builds are not
   bit-reproducible.

---

## Disposition

**Production is healthy; nothing here warrants rollback or emergency change.** M-1 is downgraded to
unobserved upstream-validation / verification-integrity hardening. N-1 and N-5 are confirmed live
correctness defects. N-2, N-3 and N-4 are independently reviewed and not yet acted on. No fix begins until
this corrected review is durable and the bounded follow-up work is defined.

**Recommended shape of the fix** — upstream, not post-hoc, because a self-referential `RETURN` cannot verify
a write: validate `embedding.len() == dimensions()` before the `UPDATE` and set `'failed'` on mismatch
(`reembed.rs:154-161` already `bail!`s on exactly this — copy that guard). Carry these one-liners in the
same commit: `GROUP ALL` at `maintenance.rs:960` and `admin.rs:672` (**N-1**); replace `response.take(0)?`
at `:917` with the per-row `Err` arm already present at `:941` **and mark the offending row `'failed'`** so
it cannot wedge the backlog at zero progress (**N-2**). Narrowing `supported_protocol_versions()` (**N-3**)
is separately scheduled.

**Method note for the next reviewer — the most transferable thing in this document.** Revision 1 was a
competent single pass and it still shipped a wrong verdict into a committed record. All three of its errors
were the *same* error: **a correctly-executed check answering a proposition adjacent to the one that
mattered.** "The count runs after the loop" ≠ "the count aggregates." "`embed_pending` cannot reach the row"
≠ "no healer can reach the row." "A peer reported 74/74" ≠ "74/74 is true." The adversarial pass did not
find these by being smarter — it found them by having three readers who could not all make the same
substitution at once.
