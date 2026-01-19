# Auto‑Ingest README/CHANGELOG → KG (surreal‑mind)

Owner: Codex • Date: 2025‑09‑07

Goal: When `README.md` or `CHANGELOG.md` changes, extract structured information and upsert it into SurrealDB as first‑class knowledge (with provenance and versioning), without breaking existing flows.

## Scope
- Parse `README.md` (sections, commands, architecture) and `CHANGELOG.md` (Keep‑a‑Changelog semantics).
- Create/refresh doc metadata tables and produce KG candidates (entities/edges) for moderation → promotion.
- Automate via GitHub Actions path filters; optional local `pre-push` hook.

## Non‑Goals
- No direct writes to `kg_entities`/`kg_edges` without moderation.
- No LLM‑heavy extraction in CI; deterministic extraction only (heuristics optional + bounded).

## Design Overview
- Binary: `src/bin/ingest_repo_docs.rs` (alias `sm_ingest_docs`).
  - Reads repo root (default `.`) + file selectors `--readme --changelog`.
  - Loads config via `Config::load()` and uses the existing Surreal client conventions.
  - Emits both: (1) structured doc records, (2) candidate KG rows for review.

### Hypothesis Verification Integration (doc_claims)
- Purpose: Make README/CHANGELOG changes immediately verifiable by the Phase C hypothesis tool.
- New table `doc_claims` stores deterministic, atomic statements derived from docs.
  - Fields: `{ id, source_type: 'readme'|'changelog', source_id, release_id?, commit_sha, claim_text, normalized_text, blake3_hash, embedding, embedding_model, embedding_dim, created_at, verification? }`
  - Vector index on `embedding` (dim-safe; same model/dim as KG: 1536 by default).
- Deterministic claim extraction (no LLM):
  - README: sentences under Architecture/Modules/Tools/Config/Usage headings matching patterns like `X is|supports|requires ...`.
  - CHANGELOG: map kinds to claims: Added→"X exists"; Removed→"X no longer exists"; Deprecated→"X is deprecated"; Changed→"P of X changed in vY"; Fixed/Security→"Issue I in X is fixed in vY".
- CLI additions:
  - `--claims-only` to extract + persist doc_claims (no KG candidates).
  - `--verify-claims` to run Phase C verification over new/changed doc_claims; write compact `verification{confidence, matched_support, matched_contradict, k, min_similarity, time_ms}` onto each claim row.
  - `--min-sim <0..1>` (default 0.5), `--verify-top-k <n>` (default 200), `--evidence-limit <n>` (default 5).
- Parsers:
  - README: CommonMark → headings H1–H3 into `doc_section`; capture code fences labeled `bash`, `sh`, `console` as `command` items.
  - CHANGELOG: semver blocks with date; recognize sections: Added/Changed/Deprecated/Removed/Fixed/Security/Other.
- Idempotency:
  - Stable IDs using `project_slug:file:slug` + `commit_sha` + `content_hash`.
  - Upsert on `(doc_id, section_slug)` with latest content; preserve historical snapshots keyed by commit SHA.
- Provenance:
  - Store `repo`, `path`, `commit_sha`, `author`, `committed_at`, `line_start..line_end`, `ingestor_version`.
- Moderation path:
  - Insert into `kg_entity_candidates`/`kg_edge_candidates` with `status='pending'`, `confidence` from deterministic rules.
  - Human approval via existing `knowledgegraph_moderate` tools promotes to canonical tables.

## Minimal Schema Additions (safe, isolated)
- `doc_documents`: `{ id, project, path, kind: 'readme'|'changelog', latest_sha, created_at }`
- `doc_sections`: `{ id, doc_id, slug, title, level, content, content_hash, commit_sha, line_from, line_to, created_at }`
- `releases`: `{ id: 'v<semver>', semver, date, commit_sha, created_at }`
- `changelog_entries`: `{ id, release_id, kind, text, content_hash, created_at }`
- Edges (as edge tables or RELATE records if preferred later) are not required initially; linkage done via IDs.

Indexes (performance)
- `DEFINE INDEX doc_lookup ON doc_sections FIELDS doc_id, slug, commit_sha;`
- `DEFINE INDEX changelog_lookup ON changelog_entries FIELDS release_id, kind;`
- `DEFINE INDEX doc_claims_embed_vector ON doc_claims FIELDS embedding TYPE hnsw DIM 1536;` (keep DIM in sync with SURR_EMBED_MODEL dims)
 - `DEFINE INDEX doc_claims_by_source ON doc_claims FIELDS source_id;`
 - `DEFINE INDEX doc_claims_by_release ON doc_claims FIELDS release_id;`

