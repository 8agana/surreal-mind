## 2025-11-29 - Cognitive Kernel Purification
- **Cleanup**: Removed 9 legacy photography binaries from `src/bin/` to complete the separation of concerns.
- **Cleanup**: Removed dead legacy tool handlers (`convo_think`, `tech_think`, `search_thoughts`) superseded by `legacymind_think` and `unified_search`.
- **Config**: Fixed hallucinated timeout parameters in `surreal_mind.toml`.
- **Documentation**: Created `GEMINI.md` for `surreal-mind` context tracking.

## 2025-11-24 - Photography Split Finalized
- **Single-mind codebase**: Removed all photography modules/binaries/config from SurrealMind; only the 8 core thinking tools remain.
- **Tool surface cleanup**: Removed lingering photography tool metadata from detailed_help and rebuilt/restarted service.
- **Ops relocation**: Photography CLI/ops now live in the new repo `8agana/photography-mind` (https://github.com/8agana/photography-mind).
- **Config cleanup**: Dropped `photo_*` runtime/env settings and photography DB health checks; injection scaling table no longer references photography.

## 2025-11-22 - Safety Hardening & Test Coverage
- **SQL Safety**: Replaced string-interpolated SURQL in photography commands (`update_gallery`, `list_events_for_skater`, `show_event`, `competition_stats`) with bound parameters.
- **Regression Tests**: Added unit tests for `build_update_gallery_sql` and for SkaterRequests parsing in `find_missing_skaters`.
- **Tooling Hygiene**: Repository now clippy-clean with `-D warnings`; full suite (`RUN_DB_TESTS=1 cargo test --workspace`) passes.
- **Single-mind surface**: Removed photography MCP tools from SurrealMind (one thinking surface); photography stays as ops/CLI. `list_tools` now exposes 8 core tools.

## 2025-11-20 - Photography CLI Bug Fixes & Final Polish
- **Fixed `check-status` Filtering**: Corrected a logic bug where the `--status` flag filtered by `request_status` instead of `gallery_status`. Now correctly filters for `sent`, `needs_research`, etc.
- **Fuzzy Competition Matching**: Implemented `resolve_competition` helper using Jaro-Winkler similarity (threshold 0.7) to handle competition name typos (e.g., "pony" -> "2025 Pony Express").
- **CLI Integration**: Updated all relevant commands (`check-status`, `mark-sent`, `import`, etc.) to use fuzzy resolution.
- **Import Logic**: `import_roster` now falls back to creating a new competition if no fuzzy match is found (safe for new comps).
- **UX**: `check-status` now reports the *resolved* competition name, providing clarity on what was matched.

## 2025-11-20 - Data Restoration & Import Logic Fixes
- **Data Repair**: Successfully re-imported ~200 skaters and events for "2025 Pony Express" from `SkaterRequests.md`.
- **Deduplication**:
  - Identified and merged 162 duplicate family records caused by nested ID strings.
  - Deduplicated `family_competition` edges, prioritizing "Sent/Purchased" status.
- **Integrity Verified**: Confirmed single "Sent" record for "Williams" (previously duplicated). Total unique families for Pony Express: 165.
- **Import Logic Improvements**:
  - Updated `import_roster` to capture and insert `delivery_email` for families.
  - Modified import logic to *always* create a Family record (even for single skaters) if an email is present.
  - Added automatic creation of `family_competition` edges during import.
  - Relaxed `family` schema: `primary_contact` is now `option<record<client>>`.
- **Config**: Centralized `DEFAULT_COMPETITION`.
- **Refactoring**: Updated `StatusRow` and `SkaterRow` structs to support new fields.

## 2025-09-20 - Photography Schema Extension
- ... (same as before)
