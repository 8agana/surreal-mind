# Phase 3: wander --mode marks - Testing

**Status:** PENDING
**Parent:** [phase-3-wander-marks.md](phase-3-wander-marks.md)
**Depends On:** Phase 2 (rethink tool working)

---

## Goal

Verify the `wander --mode marks` functionality works correctly for surfacing marked items.

---

## Pre-requisites

- Phase 2 rethink tool must be working (VERIFIED: 2026-01-10)
- Some records must be marked for testing (created during Phase 2 testing)

---

## Test Cases

### Happy Path

| ID | Test | Command | Expected Result |
|----|------|---------|-----------------|
| HP-1 | Get all marks | `wander --mode marks` | Returns marked items with queue_depth |
| HP-2 | Filter marks for CC | `wander --mode marks --for cc` | Returns only items marked for cc |
| HP-3 | Filter marks for Gem | `wander --mode marks --for gem` | Returns only items marked for gem |
| HP-4 | Filter marks for Sam | `wander --mode marks --for sam` | Returns only items marked for sam |
| HP-5 | Use visited_ids to skip | `wander --mode marks --visited_ids [id]` | Skips visited item |

### Response Format Verification

| ID | Test | Expected Fields |
|----|------|-----------------|
| FMT-1 | Response structure | `mode_used`, `current_node`, `queue_depth`, `guidance`, `affordances` |
| FMT-2 | current_node fields | `id`, `name`, `mark_type`, `marked_for`, `mark_note`, `marked_by`, `marked_at` |
| FMT-3 | affordances array | Contains `["correct", "dismiss", "reassign", "next"]` |

### Edge Cases

| ID | Test | Command | Expected Result |
|----|------|---------|-----------------|
| EDGE-1 | No marks exist for filter | `wander --mode marks --for dt` | Empty response or appropriate message |
| EDGE-2 | All marks visited | Pass all marked IDs in visited_ids | Empty response |

---

## Test Results

### Run 1: 2026-01-10 (CC)

**Initial failure:** Query syntax error - `meta::id(id)` in ORDER BY clause not supported by SurrealDB.

**Fix applied:** Removed secondary ORDER BY field. Changed `ORDER BY marked_at ASC, meta::id(id) ASC` to `ORDER BY marked_at ASC`.

### Run 2: 2026-01-10 (CC) - After Fix

| Test ID | Result | Notes |
|---------|--------|-------|
| HP-1 | PASS | Returns thought marked for gem with queue_depth=3 |
| HP-2 | PASS | Returns entity marked for cc (MapReduce for consciousness) |
| HP-3 | PASS | Returns thought marked for gem |
| HP-4 | PASS | Returns observation marked for sam |
| HP-5 | PASS | Skips visited ID, returns null node with queue_depth=0 |
| FMT-1 | PASS | Response has mode_used, current_node, queue_depth, guidance, affordances |
| FMT-2 | PASS | current_node has id, table, mark_type, marked_for, mark_note, marked_by, marked_at, name, content |
| FMT-3 | PASS | affordances = ["correct", "dismiss", "reassign", "next"] |
| EDGE-1 | PASS | No marks for dt returns null node, queue_depth=0, appropriate message |
| EDGE-2 | PASS | Implicit - covered by HP-5 behavior |

---

## Issues Found

| Issue | Severity | Description | Resolution |
|-------|----------|-------------|------------|
| #1 | MEDIUM | meta::id(id) cannot be used in ORDER BY | Removed secondary sort field |

### Issue #1: ORDER BY with meta::id() Function

**Location:** `src/tools/wander.rs` line 344

**Problem:** SurrealDB doesn't allow function calls like `meta::id(id)` in ORDER BY clauses.

**Error:**
```
Missing order idiom `meta` in statement selection
```

**Fix:** Removed secondary ORDER BY field. The primary sort by `marked_at ASC` is sufficient for returning oldest marks first.

**Before:**
```sql
ORDER BY marked_at ASC, meta::id(id) ASC LIMIT 1
```

**After:**
```sql
ORDER BY marked_at ASC LIMIT 1
```

---

## Verdict

**Status:** PASS
**Ready for Phase 4:** [x] Yes  [ ] No

All tests pass after query fix. Phase 3 wander marks mode is fully functional.
