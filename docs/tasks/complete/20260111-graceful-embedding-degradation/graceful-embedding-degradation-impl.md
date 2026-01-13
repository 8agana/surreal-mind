# Graceful Embedding Degradation - Implementation

**Status:** Implementation Complete
**Parent:** [graceful-embedding-degradation.md](graceful-embedding-degradation.md)
**Depends On:** None

---

## Goal

Decouple thought persistence from embedding generation so thoughts are never lost due to transient API failures.

---

## Phases

### Phase 1: Schema Extension

**Deliverables:**
- [x] Add `embedding_status` field to thoughts table
- [x] Add index on `embedding_status` for efficient queries

**Spec:**
```sql
DEFINE FIELD embedding_status ON TABLE thoughts TYPE option<string> DEFAULT "complete";
-- Values: "pending" | "complete" | "failed"
DEFINE INDEX idx_thoughts_embedding_status ON TABLE thoughts FIELDS embedding_status;
```

**Location:** `src/server/schema.rs`

---

### Phase 2: ThoughtBuilder Refactor

**Deliverables:**
- [x] Refactor `ThoughtBuilder::execute()` to save thought before embedding
- [x] Handle embedding failure gracefully (update status, return success with warning)
- [x] Preserve thought content even when embedding fails

**Spec:**

Current flow:
```
embed() → fail? → ERROR (thought lost)
       → ok?   → save() → return
```

New flow:
```
save(status="pending") → embed() → fail? → update(status="failed") → return OK + warning
                                 → ok?   → update(status="complete", embedding=vec) → return OK
```

**Location:** `src/tools/thinking.rs` - `ThoughtBuilder::execute()`

**Interface Change:**
- `execute()` returns `Result<(String, Vec<f32>, ContinuityResult, String)>` - 4th element is embedding_status
- Success response includes `"embedding_status": "complete"` or `"embedding_status": "pending"` with warning
- Callers in `runners.rs` updated to handle new return type and skip memory injection when embedding is empty

---

### Phase 3: Maintain Extension

**Deliverables:**
- [x] Add `embed_pending` subcommand to `maintain` tool
- [x] Query thoughts where `embedding_status = "pending"` or `"failed"`
- [x] Attempt embedding, update status on success/failure
- [x] Return count of processed/succeeded/failed

**Spec:**
```json
{
  "subcommand": "embed_pending",
  "limit": 100,
  "dry_run": false
}
```

Response:
```json
{
  "processed": 10,
  "succeeded": 8,
  "failed": 2,
  "remaining": 5
}
```

**Location:** `src/tools/maintenance.rs` - `handle_embed_pending()`

---

### Phase 4: Health Reporting

**Deliverables:**
- [x] Add `pending_embeddings` count to health check
- [ ] Add embedding API status probe (optional ping) - deferred, not critical

**Spec:**

Health response extension (via `maintain health_check_embeddings`):
```json
{
  "pending_embeddings": {
    "count": 5,
    "note": "Use 'maintain embed_pending' to retry these"
  }
}
```

**Location:** `src/tools/maintenance.rs` - `handle_health_check_embeddings()`

**Note:** API status probe deferred - the pending count is sufficient for monitoring. API failures surface naturally through the think tool's warning response.

---

## Implementation Notes

**Files Modified:**
- `src/server/schema.rs` - Added `embedding_status` field and index
- `src/tools/thinking.rs` - Refactored `ThoughtBuilder::execute()` to save-first flow
- `src/tools/thinking/runners.rs` - Updated `run_convo()` and `run_technical()` to handle 4-tuple return
- `src/tools/maintenance.rs` - Added `handle_embed_pending()` and pending count to health check
- `src/schemas.rs` - Added `embed_pending` to maintain subcommand enum
- `src/tools/detailed_help.rs` - Added `embed_pending` to howto documentation

**Additional Fixes (pre-existing Clippy warnings):**
- `src/tools/unified_search.rs` - Changed `&mut Vec<>` to `&mut []` per clippy::ptr_arg
- `src/tools/wander.rs` - Collapsed nested if per clippy::collapsible_if

---

## Review Notes

- All phases implemented
- `cargo fmt`, `cargo clippy -- -D warnings`, `cargo check` pass
- Release binary built and launchd service restarted
- CHANGELOG.md updated

---

## Testing

See [graceful-embedding-degradation-testing.md](graceful-embedding-degradation-testing.md)
