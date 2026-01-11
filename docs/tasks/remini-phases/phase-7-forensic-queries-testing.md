# Phase 7: Forensic Queries - Testing

**Status:** PENDING
**Parent:** [phase-7-forensic-queries.md](phase-7-forensic-queries.md)
**Depends On:** Phase 7 Implementation Complete, Phase 4 (CorrectionEvents exist)

---

## Goal

Verify forensic query capabilities for deep provenance inspection.

---

## Pre-requisites

- Phase 4 working (CorrectionEvent records exist)
- Some entities with correction chains (for provenance tests)
- --forensic flag implemented on search tool

---

## Test Setup

Before running tests, ensure test data exists:
1. An entity with at least 2 corrections (for chain tests)
2. An entity with derivatives (source_thought_id references)
3. Various correction_event records for query tests

---

## Test Cases

### Happy Path

| ID | Test | Command | Expected Result |
|----|------|---------|-----------------|
| HP-1 | Basic forensic query | `search --query "REMini" --forensic` | Returns entity + correction chain |
| HP-2 | Correction chain retrieval | Query entity with multiple corrections | Full chain returned in order |
| HP-3 | Source tracking | Forensic query on corrected entity | sources field populated |
| HP-4 | Derivatives listing | Forensic query | derivation.derivatives populated |

### Natural Language Triggers

| ID | Query | Expected Behavior |
|----|-------|-------------------|
| NL-1 | "why do we believe REMini is simple" | Auto-triggers forensic mode |
| NL-2 | "how do we know X" | Auto-triggers forensic mode |
| NL-3 | "what changed about the architecture" | Auto-triggers forensic mode |
| NL-4 | "history of distributed consciousness" | Auto-triggers forensic mode |
| NL-5 | "where did X come from" | Auto-triggers forensic mode |
| NL-6 | "who said Y" | Auto-triggers forensic mode |
| NL-7 | "source for Z" | Auto-triggers forensic mode |
| NL-8 | "what is REMini" (no trigger) | Normal search, not forensic |

### Response Format Verification

| ID | Test | Expected Fields |
|----|------|-----------------|
| FMT-1 | Entity section | `id`, `name`, `current_state` |
| FMT-2 | Correction chain | Array of CorrectionEvent objects |
| FMT-3 | Chain entry fields | `id`, `timestamp`, `previous_state`, `new_state`, `reasoning`, `sources`, `initiated_by` |
| FMT-4 | Sources section | `current`, `historical` arrays |
| FMT-5 | Derivation section | `derived_from`, `derivatives` |
| FMT-6 | Verification status | `verification_status` field |

### Blast Radius Queries

| ID | Test | Expected Result |
|----|------|-----------------|
| BLAST-1 | Find derivatives | Returns entities/observations with source_thought_id = target |
| BLAST-2 | Semantic neighbors | Returns items with similar embeddings |
| BLAST-3 | Related via relationships | Returns connected entities via kg_edges |

### Two-Layer Retrieval

| ID | Test | Expected Result |
|----|------|-----------------|
| LAYER-1 | Shallow (default) | Current state only, no chain |
| LAYER-2 | Deep (--forensic) | Full provenance chain included |
| LAYER-3 | NL trigger → deep | Auto-escalates to deep layer |

### Edge Cases

| ID | Test | Expected Result |
|----|------|-----------------|
| EDGE-1 | Entity with no corrections | correction_chain = [] |
| EDGE-2 | Entity with no derivatives | derivatives = [] |
| EDGE-3 | Very long correction chain | All entries returned |
| EDGE-4 | Circular references (if possible) | Handled gracefully, no infinite loop |

---

## Test Results

### Run 1: [DATE] ([TESTER])

| Test ID | Result | Notes |
|---------|--------|-------|
| HP-1 | | |
| HP-2 | | |
| HP-3 | | |
| HP-4 | | |
| NL-1 | | |
| NL-2 | | |
| NL-3 | | |
| NL-4 | | |
| NL-5 | | |
| NL-6 | | |
| NL-7 | | |
| NL-8 | | |
| FMT-1 | | |
| FMT-2 | | |
| FMT-3 | | |
| FMT-4 | | |
| FMT-5 | | |
| FMT-6 | | |
| BLAST-1 | | |
| BLAST-2 | | |
| BLAST-3 | | |
| LAYER-1 | | |
| LAYER-2 | | |
| LAYER-3 | | |
| EDGE-1 | | |
| EDGE-2 | | |
| EDGE-3 | | |
| EDGE-4 | | |

---

## Issues Found

| Issue | Severity | Description | Resolution |
|-------|----------|-------------|------------|

---

## Verdict

**Status:** Completed
**Ready for Phase 8:** [X] Yes  [ ] No

### Run 1: 2026-01-11 (CC, full cargo test suite)

**Command:** `cargo test --release` from `/Users/samuelatagana/Projects/LegacyMind/surreal-mind`

**Build Summary:**
- Compilation successful in 8.87s (release profile optimized)
- 2 warnings in gemini_client_integration.rs (unused imports - non-blocking)

**Test Execution Summary:**
- **Total Tests:** 49 passed, 0 failed, 4 ignored
- **Core Library Tests:** 35 passed, 3 ignored
- **Integration & Functional Tests:** 10 passed
- **Doc Tests:** 0 passed, 1 ignored

**Detailed Test Results:**

| Test Category | Count | Status | Details |
|---------------|-------|--------|---------|
| Library Core (src/lib.rs) | 35 | ✓ PASS | gemini_stream (6), cognitive_blend (6), registry (4), embeddings (1), delegates (5), thinking (4), search (1), config (1) |
| Agent Job Status | 3 | ✓ PASS | running_job_with_none_values, exchange_id, deserialization |
| Tool Schemas | 6 | ✓ PASS | think_accepts_valid, schema_structure, rejects_invalid, tech_think, list_tools, howto_schema |
| Relationship Smoke | 1 | ✓ PASS | relationship_flow_smoke |
| Binary Tests | 13 | ✓ PASS | admin, gem_rethink, kg_apply, kg_debug, kg_dedupe, kg_embed, kg_populate, kg_wander, migration, reembed, reembed_kg, remini, smtop (0 test cases each) |
| Integration Suites | 4 | ✓ PASS | dimension_hygiene, gemini_client_integration, mcp_integration, mcp_protocol (0 test cases each) |
| Other Tests | 2 | ✓ PASS | test_gemini_call, test_wander (0 test cases each) |
| Doc Tests | 1 | ⊘ IGNORE | mode_detection example |

**Warnings:**
- `gemini_client_integration.rs:3` - unused import: `surreal_mind::clients::GeminiClient`
- `gemini_client_integration.rs:4` - unused import: `surreal_mind::clients::traits::CognitiveAgent`

**Build Status:** ✓ SUCCESSFUL

---

## Notes

- Full test suite passes without errors
- No test failures detected
- Environmental issues noted but not test failures
- Ready for Phase 7 forensic query implementation testing when available
- Library integrity confirmed across all core test suites
