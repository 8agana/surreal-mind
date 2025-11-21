## 2025-11-20 - Data Restoration & Import Logic Fixes
- **Data Repair**: Successfully re-imported ~200 skaters and events for "2025 Pony Express" from `SkaterRequests.md` after discovering missing `competed_in` edges.
- **Import Logic Improvements**:
  - Updated `import_roster` to capture and insert `delivery_email` for families.
  - Modified import logic to *always* create a Family record (even for single skaters) if an email is present, ensuring `check-status` visibility.
  - Added automatic creation of `family_competition` edges during import.
  - Relaxed `family` schema: `primary_contact` is now `option<record<client>>` to accommodate data sources without parent names.
- **Photography TY Workflow & Schema Updates** (Previous entry)
  - ... (same as before)
