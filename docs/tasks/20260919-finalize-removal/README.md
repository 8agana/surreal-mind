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
cargo run --quiet --manifest-path docs/tasks/20260919-finalize-removal/fixture/Cargo.toml
```

The exact raw run used a generated scratch `Cargo.lock` under the temporary
harness. The project-local fixture intentionally carries no lockfile, so
dependency resolution may drift until a future acceptance gate pins it.

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

The empty delete result arrays are SDK statement-result receipts; this report
does not claim that either `DELETE` returned rows. Independent manifests show
the string-bound query deleted zero rows while the typed-record control deleted
only `eligible`.

## Verdict

**RED for the old handler:** it selects `eligible`, sets `deleted_count` to
`ids.len()` (1), ignores the string-bound DELETE statement result, and leaves
the eligible record present. The typed-record control proves the no-op is in
the query/binding path rather than a general deletion failure.

Implementation, remaining positive/stale/failure matrix, and push remain
**PENDING** PRIMARY review; this evidence artifact is now being committed
locally on the isolated branch.
