# Priority 1 Fixes for surreal-mind KG Retrieval

## Context
We just implemented KG-only retrieval to fix performance issues. The system now pulls from ~100 KG entities instead of thousands of thoughts, and scale 3 works without timeout! However, your code review identified critical issues that need fixing.

## Your Mission
Implement these Priority 1 fixes from your code review. Be surgical and precise - the architecture is working, just needs these specific corrections.

## Priority 1 Fixes Required

### 1. Fix memories_injected Count (Line 1299)
**Problem**: Returns 0 instead of actual count
**Location**: `src/main.rs:1299`
**Current Code**:
```rust
memories_injected: 0, // Wrong!
```
**Fix**: Should return `kg_memories.len()`

### 2. Update Validation Message (Lines 1068-1073)
**Problem**: Still mentions scales 0-5, but we only support 0-3 now
**Location**: `src/main.rs:1068-1073`
**Current**: Message says "injection_scale must be 0-5"
**Fix**: Update to "injection_scale must be 0-3" and update the explanation to match new orbit system (Mercury, Venus, Mars only)

### 3. Batch Neighbor Queries (Lines 1496-1528)
**Problem**: N+1 query problem - making separate query for each neighbor
**Location**: `src/main.rs:1496-1528` in `retrieve_from_kg()`
**Current**: Loop with individual queries for each neighbor
**Fix**: Batch the neighbor queries into a single SurrealDB query using IN clause or similar

### 4. Wire Up Config System
**Problem**: Config system exists but isn't being used
**Files**: 
- `src/config.rs` - Config loader (complete)
- `surreal_mind.toml` - Config file (complete)
- `src/main.rs` - Still using env vars instead of config

**Fix**: 
- Load config at startup
- Replace env var lookups with config values
- Use submode profiles for defaults (injection_scale, frameworks, etc.)

## Testing After Fixes
1. Verify memories_injected shows correct count
2. Test validation rejects scale 4-5 with correct message
3. Confirm neighbor queries are batched (check query logs)
4. Verify config values override env vars

## Important Notes
- Don't change the core KG retrieval logic - it's working!
- Don't implement partial timeout results (skip for now)
- Don't add vector search fallback (not needed with KG)
- Keep changes surgical and focused

## Files to Edit
1. `src/main.rs` - All fixes except config loading
2. Maybe `src/config.rs` - Only if config loading needs adjustment

Good luck! These fixes will complete our KG-only retrieval implementation.