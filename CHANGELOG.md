## [Unreleased]

### Added
- (2025-12-20) Implemented `memories_populate` tool: Processes unextracted thoughts via Gemini CLI to populate knowledge graph, with session persistence, auto-approval, and batch tracking. Includes schema updates, session management, and integration with existing KG tables.
- (2025-12-19) Added `curiosity_add`, `curiosity_get`, `curiosity_search` tools for lightweight note-taking with semantic search.

### Fixed
- (2025-12-23) Updated `detailed_help` documentation for `legacymind_think` to accurately reflect its return structure (flat JSON, not nested) and clarify that framework analysis is DB-only.
- (2025-12-19) Fixed `recency_days` parameter in search tools - was being ignored, now properly filters by date.

### Changed
- (2025-12-23) Database migration: Updated 552 thoughts from `extracted_to_kg = NONE` to `extracted_to_kg = false` to make them eligible for memories_populate processing.

### Known Issues
- (2025-12-23) `memories_populate` returns serialization error "invalid type: enum" - rmcp 0.11 requires all output schema fields in every return path. Partial fix applied (commit 187be2b) but additional return paths need updating.