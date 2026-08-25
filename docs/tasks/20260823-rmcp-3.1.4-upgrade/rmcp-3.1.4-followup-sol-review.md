# rmcp 3.1.4 follow-up — Sol closure review

**Reviewer/implementer:** Codex Sol closure worker
**Branch/worktree:** `codex/rmcp-3.1.4-followup` / isolated Studio worktree only
**Status:** re-review implementation and full verification complete. The final clean release-candidate identity belongs in the external closure receipt: committing a hash into this file would create a different commit and therefore a different embedded identity.

## Blocking disposition

| Group | Disposition | Implemented boundary |
|---|---|---|
| N-2 recurrence | Closed | Every one of the six KG batch UPDATE sites calls the same `execute_embedding_update` helper, which owns transport, statement, zero-match, and success classification. Per-row continuation and `*_failed`/`*_no_match` accounting are therefore one tested path. |
| Generated-vector correctness | Closed | `ensure_generated_embedding_dimension` is the sole pre-mutation guard used by thought, KG, pending, admin, and re-embed writers. A fake-embedder disposable test invokes the active `ensure_kg_embedding` write path, proves wrong length returns an error, and proves its record remains unmodified. |
| Negative tests | Closed | Pure guard/identity/classifier tests cover wrong length, dirty-state serialization, transport, statement error, and zero match. The real-driver classifier continuation and active KG wrong-dimension tests passed in `sol_rereview_paths_20260825`, which was removed and absent from `INFO FOR ROOT` afterward. |
| Provenance | Closed | `build.rs` watches every tracked file as well as refs, accepts Git metadata only when canonical Git top-level equals `CARGO_MANIFEST_DIR`, and serializes clean, dirty, dirty-unknown, and unknown commit explicitly. The passing fixture covers source and non-source dirty/cleared changes, unavailable Git, source archive, and nested ancestor archive. |
| Failure reporting | Closed | `failed` reaches MCP `reembed`, every `reembed_kg` MCP table block, and both KG CLI summaries. Pure summary tests fail if any failure field is omitted. |
| Emergency bypass | Closed | `SURR_SKIP_DIM_CHECK` now gates the schema index-dimension verification inside `SurrealMindServer::new()` as well as main's later preflight. Normal startup still rejects a real stale HNSW dimension; the test serializes its process-global environment mutation. |

## Provenance boundary

`--version` is build metadata, not reproducible binary hashing and not a deployment claim. A measured artifact SHA-256 plus an external deployment receipt is required to bind a commit to a deployed binary. The previous changelog wording that made unknown metadata appear as a bare clean package version is superseded.

## Explicit deferrals

- **N-3:** no protocol behavior change. Required next witness is a real isolated client trace for the three list methods.
- **N-4:** no CI push or CI hard assertion/wiring change.
- **M-2:** angle-bracket record-key normalization remains a follow-up.
- **Total-order tie-breaker:** remains a follow-up.

## Non-mutation boundary

No production install, launchd/tunnel/port-8787 change, live database write, peer notification, or CI push is authorized by or performed for this follow-up. Runtime database tests must use a freshly created disposable namespace and remove/recheck it after execution.

## Executed closure evidence

- `cargo fmt --check`, default and `db_integration` all-target `cargo check --locked`, focused unit negatives, full default `cargo test --workspace --locked`, and all-feature Clippy with `-D warnings` passed on Studio.
- `scripts/verify-build-provenance-fixture.sh` passed after initially catching a real clean-status serialization defect; the corrective commit is `012dc5d`. The re-review rerun also passed the non-source dirty/cleared and unavailable-Git cases.
- No source archive, fixture directory, or disposable namespace was retained after its witness completed.