Timestamps & auditing
- Add `created_at` and `modified_at` to all new tables; update `modified_at` on upsert for cheap auditing.
- Use a stable hash prefix (e.g., `claim:`) when computing `blake3_hash` to prevent cross-domain collisions.

Notes
- These tables do not alter existing KG behavior and can be purged/replayed from git history.

## Candidate Generation Rules (deterministic)
- README
  - Section titles → `kg_entity_candidates(name=title, entity_type='doc_section')` with provenance to `doc_sections.id`.
  - Code blocks with shell commands → `kg_entity_candidates(name=command, entity_type='command')`.
  - Architecture/Modules headings containing keywords (e.g., "server", "tool", "module", "schema") → `kg_entity_candidates(..., entity_type='component')`.
  - Simple edges: `component --rel:part_of--> project('surreal-mind')` as `kg_edge_candidates`.
- CHANGELOG
  - Each release → `kg_entity_candidates(name='surreal-mind', entity_type='project')` upsert; ensure existence.
  - Each entry → `kg_entity_candidates(name='<short slug>', entity_type=entry.kind)` and
    `kg_edge_candidates(rel_type='affects', source=entry, target=release)`.
  - Added/Removed entries mentioning a component → `rel_type='touches'` between component and release.

Confidence (configurable)
- Defaults (env‑tuned):
  - Headings/components: `${SURR_INGEST_CONFIDENCE_HEADING:=0.65}`
  - Commands: `${SURR_INGEST_CONFIDENCE_COMMAND:=0.80}`
  - Changelog entries: `${SURR_INGEST_CONFIDENCE_CHANGELOG:=0.75}`
  - Edge adjustments: `+0.05` for explicit mentions; `-0.10` for heuristic inferences (min 0.4).
  - If a claim’s verification.confidence ≥ 0.70: apply `+0.05` bonus; if contradicting≥1: apply `-0.10` penalty.
  - Extra envs: `${SURR_INGEST_CONFIDENCE_VERIFICATION_BONUS:=0.05}`, `${SURR_INGEST_CONFIDENCE_CONTRADICT_PENALTY:=0.10}`.

Confidence
- Headings/components: 0.65
- Commands: 0.80
- Changelog entries: 0.75
- Edges derived from explicit mentions: +0.05; from heuristics: −0.10 (min 0.4)

## CLI Contract (`sm_ingest_docs`)
- Args
  - `--root <path>` default `.`
  - `--readme` / `--changelog` (default: both if present)
  - `--commit <sha>` (fallback to `git rev-parse HEAD`)
  - `--dry-run` (print plan; no DB writes)
  - `--json` (emit structured summary to stdout)
- Env/Config
  - Reuse existing: DB URL/NS/DB and auth from `Config::load()` and `.env`.
  - Multi-repo: supply `--project <slug>` (default `surreal-mind`) for provenance and IDs.
- Output
  - Summary counts: sections parsed, candidates upserted, skipped (unchanged hash), errors.
  - Claims pipeline: `claims_extracted`, `claims_deduped`, `claims_verified`, `support_hits`, `contradict_hits`.

Batching & Recovery
- Flags: `--batch-size <n>` (default 100), `--continue-on-error` (best‑effort mode with per‑item error logging), `--max-retries <n>` (default 2 with backoff).
- Rationale: avoid DB overload and allow partial success in CI.
 - UX: `--progress` prints periodic counters locally. `--version` prints ingestor version.
 - Metrics: optional `--prometheus` prints counters in Prometheus text format to stdout.

## CI: GitHub Actions
- Trigger
  - `on: push: paths: [ 'README.md', 'CHANGELOG.md' ]`
- Steps
  - `actions/checkout@v4`
  - `dtolnay/rust-toolchain@stable`
  - `cargo build --release --bin ingest_repo_docs`
  - `./target/release/ingest_repo_docs --root . --readme --changelog --commit $GITHUB_SHA`
- Secrets
  - `SURR_DB_URL`, `SURR_DB_NS`, `SURR_DB_DB`, `SURR_DB_USER`, `SURR_DB_PASS`
