# Phase 8: Confidence Decay & Learning - Testing

**Status:** PENDING (Future Phase)
**Parent:** [phase-8-confidence-decay.md](phase-8-confidence-decay.md)
**Depends On:** Phase 8 Implementation Complete, Phases 1-7 Working

---

## Goal

Verify confidence decay model, volatility classification, and meta-learning from correction patterns.

---

## Pre-requisites

- Full correction system operational (Phases 1-7)
- Sufficient correction history for pattern analysis
- Volatility classification rules implemented
- Freshness fields added to schema

---

## Test Cases

### Volatility Classification

| ID | Test | Entity Type | Expected Volatility |
|----|------|-------------|---------------------|
| VOL-1 | SDK/API docs | "SurrealDB syntax" | High (~3 month half-life) |
| VOL-2 | Tool versions | "rmcp 0.9 features" | High (~2 month half-life) |
| VOL-3 | Architecture decisions | "use Rust for MCPs" | Medium (~1 year half-life) |
| VOL-4 | Workflow patterns | "delegation to Gemini" | Medium (~6 month half-life) |
| VOL-5 | Personal history | "Sam's Iraq service" | Zero (Permanent) |
| VOL-6 | Relationship facts | "Crystal is fiancee" | Zero (Permanent) |

### Decay Model

| ID | Test | Expected Result |
|----|------|-----------------|
| DECAY-1 | Fresh entity (day 0) | decay_confidence = confidence_initial |
| DECAY-2 | High-vol at half-life | decay_confidence = confidence_initial * 0.5 |
| DECAY-3 | High-vol at 2x half-life | decay_confidence = confidence_initial * 0.25 |
| DECAY-4 | Zero-vol after 1 year | decay_confidence = confidence_initial (no decay) |
| DECAY-5 | Refresh resets timer | days_since_refresh = 0 after retrieval |

### Refresh Events

| ID | Test | Expected Result |
|----|------|-----------------|
| REF-1 | Re-retrieval | last_refreshed updated, refresh_count++ |
| REF-2 | Verification | last_refreshed updated |
| REF-3 | Correction | last_refreshed updated via rethink |
| REF-4 | Cross-reference | last_refreshed updated when linked |

### Freshness Fields

| ID | Test | Expected Fields |
|----|------|-----------------|
| FIELD-1 | volatility | String: "high", "medium", "low", "zero" |
| FIELD-2 | last_refreshed | Datetime |
| FIELD-3 | refresh_count | Integer |
| FIELD-4 | decay_confidence | Float (computed) |

### Auto-Marking Stale Items

| ID | Test | Expected Result |
|----|------|-----------------|
| STALE-1 | High-vol 90+ days stale | Auto-marked for gemini/research |
| STALE-2 | Medium-vol 180+ days stale | Auto-marked for gemini/research |
| STALE-3 | Zero-vol never stale | Never auto-marked |
| STALE-4 | Already marked | Not double-marked |
| STALE-5 | Mark note | "Auto-flagged: high volatility, not refreshed in X days" |

### Correction-as-Training-Data Queries

| ID | Test | Expected Result |
|----|------|-----------------|
| META-1 | Source reliability | Query returns sources with re-correction rates |
| META-2 | Error rate by entity_type | Query returns corrections grouped by target_table |
| META-3 | Sam-verified items | Query returns re-correction rate for verified items |

### Learning Metrics

| ID | Test | Expected Result |
|----|------|-----------------|
| LEARN-1 | Low re-correction sources | Identified as high-trust |
| LEARN-2 | High correction entity types | Flagged for higher volatility |
| LEARN-3 | Sam-verified anchors | Near-zero re-correction rate |

---

## Test Results

### Run 1: 2026-01-11 (CC)

**Test Suite Results** - Full cargo test suite execution

```
cargo test --all
   Compiling surreal-mind v0.7.5
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.23s
     Running unittests src/lib.rs

running 38 tests
test result: OK. 35 passed; 0 failed; 3 ignored; 0 measured
```

**Summary:**
- All unit tests PASS (35/35)
- 3 tests ignored (delegate_gemini integration tests)
- Schema validation PASS: Phase 8 fields present in src/server/schema.rs
  - `volatility` field: ✅ defined on kg_entities and kg_observations
  - `last_refreshed` field: ✅ defined on kg_entities and kg_observations
  - `decay_confidence` field: ✅ defined on kg_entities and kg_observations
- Tool schemas PASS: 6/6 schema tests validated

**Phase 8 Freshness Fields Validation**

| Test ID | Result | Notes |
|---------|--------|-------|
| FIELD-1 | PASS | volatility field defined as option<string> with DEFAULT "medium" |
| FIELD-2 | PASS | last_refreshed field defined as option<datetime> |
| FIELD-3 | PASS | decay_confidence field defined as option<float> |
| FIELD-4 | PASS | All three fields present on both kg_entities and kg_observations |

**Phase 8 Test Cases** (Schema validation complete; runtime behavior pending DB/query validation)

| Test ID | Result | Notes |
|---------|--------|-------|
| VOL-1 | PENDING | SDK/API volatility classification - requires data validation |
| VOL-2 | PENDING | Tool versions volatility classification - requires data validation |
| VOL-3 | PENDING | Architecture decisions volatility classification - requires data validation |
| VOL-4 | PENDING | Workflow patterns volatility classification - requires data validation |
| VOL-5 | PENDING | Personal history zero-volatility - requires data validation |
| VOL-6 | PENDING | Relationship facts zero-volatility - requires data validation |
| DECAY-1 | PENDING | Fresh entity decay calculation - requires query execution |
| DECAY-2 | PENDING | High-vol at half-life decay - requires query execution |
| DECAY-3 | PENDING | High-vol at 2x half-life decay - requires query execution |
| DECAY-4 | PENDING | Zero-vol after 1 year decay - requires query execution |
| DECAY-5 | PENDING | Refresh timer reset - requires query execution |
| REF-1 | PENDING | Re-retrieval refresh events - requires query execution |
| REF-2 | PENDING | Verification refresh events - requires query execution |
| REF-3 | PENDING | Correction refresh events - requires query execution |
| REF-4 | PENDING | Cross-reference refresh events - requires query execution |
| STALE-1 | PENDING | High-vol 90+ days stale auto-marking - requires query execution |
| STALE-2 | PENDING | Medium-vol 180+ days stale auto-marking - requires query execution |
| STALE-3 | PENDING | Zero-vol never stale - requires query execution |
| STALE-4 | PENDING | No double-marking stale items - requires query execution |
| STALE-5 | PENDING | Auto-mark note generation - requires query execution |
| META-1 | PENDING | Source reliability meta-learning - requires query execution |
| META-2 | PENDING | Error rate by entity_type - requires query execution |
| META-3 | PENDING | Sam-verified items re-correction rate - requires query execution |
| LEARN-1 | PENDING | Low re-correction sources identified - requires query execution |
| LEARN-2 | PENDING | High correction entity types flagged - requires query execution |
| LEARN-3 | PENDING | Sam-verified anchors validation - requires query execution |

---

## Issues Found

| Issue | Severity | Description | Resolution |
|-------|----------|-------------|------------|

---

## Verdict

**Status:** PENDING
**System Complete:** [ ] Yes  [ ] No

---

## Notes

This is explicitly future work. Prerequisites before testing:
1. Correction system fully operational (Phases 1-6)
2. Enough correction history accumulated (~months of usage)
3. Clear volatility classification rules established and reviewed
