# MCP Tool: `update_docs` — Implementation Plan (2025-09-08)

Owner: Codex  •  Intent: Exactly match request — one tool, no flags.

## Goal
- Provide a single MCP tool `update_docs` that:
  - Reads `README.md` and `CHANGELOG.md` from the repo root.
  - Extracts deterministic entries (sections/headings, commands, releases + items).
  - Stages them to the moderation queue (visible via `memories_moderate` with status `pending`).
  - Returns a short JSON summary. No direct KG writes. No embeddings. No CI/GitHub.

## Scope (what it will do)
- Files: `README.md`, `CHANGELOG.md` only.
- Extraction (deterministic, no LLM):
  - README
    - H1–H3 sections → staged entries `{kind:"doc_section"}`
    - Command code fences labeled `bash|sh|shell|console` → `{kind:"doc_command"}`
  - CHANGELOG (Keep‑a‑Changelog)
    - Releases (semver incl. pre‑releases) + dated sections → `{kind:"changelog_release"}` and items `{kind:"changelog_entry", entry_kind: Added|Changed|Removed|…}`
- Staging target: the same “pending” queue consumed by `memories_moderate` (target="kg").
  - Write rows into candidate collections with `staged=true`, `review_state="pending"`, and `origin="update_docs"`.
  - No canonical KG tables touched; moderation approves/merges later.

## Non‑Goals
- No embeddings or hypothesis verification.
- No GitHub usage; no CI wiring.
- No generic doc ingest beyond README/CHANGELOG (can extend later).

## Data Model (staged candidate payloads)
- Common fields across all staged items:
  - `origin: "update_docs"`
  - `staged: true`, `review_state: "pending"`
  - `provenance: { path, line_from, line_to, commit_sha, project }`
  - `dedupe_key: blake3(normalized_text) + commit_sha` (used to skip duplicates)

- README section → stage to `kg_entity_candidates` with:
  - `entity_type: "doc_section"`
  - `name: <section_title>`
  - `properties: { level, slug, content_preview }`

- README command → stage to `kg_entity_candidates` with:
  - `entity_type: "doc_command"`
  - `name: <command>`
  - `properties: { section_title }`

- CHANGELOG release → stage to `kg_entity_candidates` with:
  - `entity_type: "changelog_release"`
  - `name: v<semver>`
  - `properties: { date }`

- CHANGELOG entry → stage to `kg_entity_candidates` with:
  - `entity_type: "changelog_entry"`
  - `name: <short_slug>`
  - `properties: { kind, text, release: v<semver> }`

Note: If we later want relationships (e.g., `touches` component → release), we can add `kg_edge_candidates`. For now, keep entries entity‑only per request.

## Dedupe & Idempotency
- Key: `dedupe_key = blake3(normalized_text) + commit_sha`.
- On stage: if an item with the same key exists and `review_state in {pending, approved}` for the same commit, skip.
- Normalization: lowercase, collapse whitespace, strip markdown links; keep identifiers verbatim.

## MCP Tool Surface
- Name: `update_docs`
- Params: none
- Result (JSON):
```json
{
  "staged_total": <int>,
  "readme_sections_staged": <int>,
  "readme_commands_staged": <int>,
  "changelog_releases_staged": <int>,
  "changelog_entries_staged": <int>,
  "skipped_duplicates": <int>,
  "commit_sha": "<sha>",
  "staged_ids": ["<id1>", "<id2>"]
}
```

## Implementation Steps (surgical)
1) Parser reuse
   - Reuse the deterministic parsers already in `src/ingest/markdown.rs` and `src/ingest/changelog.rs` to produce in‑memory items (no embeddings, no candidates for KG yet).

2) New tool file
   - Add `src/tools/update_docs.rs` that:
     - Finds repo root, reads files, fetches `commit_sha` via `git rev-parse HEAD`.
     - Calls the parsers to produce sections/commands/releases/entries.
     - Normalizes text and computes `dedupe_key`.
     - Stages items by writing candidate rows with required staging fields.

3) Staging writer
   - Add a small helper (new module `src/tools/staging.rs`) that writes into the candidate collections used by `memories_moderate` (target="kg"):
     - `kg_entity_candidates(id, entity_type, name, properties, staged, review_state, origin, provenance, dedupe_key, created_at)`
   - Provide upsert semantics keyed by `(dedupe_key)`.

4) Register tool
   - Wire `update_docs` into MCP tool registry so it always appears in `list_tools`.
   - No env flags; fails gracefully with actionable messages if files missing.

5) Return summary
   - Count staged vs skipped; collect IDs of staged rows; return JSON.

## Acceptance Criteria
- Calling `update_docs` returns a JSON object with non‑zero `staged_total` on a repo with content.
- `memories_moderate` shows the new items with `origin="update_docs"`, `staged=true`, `review_state="pending"`.
- Re‑running without README/CHANGELOG changes results in `skipped_duplicates > 0` and `staged_total == 0`.
- No writes to canonical KG tables; only candidate (staging) collections are affected.
- No embeddings, no LLM calls, no GitHub.

## Tests (lightweight)
- Unit: normalization and `dedupe_key` stability across whitespace/link variants.
- Integration: run `update_docs` in this repo; assert pending items increase on first run and not on second.

## Rollback
- Delete staged items by `origin="update_docs"` and `commit_sha=<sha>`:
  - `DELETE FROM kg_entity_candidates WHERE origin='update_docs' AND provenance.commit_sha='<sha>'`.
- Tool code is isolated; removal requires unregister + file delete.

## Risks & Mitigations
- Duplicate staging across tools → mitigated by `dedupe_key` and `origin` tagging.
- Parser drift → reuses existing ingest parsers to minimize new logic.
- Moderation UX load → entries are compact with provenance; can be filtered by `origin`.

## Timeline
- Implementation + wiring: ~1–2 hours.
- Tests + smoke: ~30 minutes.

