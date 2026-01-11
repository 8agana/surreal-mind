# Phase 5: gem_rethink Process - Testing

**Status:** PENDING
**Parent:** [phase-5-gem-rethink.md](phase-5-gem-rethink.md)
**Depends On:** Phase 5 Implementation Complete, Phases 1-4 Working

---

## Goal

Verify the `gem_rethink` binary correctly processes the Gemini mark queue autonomously.

---

## Pre-requisites

- Phase 4 rethink correct mode working
- gem_rethink binary built
- Test data: items marked for gemini with various mark types

---

## Test Setup

Before running tests, create test marks:
```bash
# Mark items for gemini with each type
rethink <thought_id> --mark --type correction --for gemini --note "Test correction mark"
rethink <entity_id> --mark --type research --for gemini --note "Test research mark"
rethink <observation_id> --mark --type enrich --for gemini --note "Test enrich mark"
rethink <thought_id_2> --mark --type expand --for gemini --note "Test expand mark"
```

---

## Test Cases

### Happy Path

| ID | Test | Command | Expected Result |
|----|------|---------|-----------------|
| HP-1 | Dry run mode | `gem_rethink --dry-run` | Lists items to process, no changes made |
| HP-2 | Process single type | `gem_rethink --type correction` | Only correction marks processed |
| HP-3 | Process all types | `gem_rethink` | All mark types processed |
| HP-4 | Verify marks cleared | Query after run | marked_for = NULL on processed items |
| HP-5 | Verify report generated | Check report output | Valid JSON report with counts |

### Mark Type Processing

| ID | Mark Type | Expected Behavior |
|----|-----------|-------------------|
| MT-1 | correction | CorrectionEvent created, content fixed |
| MT-2 | research | Web/KG search performed, content enriched |
| MT-3 | enrich | New relationships/entities extracted |
| MT-4 | expand | New connected thoughts created |

### Report Format Verification

| ID | Test | Expected Fields |
|----|------|-----------------|
| RPT-1 | Report structure | `run_timestamp`, `items_processed`, `by_type`, `corrections_made`, `errors`, `duration_seconds` |
| RPT-2 | by_type breakdown | Counts for each mark type processed |
| RPT-3 | Error reporting | Any errors captured with context |

### Error Handling

| ID | Test | Expected Result |
|----|------|-----------------|
| ERR-1 | Invalid mark in queue | Skip with error logged, continue processing |
| ERR-2 | API failure mid-run | Log error, continue with remaining items |
| ERR-3 | Empty queue | Clean exit, report shows 0 processed |
| ERR-4 | Database connection failure | Graceful error, no partial state |

### Context Gathering

| ID | Test | Expected Result |
|----|------|-----------------|
| CTX-1 | Derivatives gathered | source_thought_id derivatives included in context |
| CTX-2 | Semantic neighbors gathered | Embedding search finds related items |
| CTX-3 | Relationships gathered | Related entities via kg_edges included |

---

## Test Results

### Run 1: [DATE] ([TESTER])

| Test ID | Result | Notes |
|---------|--------|-------|
| HP-1 | | |
| HP-2 | | |
| HP-3 | | |
| HP-4 | | |
| HP-5 | | |
| MT-1 | | |
| MT-2 | | |
| MT-3 | | |
| MT-4 | | |
| RPT-1 | | |
| RPT-2 | | |
| RPT-3 | | |
| ERR-1 | | |
| ERR-2 | | |
| ERR-3 | | |
| ERR-4 | | |
| CTX-1 | | |
| CTX-2 | | |
| CTX-3 | | |

---

## Issues Found

| Issue | Severity | Description | Resolution |
|-------|----------|-------------|------------|

---

## Verdict

**Status:** PENDING
**Ready for Phase 6:** [ ] Yes  [ ] No
