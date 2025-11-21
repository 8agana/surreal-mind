## 2025-11-20 - Fuzzy Competition Matching & UX Improvements
- **Fuzzy Matching**: Implemented `resolve_competition` helper using Jaro-Winkler similarity (threshold 0.7) to handle competition name typos (e.g., "pony" -> "2025 Pony Express").
- **CLI Integration**: Updated all relevant commands (`check-status`, `mark-sent`, `import`, etc.) to use fuzzy resolution.
- **Import Logic**: `import_roster` now falls back to creating a new competition if no fuzzy match is found (safe for new comps).
- **UX**: `check-status` now reports the *resolved* competition name, providing clarity on what was matched.

## 2025-11-20 - Data Restoration & Import Logic Fixes
- **Data Repair**: Successfully re-imported ~200 skaters and events for "2025 Pony Express" from `SkaterRequests.md` after discovering missing `competed_in` edges.
- **Import Logic Improvements**:
  - Updated `import_roster` to capture and insert `delivery_email` for families.
  - Modified import logic to *always* create a Family record (even for single skaters) if an email is present, ensuring `check-status` visibility.
  - Added automatic creation of `family_competition` edges during import.
  - Relaxed `family` schema: `primary_contact` is now `option<record<client>>` to accommodate data sources without parent names.
- **Photography TY Workflow & Schema Updates**
  - ... (same as before)