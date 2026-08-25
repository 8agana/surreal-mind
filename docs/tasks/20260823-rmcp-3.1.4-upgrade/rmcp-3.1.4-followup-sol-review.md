# rmcp 3.1.4 follow-up — Sol closure review

**Reviewer/implementer:** Codex Sol closure worker  
**Branch/worktree:** `codex/rmcp-3.1.4-followup` / isolated Studio worktree only  
**Status:** implementation and closure verification complete; final clean release candidate identity is recorded after the final build below

## Blocking disposition

| Group | Disposition | Implemented boundary |
|---|---|---|
| N-2 recurrence | Closed in source | All six KG batch UPDATE paths in `run_reembed_kg` and `run_kg_embed` now distinguish transport failure, statement failure (`take(0)`), zero match, and verified success. They continue per-row and report separate `*_failed` and `*_no_match` counters. |
| Generated-vector correctness | Closed in source | `ensure_generated_embedding_dimension` is the sole pre-mutation guard used by thought, KG, pending, admin, and re-embed writers. Thought/KG/admin/re-embed UPDATEs inspect `RETURN` rows before reporting success. |
| Negative tests | Closed | Pure guard/identity/classifier tests cover wrong length, dirty-state serialization, statement error, and zero match. The real-driver classifier test passed in `sol_closure_classifier_20260825`; the real stale-index normal-failure/bypass-success test passed in `sol_closure_schema_20260825`; the complete 8-test hygiene suite passed in `sol_closure_full_20260825`. Each namespace was removed and absent from `INFO FOR ROOT` afterward. |
| Provenance | Closed in source | `build.rs` watches relevant tracked source inputs as well as refs, accepts Git metadata only when canonical Git top-level equals `CARGO_MANIFEST_DIR`, and serializes clean, dirty, dirty-unknown, and unknown commit explicitly. The fixture script covers clean, dirty, cleared, source archive, and archive nested under unrelated Git. |
| Emergency bypass | Closed in source | `SURR_SKIP_DIM_CHECK` now gates the schema index-dimension verification inside `SurrealMindServer::new()` as well as main's later preflight. Normal startup still rejects a real stale HNSW dimension. |

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
- `scripts/verify-build-provenance-fixture.sh` passed after initially catching a real clean-status serialization defect; the corrective commit is `012dc5d`.
- No source archive, fixture directory, or disposable namespace was retained after its witness completed.
