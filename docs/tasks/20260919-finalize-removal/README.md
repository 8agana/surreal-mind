# Finalize-removal typed-ID red control

Date: 2026-09-19
Branch: `codex/fed-ee4538-finalize-removal`
Base: `d28b28acc38f641670c199854c3d11cc2d9ff9b9`

## Reproduction

The project-local fixture under `fixture/` uses an in-process `SurrealKv`
database rooted in a temporary directory. It never connects to
`127.0.0.1:8000` or the production namespace/database. The original tested
scratch harness remains at `/private/tmp/fed-ee4538-delete-repro` until
PRIMARY confirms the durable artifact.

Exact command:

```text
cargo run --locked --quiet --manifest-path docs/tasks/20260919-finalize-removal/fixture/Cargo.toml
```

The fixture now carries a regenerated nested `Cargo.lock` and is pinned to
direct `surrealdb = 3.1.2`; its SurrealDB core/collections/strand/types are
also 3.1.2. Current registry metadata requires fixture `tempfile 3.27.0`
for that core; the option-A root lock now carries the same shared tempfile
upgrade plus related rustix/linux-raw-sys changes. These lock deltas are
recorded in `dependency-versions-3.1.2.txt` and are not silent.

The root lock update also raises shared default-graph `tempfile`, `rustix`,
and `linux-raw-sys` versions. The `kv-surrealkv` feature is off for the
default release graph, but this lock delta is still a production dependency
graph change and requires separate integration review.

The fixed-path control is in `fixture/src/bin/fixed.rs` and was run without
writing build artifacts into the worktree:

```text
CARGO_TARGET_DIR=/tmp/fed-ee4538-fixed-target cargo run --locked --quiet --manifest-path docs/tasks/20260919-finalize-removal/fixture/Cargo.toml --bin fixed
```

Fixed-path receipt:

```text
BEFORE=[{"id":42,"status":"removal"},{"id":"eligible","status":"removal"},{"id":"keep","status":"active"}]
CANDIDATE_COUNT=2
DELETED_ROWS=[{"created_at":"2026-08-20T04:48:06.392396Z","id":"thoughts:42","status":"removal"},{"created_at":"2026-08-20T04:48:06.389863Z","id":"thoughts:eligible","status":"removal"}]
DELETED_COUNT=2
AFTER=[{"id":"keep","status":"active"}]
```

Minimal query sequence:

```text
SELECT meta::id(id) AS id FROM thoughts
  WHERE status = 'removal'
    AND created_at < time::now() - 30d LIMIT 10;
DELETE FROM thoughts WHERE id IN $ids;       -- $ids = ["eligible"]
DELETE FROM thoughts WHERE id IN [thoughts:eligible];
```

## Raw receipts

```text
BEFORE=[{"id":"eligible","status":"removal"},{"id":"keep","status":"active"}]
CANDIDATE_IDS=["eligible"]
STRING_DELETE_RESULT=[]
AFTER_STRING=[{"id":"eligible","status":"removal"},{"id":"keep","status":"active"}]
TYPED_DELETE_RESULT=[]
AFTER_TYPED=[{"id":"keep","status":"active"}]
EXIT_CODE=0
```

This red receipt was rerun against the pinned 3.1.2 fixture lock. The fixed
control was also rerun against that same lock:

```text
BEFORE=[{"id":42,"status":"removal"},{"id":"eligible","status":"removal"},{"id":"keep","status":"active"}]
CANDIDATE_COUNT=2
DELETED_ROWS=[{"created_at":"2026-08-20T04:48:06.392396Z","id":"thoughts:42","status":"removal"},{"created_at":"2026-08-20T04:48:06.389863Z","id":"thoughts:eligible","status":"removal"}]
DELETED_COUNT=2
AFTER=[{"id":"keep","status":"active"}]
EXIT_CODE=0
```

The empty delete result arrays are SDK statement-result receipts; this report
does not claim that either `DELETE` returned rows. Independent manifests show
the string-bound query deleted zero rows while the typed-record control deleted
only `eligible`.

## Historical red verdict

**RED for the old handler:** it selects `eligible`, sets `deleted_count` to
`ids.len()` (1), ignores the string-bound DELETE statement result, and leaves
the eligible record present. The typed-record control proves the no-op is in
the query/binding path rather than a general deletion failure.

The exact command, test names, exit code, dependency identity, and matrix
scope are preserved in `matrix-receipt.md`. The current shared production
seam matrix is green under
`cargo test --locked --features test-disposable-kv --lib finalize_removal_db_tests`:
empty, valid string/numeric IDs, stale/missing, all-missing zero-row no-op,
status flip, dry-run, noncandidate survivor, and a transport-OK statement-level
DELETE error all passed with independent before/after reads. The statement
error uses a DELETE event that throws, not a parser/transport failure.
Commit/push remain **PENDING** PRIMARY review.
