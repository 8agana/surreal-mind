---
id: doc-2
title: CC review - kg_populate implementation feasibility
type: other
created_date: '2025-12-31 22:46'
---
# CC review — kg_populate implementation feasibility

**Related task**: task-1 (Implement kg_populate orchestrator binary)  
**Related doc**: doc-1 (Codex review - kg_populate orchestrator)  
**Date**: 2025-12-31  
**Context**: Architecture review after memories_populate removal, validating orchestrator design before handoff to implementation.

---

## OVERALL ASSESSMENT
**Status**: Ready to implement with clarifications noted below  
**Risk Level**: Low-Medium (clear spec, proven patterns, single concern about batch error handling)  
**Estimated Complexity**: Medium (multi-stage pipeline, but each stage is standard)

---

## STRENGTHS OF THE DESIGN

### 1. Clean Architecture
- Clear separation: load config → fetch unextracted → batch to Gemini → parse → upsert → mark extracted
- Each stage is independently testable and has clear success/failure semantics
- Idempotency requirement (`extracted_to_kg` flag) prevents duplicate processing

### 2. Solid Technical Foundation
- `delegate_gemini` already exists and handles async orchestration (no need to reinvent)
- SurrealDB schema is already defined with proper uniqueness constraints
- JSON schema (from archived memories_populate doc) is well-structured with confidence scoring
- KG tables have idempotent keys: `(name)` for entities, `(source, target, rel_type)` for edges

### 3. Practical Acceptance Criteria
- All 8 criteria are concrete, testable, and achievable
- Criteria #7 (idempotency) and #8 (logging) are the discipline requirements—good instincts

---

## CLARIFICATIONS NEEDED (BEFORE IMPLEMENTATION)

### 1. Batch Error Handling Strategy
**Issue**: If Gemini fails mid-batch (e.g., on thought #47 of 100), do we:
- Mark nothing as extracted and retry the whole batch?
- Mark successful ones and retry only failures?
- Partial mark with tracking of which thoughts failed?

**Recommendation**: Implement per-thought error handling + logging. Mark successes, log failures with thought ID + error. Allows resume without re-processing successes.

**Codex doc silence**: Doc-1 doesn't specify this. Clarify with Codex or make the call based on production safety.

### 2. Boundaries Storage Decision
**Issue**: Doc-1 notes "no `boundary` table exists; decide to map boundaries into `kg_observations`"

**Recommendation**: Create a decision before implementation:
- Option A: Store as `kg_observations` with type/tag "boundary" + reason in content
- Option B: Create a dedicated `kg_boundaries` table for future querying
- Option C: Store in a separate metadata structure

**My preference**: Option B (dedicated table). Boundaries are distinct from observations semantically. Worth the 20 minutes of schema work now vs. months of querying pain later.

### 3. Batch Size and Performance Tuning
**Issue**: Task doesn't specify:
- How many thoughts per batch to Gemini?
- Should batch size be configurable via env var?
- Timeout handling for large batches?

**Recommendation**: 
- Default batch size: 25 thoughts (reasonable for Gemini token limits + response parsing)
- Make configurable: `KG_POPULATE_BATCH_SIZE` env var
- Per-batch timeout: inherit from `delegate_gemini` timeout_ms, or set sensible default (60 seconds)

### 4. Extraction Prompt Location
**Codex says**: "Prompt file should be created at `src/prompts/kg_extraction_v1.md`"  
**Reality**: This file doesn't exist yet.

**Action**: Task should either include "create extraction prompt" or reference where it comes from. If it's the archived memories_populate doc, that should be explicit.

---

## TECHNICAL VALIDATION

### ✅ SurrealDB Integration
- Schema exists (`thoughts.extracted_to_kg`, `extraction_batch_id`, `extracted_at`)
- KG tables exist with proper uniqueness keys
- Upsert logic is straightforward

### ✅ delegate_gemini Integration
- Tool already supports all needed parameters (model, cwd, timeout_ms)
- Returns structured response (can parse JSON reliably)
- Fire-and-forget not needed here (we need the response)

### ✅ JSON Parsing
- Codex warns about ```json fences - easy fix with trim/strip before parsing
- Schema is well-defined (entities, relationships, observations, boundaries, summary)
- Confidence scoring built in - good for future filtering

### ⚠️ Logging Requirements
- Acceptance #8 says "per-batch counts and summary totals"
- Should log: thoughts_fetched, thoughts_sent, successful_extractions, failed_extractions, upserted_entities, upserted_edges, upserted_observations
- Consider structured logging (JSON) for easier querying later

---

## MISSING PIECES (NOT BLOCKERS)

1. **Extraction Prompt**: Need explicit location or inline definition
2. **Error Recovery**: No mention of retry logic or partial failure handling
3. **Boundaries Table**: Decision pending on storage approach
4. **Performance Targets**: No SLA on extraction time (useful for monitoring)
5. **Dry Run Mode**: No mention of `--dry-run` flag (helpful for testing)

---

## WHAT WORKS WELL

### Task-1 + Doc-1 Combination
- Task provides clear user-facing requirements
- Codex doc provides technical details (schema, field names, idempotency keys)
- Together they form a complete spec

### Acceptance Criteria Quality
- All testable and observable
- No vague requirements ("should be efficient" etc.)
- Idempotency (#7) and logging (#8) are the discipline gates

### Reuse of Proven Components
- `delegate_gemini` already battle-tested (async, serialization fixed)
- SurrealDB schema already defined
- JSON schema from archived doc, ready to use

---

## RECOMMENDATION: READY TO IMPLEMENT

**Next step**: Clarify the 4 items above (batch error handling, boundaries storage, batch size config, extraction prompt location) with Codex or make documented decisions.

**Handoff readiness**: 8.5/10. Missing pieces are small and don't block implementation—they're refinement decisions that can be made during coding or in a pre-implementation sync.

**Confidence level**: High. This is a straightforward orchestrator pattern using established components. Risk is low if batch error handling is thought through upfront.

---

## DECISION CHECKLIST (FOR CODEX/SAM)

- [ ] Batch error handling: which strategy (retry all / per-thought / partial)?
- [ ] Boundaries storage: dedicated table, or map to kg_observations?
- [ ] Batch size default and env override strategy
- [ ] Extraction prompt: create in-task or reference from archive?
- [ ] Logging format: structured (JSON) or human-readable?

Once these are decided, task-1 is implementation-ready with zero ambiguity.
