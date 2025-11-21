## 2025-11-21 - Server Refactoring & Thinking Tool Optimization
- **Server Modularization**: Split the monolithic `src/server/mod.rs` into focused modules:
  - `src/server/schema.rs`: Database schema definitions and initialization.
  - `src/server/db.rs`: Database connection and core operations.
  - `src/server/router.rs`: Request routing and `ServerHandler` implementation.
- **Code Deduplication**: Refactored `src/tools/thinking.rs` to use a new `ThoughtBuilder` pattern, unifying logic between `run_convo` and `run_technical`.
- **Documentation**: Consolidated architectural context into `AGENTS.md` and removed redundant `GEMINI.md`.
- **Verification**: Validated library and binary compilation on remote host `studio`.

## 2025-11-20 - Data Reconciliation & Cleanup Complete
- **Data Repair**: Successfully re-imported ~200 skaters and events for "2025 Pony Express" from `SkaterRequests.md`.
- **Deduplication**:
  - Identified and merged 162 duplicate family records caused by nested ID strings (e.g., `family:family:name` merged into `family:name`).
  - Deduplicated `family_competition` edges, prioritizing "Sent/Purchased" status over "Pending" to preserve historical data.
- **Integrity Verified**: Confirmed single "Sent" record for "Williams" (previously duplicated). Total unique families for Pony Express: 165.
- **Import Logic Improvements**:
  - Updated `import_roster` to capture and insert `delivery_email` for families.
  - Modified import logic to *always* create a Family record (even for single skaters) if an email is present.
  - Added automatic creation of `family_competition` edges during import.
  - Relaxed `family` schema: `primary_contact` is now `option<record<client>>`.
- **Fuzzy Competition Matching**: Implemented `resolve_competition` helper using Jaro-Winkler similarity.
- **CLI Integration**: Updated all relevant commands to use fuzzy resolution.
- **Config**: Centralized `DEFAULT_COMPETITION`.
