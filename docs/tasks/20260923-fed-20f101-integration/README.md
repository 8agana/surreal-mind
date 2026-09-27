# fed-20f101 isolated integration candidate

The reviewed candidate was later deployed to Studio's HTTP service on
2026-09-27. See [deployment-20260927.md](deployment-20260927.md) for the
running binary identity, health checks, and rollback path. The assembly
account below records the earlier pre-promotion state.

This candidate was assembled on Studio in the new
`codex/fed-20f101-integration` worktree from
`master` at `d28b28acc38f641670c199854c3d11cc2d9ff9b9`. The canonical
`master` checkout was not advanced; its pre-existing `.serena/project.yml`
edit was left alone. No service, deployed binary, production database, or
Topaz process was changed. Promotion requires PRIMARY inspection and CC B2
combined review.

## Ordered source history

Each source ref matched its live `origin` tip before integration. The merge
column records the first-parent merge commit in this candidate.

| # | Source ref | Source tip | Merge commit |
|---|---|---|---|
| 1 | `codex/fed-acca0a-env-policy` | `f658c715820ca9b96336e61dbfc48658148dbb79` | `522f2df8b3348fd0a3f557e9a2ef8c8994b00f69` |
| 2 | `codex/fed-70cb9c-kg-populate-retirement` | `ada507db5f14cc715b8d0baa50a4d58841504361` | `f2b357e87a959578c4b4298eb68b853a11def358` |
| 3 | `cc/fed-61d26d-codexclient-audit` | `679a101cdbd82408b2bc9af8b60969a0d96ded5e` | `256cfb29cfd321537243cd802ce169f861c10282` |
| 4 | `codex/fed-4b1bab-surreal-cli-errors` | `bcfac56377ee9b52bb7348288e5ceeaa07d3f310` | `4a0bf57cee4d801209cdf7f8538cd4af878f54d4` |
| 5 | `codex/fed-451d12-maintenance-subprocess-controls` | `5702d75347e4a0f1d191b5e8c33066536da157e9` | `769238d463239e31f1e09740b6dc3b94923b5190` |
| 6 | `cc/fed-4d3d9d-schema-truthfulness` | `3d25be07df34eb1a8b5a33ab303a9085882538dd` | `8c63dcb2ad0cb720283fe343f1dcf156bec03cba` |
| 7 | `cc/fed-c5394b-statement-results` | `db600bd27c2f439f4497121f83ea42784269f909` | `e0772811676bd0e885bdab88f75a04a5e68574ed` |
| 8 | `codex/fed-ee4538-finalize-removal` | `87d94a2acfdde5ea02d0f4b044bc2d429f4f2d0e` | `620da55581ad469dbacfbce8e19d96db455050d5` |
| 9 | `cc/fed-b9798d-audit-remediation` | `cf8e0e0820d895f40bfa4d34df620dbd9e59407e` | `4f43907f4fc5ab25035428e9973e6b857f9c23a1` |

The first-parent follow-up commits are `a651b811` (Rust formatting),
`b84ecc7c` (lock reconciliation), `9e16c3fb` (blocking audit CI gate), and
`b4dbe4ff` (SurrealDB CLI prompt compatibility). `02780ea` and `114716c`
REMini fixes and `9820354` CI repair remain ancestors through the base.
The unrelated inner-voice branch was excluded.

## Conflict decisions

- Merges 2 through 5: retained both sides of their appended `CHANGELOG.md`
  entries. Merge 6 and statement-results merge 7 were clean.
- Merge 8: combined maintenance subprocess supervision with the truthful
  DDL and typed `finalize_removal` functions, imports, handlers, unit tests,
  and the separate `ddl_classification_tests` and
  `finalize_removal_db_tests` modules. Retained the source fixture and red
  control docs. Kept `crc32fast` 1.5.2 and `snap` 1.1.2 in `Cargo.lock`.
- Merge 9: applied the audit branch's updated Windows dependency edges
  while preserving the SurrealKv 0.21.4 and `lz4_flex` test graph. Locked
  offline Cargo metadata pruned only the orphaned `mac` 0.1.1 lock entry.
  The final lock contains 630 packages, including SurrealKv 0.21.4.
- The combined audit exited 0 with the four justified ignores in
  `.cargo/audit.toml`: RUSTSEC-2026-0185, RUSTSEC-2026-0037,
  RUSTSEC-2026-0235, and RUSTSEC-2023-0071. A pre-integration lockfile
  negative control exited 1 on 12 non-allowlisted vulnerabilities. Only
  then was `continue-on-error` removed from the CI audit step.

## CLI compatibility change requiring CC review

Studio's installed SurrealDB CLI 3.2.3 emits
`namespace/database> JSON\n\nnamespace/database> ` even with
`--json --hide-welcome`. A disposable in-memory probe captured its **raw**
stdout, stderr, exact argv, version, and CLI help in `sql-probe/` in the
receipt package. The separate `b4dbe4ff` commit accepts only that exact
framing pair or the older bare JSON shape. It does not scan or replace
payload substrings. Literal prompt text inside JSON is preserved, while
missing prompts, trailing nonprompt text, malformed JSON, SurrealQL error
JSON with CLI exit 0, nonzero CLI exit, and stderr remain failures. CC should
review this added production health parser logic independently of the nine
accepted source branches.

## Verification and raw receipts

Raw logs, the binary diff, SHA-256 manifest, and self-contained Git bundle
are packaged separately in the handoff directory. The original Studio
receipt directory is
`/Users/samuelatagana/Projects/LegacyMind/fed20f101-receipts-20260923`.
All compilation used one Cargo job and disabled incremental compilation to
limit disk and memory contention with Topaz.

| Check | Result | Receipt |
|---|---|---|
| `cargo fmt -- --check` on final content | pass | `17-final-fmt.log` |
| `cargo check --locked` on final content | pass | `18-final-check.log` |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` on final content | pass | `19-final-clippy.log` |
| `cargo test --workspace --locked` on final content | pass; DB/model opt-ins unset | `20-final-test.log` |
| `cargo build --release --locked` in isolated worktree | pass | `15-cargo-build-release.log` |
| `cargo test --locked --lib ddl_classification_tests` | 5 passed | `08-ddl-classification.log` |
| `cargo test --locked --features test-disposable-kv --lib finalize_removal_db_tests` | 2 passed | `09-finalize-removal-db.log` |
| disposable `scripts/test_db.sh --locked --test mcp_protocol test_maintain` | 3 passed | `10-protocol-maintain-disposable.log` |
| `kg_populate` disposable/fake-provider response controls | 9 safe cases passed | `12-kg-populate-empty-object-fixed.log`, `13-kg-*.log`, `14-kg-*.log` |
| disposable health/REMini contract and prompt framing positives/negatives | all assertions passed | `16-health-contract.log` |
| combined `cargo audit --file Cargo.lock` | exit 0 | `04-cargo-audit-positive.log` |
| pre-integration lock audit negative control | exit 1 | `05-cargo-audit-negative.log` |

The first locked check failed before compilation because the manually merged
lock still included an orphaned package. Its raw failure is retained as
`01-cargo-check-lock-mismatch.log`. The initial `kg_populate` control could
not parse the SurrealDB 3.2.3 CLI prompt; its raw failure is
`11-kg-populate-empty-object.log`. Both were resolved and rerun above.
The `entity` response fixture was not run because it may invoke embedding;
the nine run cases use no live model calls. DB-backed tests used disposable
SurrealKv tempdirs or an owned in-memory server. Default workspace DB tests
ran with `RUN_DB_TESTS` unset; the targeted protocol wrapper used an isolated
namespace and fake embedder.
