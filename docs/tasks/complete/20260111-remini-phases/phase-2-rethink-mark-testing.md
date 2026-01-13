# Phase 2: rethink Tool - Mark Mode - Testing

**Status:** PENDING RETEST - RETURN NONE + scalar count() existence fix applied
**Parent:** [phase-2-rethink-mark.md](phase-2-rethink-mark.md)
**Depends On:** Phase 2 Implementation Complete

---

## Goal

Verify the `rethink` tool mark mode works correctly before proceeding to Phase 3.

---

## Deliverables

- [ ] Happy path tests pass
- [ ] Error handling tests pass
- [ ] Database verification complete

---

## Test Cases

### Happy Path

| ID | Test | Command | Expected Result |
|----|------|---------|-----------------|
| HP-1 | Mark thought for correction | `rethink thoughts:xxx --mark --type correction --for gemini --note "Test"` | Success response with marked object |
| HP-2 | Mark entity for research | `rethink kg_entities:xxx --mark --type research --for cc --note "Needs research"` | Success response |
| HP-3 | Mark observation for enrichment | `rethink kg_observations:xxx --mark --type enrich --for dt --note "Add context"` | Success response |
| HP-4 | Verify DB update | `SELECT marked_for, mark_type, mark_note FROM thoughts:xxx` | Fields populated correctly |

### Error Cases

| ID | Test | Command | Expected Result |
|----|------|---------|-----------------|
| ERR-1 | Invalid target format | `rethink badformat --mark --type correction --for cc --note "Test"` | Error: invalid target_id format |
| ERR-2 | Non-existent record | `rethink thoughts:doesnotexist --mark --type correction --for cc --note "Test"` | Error: record not found |
| ERR-3 | Invalid mark_type | `rethink thoughts:xxx --mark --type invalid --for cc --note "Test"` | Error: invalid mark_type |
| ERR-4 | Invalid marked_for | `rethink thoughts:xxx --mark --type correction --for nobody --note "Test"` | Error: invalid marked_for |
| ERR-5 | Missing note | `rethink thoughts:xxx --mark --type correction --for cc` | Error: note required |

### Edge Cases

| ID | Test | Command | Expected Result |
|----|------|---------|-----------------|
| EDGE-1 | Re-mark already marked record | Mark same record twice | Overwrites previous mark |
| EDGE-2 | Entity alias format | `rethink entity:xxx` vs `rethink kg_entities:xxx` | Both work (alias resolved) |
| EDGE-3 | Long note | Note with 1000+ characters | Accepted and stored |

---

## Test Results

### Run 1: 2026-01-10 (CC)

| Test ID | Result | Notes |
|---------|--------|-------|
| HP-1 | FAIL | "Record not found" - see Issue #1 |
| HP-2 | FAIL | "Record not found" - see Issue #1 |
| HP-3 | BLOCKED | Cannot test until Issue #1 fixed |
| HP-4 | BLOCKED | Cannot test until Issue #1 fixed |
| ERR-1 | PASS | Returns "Invalid target_id format" as expected |
| ERR-2 | INCONCLUSIVE | Returns "Record not found" but can't distinguish from Issue #1 bug |
| ERR-3 | PASS | Returns "Invalid mark_type" as expected |
| ERR-4 | PASS | Returns "Invalid marked_for" as expected |
| ERR-5 | BLOCKED | Cannot test - parameter is required by schema |
| EDGE-1 | BLOCKED | Cannot test until Issue #1 fixed |
| EDGE-2 | BLOCKED | Cannot test until Issue #1 fixed |
| EDGE-3 | BLOCKED | Cannot test until Issue #1 fixed |

**Tests attempted:**
```
# HP-1: Mark thought
rethink thoughts:d087dc2b-3819-4112-9c00-1e4530f9fc17 --mark --type correction --for gemini --note "Test"
Result: MCP error -32602: Validation error: Record not found

# HP-2: Mark entity (multiple ID formats tried)
rethink kg_entities:cq926p161dm4nmv9u7fo --mark --type correction --for gemini --note "Test"
Result: MCP error -32602: Validation error: Record not found

rethink entity:cq926p161dm4nmv9u7fo --mark --type correction --for gemini --note "Test"
Result: MCP error -32602: Validation error: Record not found
```

---

## Issues Found

| Issue | Severity | Description | Resolution |
|-------|----------|-------------|------------|
| #1 | CRITICAL | Record ID query doesn't use `type::thing()` | Fix query in rethink.rs |

