# Finalize-removal shared-seam matrix receipt

Observed on 2026-09-20 in the isolated worktree
`/Users/samuelatagan/Projects/LegacyMind/surreal-mind-wt-fed-ee4538`.

Identity:

- HEAD: `d2c93934afcf89dac13af91146d88b25059c3be4`
- Fixture dependency: `surrealdb = =3.1.2`, features `protocol-ws,kv-surrealkv`
- Root lock package identity: `surrealdb 3.1.2`, `tempfile 3.27.0`
- Test-only feature: `test-disposable-kv`

Exact command:

```text
cargo test --locked --features test-disposable-kv --lib finalize_removal_db_tests -- --nocapture
```

Observed result: exit code `0`; `2 passed`, `0 failed`, `71 filtered out`,
finished in `0.55s`.

Exact tests:

```text
tools::maintenance::finalize_removal_db_tests::typed_record_control_supports_numeric_and_string_keys
tools::maintenance::finalize_removal_db_tests::finalize_removal_matrix_uses_shared_selection_and_delete_seams
```

The matrix test covers empty, valid string/numeric IDs, stale/missing,
all-missing zero-row no-op, status flip, dry-run, noncandidate survivor, and a
transport-OK DELETE event error observed through `take_errors`, with
independent before/after reads.
