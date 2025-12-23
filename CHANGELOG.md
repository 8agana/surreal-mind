## [Unreleased]

### Added
- Implemented `memories_populate` tool: Processes unextracted thoughts via Gemini CLI to populate knowledge graph, with session persistence, auto-approval, and batch tracking. Includes schema updates, session management, and integration with existing KG tables.

### Fixed
- Updated `detailed_help` documentation for `legacymind_think` to accurately reflect its return structure (flat JSON, not nested) and clarify that framework analysis is DB-only.