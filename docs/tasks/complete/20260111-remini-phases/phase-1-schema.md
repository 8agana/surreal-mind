# Phase 1: Schema & Data Model

**Status:** Complete
**Parent:** [remini-correction-system.md](../remini-correction-system.md)

---

## Goal

Define the data structures for marks and corrections in SurrealDB.

---

## Deliverables

- [x] Mark fields on thought/entity/observation tables
- [x] CorrectionEvent table schema
- [x] Migration scripts for existing tables

---

## Schema

### Mark Fields (added to existing tables)

```sql
-- Fields added to thoughts, kg_entities, kg_observations tables
DEFINE FIELD marked_for ON TABLE thoughts TYPE option<string>;
DEFINE FIELD mark_type ON TABLE thoughts TYPE option<string>;
DEFINE FIELD mark_note ON TABLE thoughts TYPE option<string>;
DEFINE FIELD marked_at ON TABLE thoughts TYPE option<datetime>;
DEFINE FIELD marked_by ON TABLE thoughts TYPE option<string>;
```

### CorrectionEvent Table

```sql
DEFINE TABLE correction_events SCHEMAFULL;
DEFINE FIELD id ON TABLE correction_events TYPE record<correction_events>;
DEFINE FIELD timestamp ON TABLE correction_events TYPE datetime DEFAULT time::now();
DEFINE FIELD target_id ON TABLE correction_events TYPE string;
DEFINE FIELD target_table ON TABLE correction_events TYPE string;
DEFINE FIELD previous_state ON TABLE correction_events TYPE object;
DEFINE FIELD new_state ON TABLE correction_events TYPE object;
DEFINE FIELD initiated_by ON TABLE correction_events TYPE string;
DEFINE FIELD reasoning ON TABLE correction_events TYPE string;
DEFINE FIELD sources ON TABLE correction_events TYPE array<string>;
DEFINE FIELD verification_status ON TABLE correction_events TYPE string DEFAULT "auto_applied";
DEFINE FIELD corrects_previous ON TABLE correction_events TYPE option<record<correction_events>>;
DEFINE FIELD spawned_by ON TABLE correction_events TYPE option<record<correction_events>>;
DEFINE INDEX idx_correction_events_target ON TABLE correction_events FIELDS target_id, target_table;
DEFINE INDEX idx_correction_events_timestamp ON TABLE correction_events FIELDS timestamp;
```

---

## Implementation

**Commit:** `2d4cd71` (initial) + `2bb0914` (DEFAULT fix)
**Implementer:** Vibe

### What Was Done
1. Added Mark fields to `thoughts`, `kg_entities`, `kg_observations` tables
2. Created `correction_events` table with provenance tracking
3. Added indexes for efficient querying
4. Created migration script at `migrations/phase_1_remini_schema.sql`

---

## Review (CC - 2026-01-10)

**Score:** 8/10

### Summary
Solid implementation. All required fields present with correct types. Foundation for REMini correction system is in place.

### Fix Applied
- Added `DEFAULT "auto_applied"` to `verification_status` field (commit `2bb0914`)

---

## Next Steps

Phase 1 complete. Ready for **Phase 2: rethink Tool - Mark Mode**.
