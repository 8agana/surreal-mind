# Bug: legacymind_think Memory Injection Content Not Included in Response

**ID**: `legacymind_think_memory_injection_bug`  
**Status**: Fixed  
**Priority**: high  
**Created**: 2026-01-04  
**Component**: `legacymind_think` tool, memory injection telemetry

## Problem Statement

The `legacymind_think` tool reports memory injection metrics in its response telemetry, but **the actual injected memory content is NOT included in the response payload**.

### What CC sees

```json
{
  "delegated_result": {
    "thought_id": "abc123",
    "embedding_model": "bge-small-en-v1.5",
    "embedding_dim": 384,
    "memories_injected": 20,
    "framework_enhanced": false
  },
  "telemetry": { ... }
}
```

### What CC needs

The actual memory content that was injected - the entity names, types, and similarity scores that enriched the thought. This is critical for CC's reasoning to understand what context was provided.

## Root Cause Analysis

### Call Stack

1. **`thinking.rs:run_convo()` or `run_technical()` lines ~330-380**
   - These methods call `inject_memories()` and get back `(mem_count, enriched_content)`
   - The tuple return is: `Result<(usize, Option<String>)>`
   - **BUG**: Only `mem_count` is used in the response JSON, `enriched_content` is DROPPED

2. **`db.rs:inject_memories()` lines ~165-380**
   - Correctly retrieves similar entities from KG
   - Correctly scores by cosine similarity
   - Correctly builds enriched text (lines ~330-345):

     ```rust
     let enriched = if !selected.is_empty() {
         let mut s = String::new();
         if let Some(sm) = submode { ... }
         s.push_str("Nearby entities:\n");
         for (i, (_id, sim, name, etype)) in selected.iter().take(5).enumerate() {
             // Formats: "- (0.92) EntityName [type]"
         }
     }
     ```

   - Returns: `Ok((memory_ids.len(), enriched))`
   - **This part is correct** - the enriched content exists and is computed

3. **Response Building in `thinking.rs:handle_legacymind_think()` lines ~1320-1380**
   - Calls the mode-specific function: `run_convo()` or `run_technical()`
   - Gets back: `(delegated_result, continuity_result)`
   - **Builds response with ONLY delegated_result** - never accesses the enriched memory content

### The Exact Issue

**File**: `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/src/tools/thinking.rs`

**Lines ~370-380** (in `run_technical`):

```rust
let (mem_count, _enriched) = self
    .inject_memories(
        &thought_id,
        &embedding,
        injection_scale_val,
        None,
        Some(&tool_name),
    )
    .await
    .unwrap_or((0, None));

let original_result = json!({
    "thought_id": thought_id,
    "embedding_model": self.get_embedding_metadata().1,
    "embedding_dim": self.embedder.dimensions(),
    "memories_injected": mem_count
    // ^^^ ONLY mem_count is used, _enriched is explicitly ignored!
});
```

**Same issue in `run_convo` lines ~330-345** - same pattern, `_enriched` discarded.

## Solution Approach

### Option 1: Include enriched_content in delegated_result

Add the enriched content string to the JSON response so CC can see what memories were actually injected.

**Pros**: Simple, CC gets the data in the main result object
**Cons**: Adds string content to response, may increase message size

### Option 2: Create separate memories field in response

Structure the response to include a `memories` object with both count and content.

**Pros**: Cleaner separation of concerns
**Cons**: Requires response structure change in callers

### Option 3: Both - for maximum transparency

Include enriched content in `delegated_result` AND add top-K memory details in main response.

**Pros**: Full transparency about what was injected
**Cons**: Largest response size increase

## Implementation Steps

1. **Modify `run_convo()` in `thinking.rs`** (line ~330):
   - Change `let (mem_count, _enriched)` to `let (mem_count, enriched)`
   - Add enriched content to `original_result` JSON

2. **Modify `run_technical()` in `thinking.rs`** (line ~370):
   - Same pattern as run_convo()

3. **Update response structure** (choose Option 1, 2, or 3)
   - Recommended: Option 1 (simplest, backward-compatible with existing mem_count field)

4. **Test**: Verify that CC receives enriched memory content in responses

## Acceptance Criteria

- [ ] legacymind_think response includes the enriched_content string
- [ ] Response shows actual memory entity names and similarity scores
- [ ] Both run_convo() and run_technical() return enriched memories
- [ ] Backward compatibility maintained (mem_count still reported)
- [ ] No clippy warnings (unused variable _enriched is eliminated)
- [ ] Manual test: call legacymind_think and verify memory content in response

## Test Results (Completed 2026-01-04)

**Fix Applied**: Modified `thinking.rs` to preserve and return `enriched_content` from `inject_memories()` in both `run_convo()` and `run_technical()` methods.

**Test Case**: CC called `legacymind_think` with hint="debug" after rebuild.

**Result - PASS**:

```json
{
  "memories_injected": 10,
  "enriched_content": "Nearby entities:\n- (0.59) Session 3 [event]\n- (0.58) A basic connectivity and functionality test of the...\n- (0.56) A clean startup of surreal-mind was achieved at 20...\n- (0.56) Rebuilt surreal-mind with 10-thought timeout_ms im...\n- (0.55) SurrealMind MCP [mcp_server]\n"
}
```

**Acceptance Criteria Status**:
- [x] legacymind_think response includes the enriched_content string
- [x] Response shows actual memory entity names and similarity scores
- [x] Both run_convo() and run_technical() return enriched memories
- [x] Backward compatibility maintained (mem_count still reported)
- [x] No clippy warnings (unused variable _enriched is eliminated)
- [x] Manual test: call legacymind_think and verify memory content in response

**Verification**: The enriched_content field now contains a formatted list of nearby entities with similarity scores (0.59-0.55), entity names, and types. CC can now see exactly what memories were injected during thinking operations.

## Technical Notes

- The enriched content is COMPUTED in `db.rs:inject_memories()` correctly
- The `_enriched` second return value is explicitly discarded (`_` prefix)
- This is a data flow issue, not a computation issue
- No database changes needed - data is already persisted correctly
- This affects all `legacymind_think` calls (all modes)
