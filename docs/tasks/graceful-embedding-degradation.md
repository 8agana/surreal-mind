# Graceful Embedding Degradation

**Status:** Not Started
**Priority:** High
**Created:** 2026-01-10

---

## Problem

When the OpenAI embedding API is down or fails, the `think` tool fails entirely and the thought is lost. The error message:

```
MCP error -32603: Embedding error: Failed to send embedding request to OpenAI API for model 'text-embedding-3-small'
```

This is a blocking failure when it should be graceful degradation.

---

## Current Behavior

1. User calls `think` with content
2. Tool attempts to embed content via OpenAI API
3. If embedding fails, entire operation fails
4. Thought content is **never saved** to database
5. User must retry or lose the thought

---

## Desired Behavior

1. User calls `think` with content
2. **Save thought to database FIRST** (without embedding)
3. Attempt embedding via OpenAI API
4. If embedding fails:
   - Mark thought with `needs_embedding: true` (or similar flag)
   - Log warning but return success with note that embedding is pending
   - Thought is preserved and searchable by content (just not by vector similarity)
5. Later: `maintain reembed` or similar can process pending embeddings

---

## Implementation Notes

- The `maintain reembed` flow already exists for re-embedding thoughts
- Need to decouple "persist thought" from "embed thought" in the think tool
- Consider a `pending_embeddings` table or flag on thoughts table
- Health check could report embedding API status

---

## Acceptance Criteria

- [ ] Thoughts are saved even when embedding API is down
- [ ] Failed embeddings are queued for retry
- [ ] `maintain` has a way to process pending embeddings
- [ ] User receives clear feedback about embedding status
- [ ] No data loss from transient API failures
