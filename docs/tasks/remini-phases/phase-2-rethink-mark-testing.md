# Phase 2: rethink Tool - Mark Mode - Testing

**Status:** FAIL - Needs Fixes
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