### Issue #1: Record ID Query Bug

**Location:** `src/tools/rethink.rs` lines 101 and 118

**Problem:**
The query uses `WHERE id = $id` but should use `WHERE id = type::thing($table, $id)` to properly convert the string parameter to a SurrealDB record reference.

**Current (broken):**
```rust
.query(format!("SELECT id FROM {} WHERE id = $id", table_name))
.bind(("id", params.target_id.clone()))
```

**Should be (working pattern from wander.rs):**
```rust
.query(format!("SELECT id FROM {} WHERE id = type::thing('{}', $id)", table_name, table_name))
.bind(("id", id_part))  // just the ID without table prefix
```

**Evidence:**
- `wander.rs` line 43: `WHERE id = type::thing('thoughts', $id)`
- `admin.rs` line 212: `WHERE id = type::thing('thoughts', $id)`
- Both of these work correctly

**Impact:** ALL record operations fail with "Record not found" because string comparison to record type never matches.

---

## Verdict

**Status:** FAIL - NEEDS FIXES

**Ready for Phase 3:** [ ] Yes  [x] No - requires fixes above

**Next Steps:**
1. Fix Issue #1 in `src/tools/rethink.rs` (use type::thing() pattern)
2. Rebuild and restart service
3. Re-run all tests

---

## Remediation Plan

**Owner:** CC
**Priority:** CRITICAL (blocks Phase 3)
**Estimated Effort:** 30 minutes (code fix + rebuild + retest)

### Root Cause Analysis

The `rethink.rs` implementation fails all happy path tests because it uses direct string comparison for record IDs instead of SurrealDB's `type::thing()` function. In SurrealDB, record references are a special type - comparing a raw string to a record type always returns false.

**Evidence chain:**
1. **Working code (wander.rs:43-45)**: Uses `WHERE id = type::thing('thoughts', $id)` → queries succeed
2. **Broken code (rethink.rs:101)**: Uses `WHERE id = $id` → all queries return empty
3. **Pattern consistency**: Both `wander.rs` and `thinking.rs` use the `type::thing()` pattern successfully

### Remediation Steps

#### Step 1: Update Record Existence Check (rethink.rs, lines 98-108)

**Current code:**
```rust
let exists: Option<serde_json::Value> = self
    .db
    .query(format!("SELECT id FROM {} WHERE id = $id", table_name))
    .bind(("id", params.target_id.clone()))
    .await?
    .take(0)?;

if exists.is_none() {
    return Err(SurrealMindError::Validation {
        message: format!("Record not found: {}", params.target_id),
    });
}
```

**Fix approach:**
- Parse `params.target_id` to extract the ID portion (after the colon)
- Use `type::thing(table_name, id_part)` in the WHERE clause
- Keep the full `target_id` for the UPDATE query (SurrealDB accepts both formats there)

**Implementation:**
```rust
// Extract ID part from "table:id" format
let id_part = parts[1];

// Check if record exists using type::thing()
let exists: Option<serde_json::Value> = self
    .db
    .query(format!("SELECT id FROM {} WHERE id = type::thing('{}', $id)", table_name, table_name))
    .bind(("id", id_part))
    .await?
    .take(0)?;

if exists.is_none() {
    return Err(SurrealMindError::Validation {
        message: format!("Record not found: {}", params.target_id),
    });
}
```

#### Step 2: Update Record Update Query (rethink.rs, lines 115-128)

**Current code:**
```rust
self.db
    .query(format!(
        "UPDATE {} SET marked_for = $marked_for, mark_type = $mark_type, mark_note = $note, marked_at = $marked_at, marked_by = $marked_by WHERE id = $id",
        table_name
    ))
    .bind(("id", params.target_id.clone()))
    .bind(("marked_for", params.marked_for.clone()))
    .bind(("mark_type", params.mark_type.clone()))
    .bind(("note", params.note.clone()))
    .bind(("marked_at", marked_at.clone()))
    .bind(("marked_by", marked_by))
    .await?;
```

**Fix approach:**
- The UPDATE can use the full record ID format (SurrealDB handles it), but for consistency with the SELECT check, use `type::thing()` here too
- This ensures if there's any ambiguity, both queries use the same resolution method

