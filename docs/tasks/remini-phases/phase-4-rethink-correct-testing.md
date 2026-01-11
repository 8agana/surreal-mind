# Phase 4: rethink Tool - Correct Mode - Testing

**Status:** PENDING
**Parent:** [phase-4-rethink-correct.md](phase-4-rethink-correct.md)
**Depends On:** Phase 4 Implementation Complete

---

## Goal

Verify the `rethink --correct` mode works correctly for executing corrections with full provenance tracking.

---

## Pre-requisites

- Phase 2 rethink mark mode working (VERIFIED)
- Phase 3 wander marks mode working (VERIFIED)
- Phase 4 correct mode implemented

---

## Test Cases

### Happy Path

| ID | Test | Command | Expected Result |
|----|------|---------|-----------------|
| HP-1 | Basic correction | `rethink <entity_id> --correct --reasoning "..." --sources '[...]'` | Success with CorrectionEvent created |
| HP-2 | Correction with cascade | `rethink <entity_id> --correct --reasoning "..." --sources '[...]' --cascade` | Success + derivatives flagged |
| HP-3 | Verify CorrectionEvent record | Query correction_event table | Record exists with all fields |
| HP-4 | Verify previous_state preserved | Check CorrectionEvent.previous_state | Contains original entity data |
| HP-5 | Verify new_state recorded | Check CorrectionEvent.new_state | Contains corrected entity data |
| HP-6 | Verify mark fields cleared | Query target after correction | marked_for, mark_type, mark_note = NULL |
| HP-7 | Chain corrections | Correct same entity twice | Second has corrects_previous linking to first |

### Response Format Verification

| ID | Test | Expected Fields |
|----|------|-----------------|
| FMT-1 | Response structure | `success`, `correction` object |
| FMT-2 | Correction object fields | `id`, `target_id`, `previous_state`, `new_state`, `reasoning`, `sources`, `initiated_by` |
| FMT-3 | Cascade response | `derivatives_flagged` count when --cascade used |

### Error Cases

| ID | Test | Command | Expected Result |
|----|------|---------|-----------------|
| ERR-1 | Missing reasoning | `rethink <id> --correct --sources '[...]'` | Error: reasoning required |
| ERR-2 | Missing sources | `rethink <id> --correct --reasoning "..."` | Error: sources required |
| ERR-3 | Invalid sources JSON | `rethink <id> --correct --reasoning "..." --sources 'not json'` | Error: invalid sources format |
| ERR-4 | Non-existent target | `rethink entity:doesnotexist --correct ...` | Error: record not found |
| ERR-5 | Invalid target format | `rethink badformat --correct ...` | Error: invalid target_id format |

### Edge Cases

| ID | Test | Command | Expected Result |
|----|------|---------|-----------------|
| EDGE-1 | Empty sources array | `--sources '[]'` | Should succeed (empty but valid) |
| EDGE-2 | Correct unmarked record | Correct entity without mark | Should succeed (mark not required for correction) |
| EDGE-3 | Long reasoning | 5000+ character reasoning | Accepted and stored |
| EDGE-4 | Cascade with no derivatives | --cascade on entity with no derivatives | derivatives_flagged = 0 |

### Provenance Chain Tests

| ID | Test | Expected Result |
|----|------|-----------------|
| PROV-1 | First correction | corrects_previous = NULL |
| PROV-2 | Second correction | corrects_previous = first correction ID |
| PROV-3 | Full chain query | Can traverse correction_event chain for entity |

---

## Test Results

### Run 1: 2026-01-11 (CC)

| Test ID | Result | Notes |
|---------|--------|-------|
| HP-1 | FAIL | Serialization error: invalid type: enum |
| HP-2 | BLOCKED | |
| HP-3 | BLOCKED | |
| HP-4 | BLOCKED | |
| HP-5 | BLOCKED | |
| HP-6 | BLOCKED | |
| HP-7 | BLOCKED | |
| FMT-1 | BLOCKED | |
| FMT-2 | BLOCKED | |
| FMT-3 | BLOCKED | |
| ERR-1 | PASS | "reasoning is required for correct mode" |
| ERR-2 | PASS | "sources is required for correct mode" |
| ERR-3 | N/A | sources accepts array directly, not JSON string |
| ERR-4 | PASS | "Record not found: entity:doesnotexist" |
| ERR-5 | PASS | "Invalid target_id format. Expected table:id" |
| EDGE-1 | BLOCKED | |
| EDGE-2 | BLOCKED | |
| EDGE-3 | BLOCKED | |
| EDGE-4 | BLOCKED | |
| PROV-1 | BLOCKED | |
| PROV-2 | BLOCKED | |
| PROV-3 | BLOCKED | |

---

## Issues Found

| Issue | Severity | Description | Resolution |
|-------|----------|-------------|------------|
| #1 | CRITICAL | Serialization error on correct mode | Change RETURN AFTER to RETURN NONE (line 226) |

### Issue #1: Serialization Error

**Error:**
```
MCP error -32603: Database error: Serialization error: invalid type: enum, expected any valid JSON value
```

**Location:** `src/tools/rethink.rs` line 226

**Problem:** Same issue that was fixed in mark mode. The `RETURN AFTER` causes SurrealDB to return datetime fields that the Rust SDK can't deserialize.

**Fix:** Change `RETURN AFTER` to `RETURN NONE` on line 226.

---

## Verdict

**Status:** FAIL - NEEDS FIX
**Ready for Phase 5:** [ ] Yes  [x] No
