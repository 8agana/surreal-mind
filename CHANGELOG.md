## 2025-11-20 - Implement Fuzzy Competition Name Matching in Photography CLI
- **Added fuzzy competition name resolution**: New `resolve_competition` helper in `utils.rs` using exact, substring, and Jaro-Winkler fuzzy matching (score >0.8)
- **Updated commands for fuzzy matching**: check_status, import_roster, mark_sent, request_ty, send_ty, record_purchase, list_events, competition_stats, set_status, list_events_for_skater
- **Backward compatible**: Exact matches prioritized, substring matches supported, fuzzy for typos (e.g., "fal fling" → "2025 Fall Fling")
- **Error handling**: Clear messages for ambiguous or no matches, with available options listed

## 2025-11-20 - Data Restoration & Import Logic Fixes
- **Data Repair**: Successfully re-imported ~200 skaters and events for "2025 Pony Express" from `SkaterRequests.md` after discovering missing `competed_in` edges.
- **Import Logic Improvements**:
  - Updated `import_roster` to capture and insert `delivery_email` for families.
  - Modified import logic to *always* create a Family record (even for single skaters) if an email is present, ensuring `check-status` visibility.
  - Added automatic creation of `family_competition` edges during import.
  - Relaxed `family` schema: `primary_contact` is now `option<record<client>>` to accommodate data sources without parent names.
- **Photography TY Workflow & Schema Updates** (Previous entry)
  - ... (same as before)
