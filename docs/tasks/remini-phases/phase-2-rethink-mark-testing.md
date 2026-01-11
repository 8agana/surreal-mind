# Phase 2: rethink Tool - Mark Mode - Testing

**Status:** Not Started
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

### Run 1: [DATE]

| Test ID | Result | Notes |
|---------|--------|-------|
| | | |

---

## Issues Found

| Issue | Severity | Description | Resolution |
|-------|----------|-------------|------------|
| | | | |

---

## Verdict

**Status:** [PENDING / PASS / FAIL - NEEDS FIXES]

**Ready for Phase 3:** [ ] Yes  [ ] No - requires fixes above