**Implementation:**
```rust
self.db
    .query(format!(
        "UPDATE {} SET marked_for = $marked_for, mark_type = $mark_type, mark_note = $note, marked_at = $marked_at, marked_by = $marked_by WHERE id = type::thing('{}', $id)",
        table_name, table_name
    ))
    .bind(("id", id_part))  // Use just the ID part, not the full "table:id"
    .bind(("marked_for", params.marked_for.clone()))
    .bind(("mark_type", params.mark_type.clone()))
    .bind(("note", params.note.clone()))
    .bind(("marked_at", marked_at.clone()))
    .bind(("marked_by", marked_by))
    .await?;
```

### Validation Checklist

After implementing fixes:

- [ ] Code compiles: `cargo check`
- [ ] No clippy warnings: `cargo clippy`
- [ ] Formatted correctly: `cargo fmt`
- [ ] Release build succeeds: `cargo build --release`
- [ ] Service restarts: `launchctl kickstart -k gui/$(id -u)/dev.legacymind.surreal-mind`
- [ ] Health check passes: `curl http://127.0.0.1:8787/health`
- [ ] Run HP-1 test with real thought ID from database
- [ ] Run HP-2 test with real entity ID from database
- [ ] Run HP-3 test with real observation ID from database
- [ ] Run HP-4 test to verify database fields populated
- [ ] All error cases still behave correctly

### Post-Fix Testing Plan

#### Test Data Acquisition
Before re-running tests, get valid IDs from the database:
```bash
# Get a thought ID
curl http://localhost:8787/api/query -H "Content-Type: application/json" \
  -d '{"query": "SELECT id FROM thoughts LIMIT 1"}'

# Get an entity ID  
curl http://localhost:8787/api/query -H "Content-Type: application/json" \
  -d '{"query": "SELECT id FROM kg_entities LIMIT 1"}'

# Get an observation ID
curl http://localhost:8787/api/query -H "Content-Type: application/json" \
  -d '{"query": "SELECT id FROM kg_observations LIMIT 1"}'
```

#### Re-Test Happy Path (Run 2)
Execute all 4 happy path tests with real IDs from the database. Expected result: **ALL PASS**

#### Re-Test Error Cases (Run 2)
Error cases should still pass since they test validation before the query executes.

#### Database Verification (Run 2)
After HP-1 succeeds, query the database to verify mark fields were actually written:
```bash
# Verify thought was marked
curl http://localhost:8787/api/query -H "Content-Type: application/json" \
  -d '{"query": "SELECT marked_for, mark_type, mark_note, marked_at, marked_by FROM thoughts WHERE marked_for = '\''gemini'\''"}'
```

### Effort Estimate

| Task | Time |
|------|------|
| Code fix (2 locations) | 5 min |
| Compile & validate | 5 min |
| Service restart | 2 min |
| Re-run all tests | 10 min |
| Document results | 5 min |
| **Total** | **27 min** |

### Success Criteria

Phase 2 testing is **COMPLETE** when:
1. All happy path tests (HP-1 through HP-4) pass with real data
2. All error case tests (ERR-1 through ERR-4) still pass
3. Database verification confirms mark fields are correctly populated
4. This document updated with "Run 2" results showing all tests PASS
5. Status changed to "PASS - Ready for Phase 3"

---

## Implementation Summary

**Date:** 2026-01-11
**Implemented by:** Vibe (Mistral Vibe)
**Status:** ✅ COMPLETE - Code changes applied and validated

### Changes Made

The fix for Issue #1 has been successfully implemented in `src/tools/rethink.rs`:

#### 1. Record Existence Check (Lines 98-111)
**Before:**
```rust
let exists: Option<serde_json::Value> = self
    .db
    .query(format!("SELECT id FROM {} WHERE id = $id", table_name))
    .bind(("id", params.target_id.clone()))
    .await?
    .take(0)?;
```

**After:**
```rust
// Extract ID part from "table:id" format and clone it for binding
let id_part = parts[1].to_string();

// Check if the record exists using type::thing()
let exists: Option<serde_json::Value> = self
    .db
    .query(format!("SELECT id FROM {} WHERE id = type::thing('{}', $id)", table_name, table_name))
    .bind(("id", id_part.clone()))
    .await?
    .take(0)?;
```

