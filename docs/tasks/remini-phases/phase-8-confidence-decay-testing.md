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

### Run 1: [DATE] ([TESTER])

| Test ID | Result | Notes |
|---------|--------|-------|
| VOL-1 | | |
| VOL-2 | | |
| VOL-3 | | |
| VOL-4 | | |
| VOL-5 | | |
| VOL-6 | | |
| DECAY-1 | | |
| DECAY-2 | | |
| DECAY-3 | | |
| DECAY-4 | | |
| DECAY-5 | | |
| REF-1 | | |
| REF-2 | | |
| REF-3 | | |
| REF-4 | | |
| FIELD-1 | | |
| FIELD-2 | | |
| FIELD-3 | | |
| FIELD-4 | | |
| STALE-1 | | |
| STALE-2 | | |
| STALE-3 | | |
| STALE-4 | | |
| STALE-5 | | |
| META-1 | | |
| META-2 | | |
| META-3 | | |
| LEARN-1 | | |
| LEARN-2 | | |
| LEARN-3 | | |

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
