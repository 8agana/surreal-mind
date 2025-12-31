---
id: doc-5
title: Testing Plan - kg_populate binary
type: other
created_date: '2025-12-31 23:24'
updated_date: '2025-12-31 23:51'
---
# Testing Plan — kg_populate binary

**Related task**: task-1 (Implement kg_populate orchestrator binary)  
**Related docs**: doc-1 through doc-4  
**Date**: 2025-12-31  
**Status**: Ready for testing

---

## TESTING CATEGORIES

### 1. Thought Fetching
- [ ] Correct WHERE clause (`extracted_to_kg = false`)
- [ ] Respects LIMIT (batch size)
- [ ] Orders by `created_at ASC` (oldest first)
- [ ] Returns empty vec when no unextracted thoughts
- [ ] Handles thoughts with empty content gracefully

### 2. Gemini Integration
- [ ] Prompt includes all thoughts in batch
- [ ] Prompt format matches kg_extraction_v1.md schema
- [ ] Response parsing handles `\`\`\`json` fences
- [ ] Response parsing handles plain JSON (no fences)
- [ ] Timeout is respected (KG_POPULATE_TIMEOUT_MS)
- [ ] Model override works (KG_POPULATE_MODEL)

### 3. KG Upserts - Entities
- [ ] New entity created with correct fields
- [ ] Existing entity found by name (no duplicate)
- [ ] source_thought_ids updated on existing entity
- [ ] extraction_batch_id set correctly
- [ ] extraction_confidence preserved from Gemini
- [ ] extraction_prompt_version set to "v1"

### 4. KG Upserts - Edges
- [ ] Edge created when both entities exist
- [ ] Edge skipped when source entity missing
- [ ] Edge skipped when target entity missing
- [ ] Existing edge found by (source, target, rel_type)
- [ ] source_thought_ids updated on existing edge
- [ ] type::thing() references created correctly

### 5. KG Upserts - Observations
- [ ] Name truncated to 50 chars + "..." for long content
- [ ] Uniqueness check by (name, source_thought_id)
- [ ] data field contains full content, context, tags
- [ ] confidence field set from Gemini response

### 6. KG Boundaries
- [ ] Created in kg_boundaries table (not kg_observations)
- [ ] All fields populated: rejected, reason, context, confidence
- [ ] source_thought_id links back correctly
- [ ] extraction_batch_id matches thought's batch

### 7. Thought Marking
- [ ] extracted_to_kg set to true after processing
- [ ] extraction_batch_id set to UUID
- [ ] extracted_at set to current timestamp
- [ ] Thoughts skipped by Gemini still marked as extracted

### 8. Idempotency
- [ ] Re-running doesn't re-fetch already extracted thoughts
- [ ] Re-running doesn't duplicate entities (name uniqueness)
- [ ] Re-running doesn't duplicate edges (triplet uniqueness)
- [ ] Re-running doesn't duplicate observations (name+thought uniqueness)
- [ ] Boundaries may be created multiple times (acceptable)

### 9. Error Handling
- [ ] Gemini timeout logged, batch marked as failed
- [ ] JSON parse failure logged, batch not marked as extracted
- [ ] Individual thought failure doesn't stop batch
- [ ] Database connection failure exits cleanly
- [ ] Config load failure shows helpful error

### 9a. Asymmetric Retry Behavior (Intentional Safety Design)
**Critical Design Decision**: When Gemini fails to parse the response as valid JSON, the entire batch is **NOT** marked as `extracted_to_kg = true`. This is intentional safety behavior.

**Rationale**:
- Network/timeout failures are temporary and recoverable → retry entire batch
- JSON parse failures indicate response corruption or schema mismatch → retry entire batch
- If we marked the batch as extracted despite parse failures, we'd lose those thoughts permanently
- The cost of re-processing is negligible vs. the cost of losing thoughts

**Test Validation**:
- [ ] Simulate Gemini returning malformed JSON (invalid JSON structure)
- [ ] Verify error is logged but thoughts remain `extracted_to_kg = false`
- [ ] Verify next run re-fetches and reprocesses the same batch
- [ ] Contrast with partial thought failures (one thought fails in a batch): only that thought stays unprocessed, batch is still marked as extracted

**Asymmetry Explained**:
- Individual thought processing failure → skip that thought, mark batch extracted (other thoughts processed successfully)
- Gemini response parsing failure → skip entire batch, don't mark extracted (no thoughts were reliably processed)

### 10. Batch Size Configuration
- [ ] Default batch size is 25
- [ ] KG_POPULATE_BATCH_SIZE env var overrides default
- [ ] Invalid env var falls back to default

### 11. Logging Output
- [ ] Startup message with config summary
- [ ] Batch processing message with count
- [ ] Per-batch extraction summary
- [ ] Final summary with all stats
- [ ] Errors logged to stderr with context

---

## TEST EXECUTION PLAN

### Phase 1: Manual Smoke Test
1. Ensure SurrealDB is running with legacymind schema
2. Insert a few test thoughts with `extracted_to_kg = false`
3. Run `cargo run --bin kg_populate`
4. Verify:
   - Console output shows progress
   - Thoughts marked as extracted in DB
   - KG tables have new entries

### Phase 2: Edge Case Testing
1. Run with no unextracted thoughts (should exit cleanly)
2. Run with malformed Gemini response (should log error, continue)
3. Run with entity references that don't exist (edges skipped)
4. Run with very long observation content (name truncation)

### Phase 3: Idempotency Verification
1. Run kg_populate on same dataset twice
2. Verify entity/edge/observation counts unchanged after second run
3. Verify no duplicate records in any KG table

### Phase 4: Performance Testing
1. Insert 100+ unextracted thoughts
2. Run with different batch sizes (10, 25, 50)
3. Measure time per batch
4. Check memory usage stability

---

## KNOWN LIMITATIONS

1. **No dry-run mode**: All operations modify database
2. **No rollback**: Failed batches leave partial state
3. **No parallelism**: Batches processed sequentially
4. **Boundaries not deduplicated**: Same boundary can be created multiple times

---

## FUTURE TESTING IMPROVEMENTS

1. Add `--dry-run` flag for safe testing
2. Add unit tests for parse_extraction_response()
3. Add integration tests with mock Gemini responses
4. Add property-based tests for uniqueness constraints