#### 2. Record Update Query (Lines 115-128)
**Before:**
```rust
self.db
    .query(format!(
        "UPDATE {} SET marked_for = $marked_for, mark_type = $mark_type, mark_note = $note, marked_at = $marked_at, marked_by = $marked_by WHERE id = $id",
        table_name
    ))
    .bind(("id", params.target_id.clone()))
    .bind(("marked_for", params.marked_for.clone()))
    .bind(("mark_type", params.mark_type.clone()))
    .bind(("note", params.note.clone()))
    .bind(("marked_at", marked_at.clone()))
    .bind(("marked_by", marked_by))
    .await?;
```

**After:**
```rust
self.db
    .query(format!(
        "UPDATE {} SET marked_for = $marked_for, mark_type = $mark_type, mark_note = $note, marked_at = $marked_at, marked_by = $marked_by WHERE id = type::thing('{}', $id)",
        table_name, table_name
    ))
    .bind(("id", id_part))
    .bind(("marked_for", params.marked_for.clone()))
    .bind(("mark_type", params.mark_type.clone()))
    .bind(("note", params.note.clone()))
    .bind(("marked_at", marked_at.clone()))
    .bind(("marked_by", marked_by))
    .await?;
```

### Key Changes

1. **ID Parsing:** Extract the ID portion from the `target_id` string (e.g., extract `d087dc2b-3819-4112-9c00-1e4530f9fc17` from `thoughts:d087dc2b-3819-4112-9c00-1e4530f9fc17`)

2. **type::thing() Usage:** Replace direct string comparison with SurrealDB's `type::thing(table_name, id)` function to properly convert string parameters to record references

3. **Consistency:** Both SELECT and UPDATE queries now use the same pattern for record identification

### Validation Results

✅ **Code compiles:** `cargo check` - PASSED
✅ **No clippy warnings:** `cargo clippy --no-deps` - PASSED
✅ **Formatted correctly:** `cargo fmt` - PASSED
✅ **Release build succeeds:** `cargo build --release` - PASSED
✅ **Service restarted:** `launchctl kickstart -k gui/$(id -u)/dev.legacymind.surreal-mind` - PASSED
✅ **Health check passes:** `curl http://127.0.0.1:8787/health` - PASSED (returns "ok")

### Technical Details

**Root Cause:** In SurrealDB, record IDs are a special type, not plain strings. Comparing a string directly to a record ID always returns false. The `type::thing()` function properly converts a string ID into a record reference type.

**Pattern Consistency:** This fix aligns with existing working code in:
- `src/tools/wander.rs` (line 43): Uses `type::thing('thoughts', $id)`
- `src/tools/thinking.rs`: Uses similar pattern
- `src/bin/admin.rs` (line 212): Uses `type::thing('thoughts', $id)`

**Lifetime Handling:** The ID part is cloned to `String` to ensure it lives long enough for the database binding, which requires `'static` lifetime.

### Next Steps

The implementation is complete and ready for testing. The next steps are:

1. **Acquire test data:** Get valid IDs from the database for thoughts, entities, and observations
2. **Run happy path tests:** Execute HP-1 through HP-4 with real data
3. **Verify database updates:** Confirm mark fields are written correctly
4. **Re-test error cases:** Ensure ERR-1 through ERR-4 still pass
5. **Update this document:** Add Run 2 results and change status to "PASS - Ready for Phase 3"

**Estimated time for remaining work:** 15-20 minutes (depending on test data availability)

---

## Updated Status

**Status:** PENDING RETEST - Datetime serialization avoided
**Ready for Phase 3:** [ ] Yes  [x] No - Awaiting MCP retest by CC

**What changed (2026-01-11, update 2):**
- UPDATE now uses `RETURN NONE`, eliminating the SurrealDB Rust SDK datetime deserialization error.
- `marked_at` is set via `time::now()` but not returned to the client; response omits the datetime field.
- Existence check now uses a scalar `RETURN count((SELECT ...))` with `type::thing()` so the SDK deserializes to plain `i64` (fixes MCP error: “expected a 64-bit signed integer, found { \"count\": 1i64 }”).

**Manual verification:**
- Direct SurrealQL update on `thoughts:001db6f2-8a92-41f3-a03f-0cb36d42d238` succeeded; `marked_for/mark_type/mark_note/marked_by/marked_at` persisted as expected.

**Remaining work:**
- Run MCP happy-path tests (HP-1..HP-4) through the `rethink` tool to confirm end-to-end behavior; serialization errors should be resolved.
- If all pass, flip status to PASS and proceed to Phase 3.