- Safeguards
  - Retries with backoff on DB unavailability; exits non‑zero on persistent failure.
  - Use `--batch-size 100 --continue-on-error` to favor progress under load.
  - Parse `--json` with `jq` for a step summary; never echo secrets.
  - Prefer GitHub Environment secrets.

## Optional: Local `pre-push` Hook
- Detect staged changes to README/CHANGELOG; run `sm_ingest_docs --dry-run` for quick sanity then full ingest.
- Skip if no network or secrets missing.

Opt‑in via git config
- Respect `git config --bool hooks.ingestDocs` (true enables the hook); default off.
 - Add a 30s timeout so pushes don’t hang; skip on timeout.

## Versioning Strategy
- Project uses SemVer for releases (`vX.Y.Z` tags).
- CHANGELOG entries must reference a tagged version; `releases` row created/updated on ingest.
- Document snapshots stored per commit; latest pointer on `doc_documents.latest_sha`.
- KG promotion does not change historical doc rows; only candidate status evolves.
- Ingestor versions as `ingestor_semver` (start at `0.1.0`) to enable deterministic replays.
 - Pre‑release support: accept semver pre‑releases (e.g., `v1.0.0‑alpha.1`) and normalize IDs to `v<semver>`.
 - Binary exposes `--version`; telemetry includes `ingestor_version` for run attribution.

## Acceptance Criteria
- Editing README heading updates exactly one `doc_sections` row and zero/one candidate rows (idempotent by hash).
- Adding CHANGELOG `v1.2.3` with items creates `releases('v1.2.3')` and `changelog_entries[...]` with correct kinds.
- `knowledgegraph_moderate` can see new candidates and promote them without schema errors.
- CI job runs only on README/CHANGELOG changes; logs omit secrets; failures are actionable.
 - For a changelog entry “Deprecated component Foo” in `v1.2.3‑beta.1`:
   - Creates `releases('v1.2.3‑beta.1')` row.
   - Produces claim “Foo is deprecated.” with embedding and verification summary.
   - Creates entity candidate (component Foo, tag deprecated) and edge candidate `deprecated_in → release` with provenance.
- Running with `--verify-claims --min-sim 0.5` yields non‑zero `total_candidates` in telemetry and stable dim/provider metadata.
 - `--verify-claims` processes only new/changed claims by default (hash+commit) with an override `--all-claims` to re-verify everything.

## Migration / Rollback
- Forward: create new tables; no impact to existing.
- Backward: drop `doc_*`, `releases`, `changelog_entries` tables; no change to `kg_*`.
- Rebuild: replay from git history using `--from-tag <vX.Y.Z>` (future option) if we add it.

## Tasks & Sequencing
1) Create schemas (SurrealQL) and add lightweight tests (idempotency checks). (0.5d)
2) Implement parsers (README CommonMark, CHANGELOG) and hashing. (0.5–1d)
3) Implement DB upsert layer and provenance stamping. (0.5d)
4) Wire candidate generation with confidence rules and moderation path. (0.5d)
5) Add CLI flags, `--dry-run`, and JSON summary. (0.25d)
6) Add GitHub Action workflow and secrets doc. (0.25d)
7) Optional pre‑push hook. (0.25d)
8) Dry‑run on current repo; verify acceptance criteria. (0.25d)

Timeline adjustment
- With verification wiring, batching, indexes, and CI, expect 4–5 days including tests and docs. Parallelize schema + parser work to hold schedule.

Open items from CC (tracked)
- Markdown link extraction (optional): capture `[text](url)` and stage `rel_type='links_to'` edge candidates from section → URL target (trusted domains only, allowlist configurable). Default off; enable with `--extract-links`.
- Performance: ensure indexes above are created; validate batch timings in CI logs.

## Open Questions
- Do we want `component` as a first‑class table now or keep components as `kg_entities` only after promotion? (Recommend: keep as candidates until promoted.)
- Should README commands be normalized (dedupe flags, collapse aliases) before candidate creation? (Default: no; preserve literal.)

## Notes on Existing Code Reuse
- Reuse `src/kg_extractor.rs` only as a secondary heuristic pass; primary extraction remains deterministic parsers.
- Reuse DB config and client patterns from `src/server` and `src/utils/db.rs`.
- Align with tool visibility rule: no gating of tools; gating happens in handler logic only.
 - Parser module (new): `src/ingest/mod.rs` with traits for `MarkdownParser` and `ChangelogParser` to enable unit tests without DB.
