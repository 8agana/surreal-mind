## [0.1.2] - 2026-01-02

### Fixed
- **kg_populate**: Initialize `embedding` field to NONE when creating kg_entities, kg_edges, kg_observations, and kg_boundaries. Previously records were created without the field, preventing kg_embed from finding them.
- **kg_embed**: Enhanced WHERE clauses to include `embedding IS NOT DEFINED` condition as fallback for pre-existing records created without embedding fields.

## [Unreleased]

### Added
- (2026-01-01) kg_populate run successful: 904 thoughts processed (Session 3) + 36 more (Session 4) = 940 total, 97.8%+ success rate. Knowledge graph extraction working: 660+ entities, 799+ edges, 1290+ observations, 259+ boundaries created. Shell aliases added: `kgpop` (kg_populate runner), `kgembed` (re-embedding workflow).
- (2025-12-20) Implemented `memories_populate` tool: Processes unextracted thoughts via Gemini CLI to populate knowledge graph, with session persistence, auto-approval, and batch tracking. Includes schema updates, session management, and integration with existing KG tables.
- (2025-12-19) Added `curiosity_add`, `curiosity_get`, `curiosity_search` tools for lightweight note-taking with semantic search.

### Fixed
- (2025-12-26) Refactored `memories_populate` update logic to use native `db.update().merge()` SDK method instead of raw SQL queries. This definitively resolves record ID binding issues (UUID vs String) that were causing silent update failures.
- (2025-12-25) `memories_populate` now returns fully structured MCP output (no RawContent paths), records `extracted_at` and `thought_ids`, and defaults Gemini CLI to `gemini-3-pro-preview`; parsing now strips code fences and surfaces stdout snippets on error. Workspace fmt/clippy/tests all passing.
- (2025-12-24) Cleared clippy `collapsible_if` and `unnecessary_unwrap` across knowledge_graph, maintenance, http, binaries (smtop, reembed_kg, kg_dedupe_plan) and tests; workspace now clippy-clean with full test suite passing.
- (2025-12-23) Updated `detailed_help` documentation for `legacymind_think` to accurately reflect its return structure (flat JSON, not nested) and clarify that framework analysis is DB-only.

### Removed
- (2025-12-30) Removed `inner_voice` tool and all supporting code, tests, scripts, and documentation. The tool's retrieval + synthesis + auto-extract workflow has been replaced by `legacymind_search` + `delegate_gemini` combinations. Removed environment variables: `SURR_ENABLE_INNER_VOICE`, `SURR_DISABLE_INNER_VOICE`, `SURR_INNER_VOICE_*`, `SURR_IV_*`, `IV_ALLOW_GROK`, `INNER_VOICE_LOCAL_FALLBACK`. Removed Cargo dependencies: `blake3`, `unicode-normalization`. Removed scripts: `scripts/iv_extract.js`, `lib/iv_utils.js`. Updated tool roster to 9 tools.
- (2025-12-19) Fixed `recency_days` parameter in search tools - was being ignored, now properly filters by date.

### Changed
- (2025-12-23) Database migration: Updated 552 thoughts from `extracted_to_kg = NONE` to `extracted_to_kg = false` to make them eligible for memories_populate processing.

### Known Issues
- (2025-12-25) None currently known. Monitor `memories_populate` on next live run to confirm `extracted_at` stamping persists.