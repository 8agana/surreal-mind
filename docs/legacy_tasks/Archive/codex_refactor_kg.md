# Codex: Refactor Existing Retrieval to Use KG System

## Context
Warp is building a new KG-only retrieval system. Your job is to refactor the existing code to use that new system once Warp has the base implementation working.

## Your Task
Work in the cc-mods worktree to refactor existing retrieval code. Focus on architectural analysis and clean integration.

## Current Problem Analysis
The current system times out because:
1. It runs cosine similarity on ALL thoughts (even with 768 dims)
2. Every thought is compared at high injection scales
3. O(n) where n = thoughts × dimensions

## Refactoring Tasks

### 1. Analyze Current Retrieval Flow
Map out exactly how the current system works:
- `retrieve_memories_with_injection()` in src/main.rs
- How it queries thoughts
- How it calculates orbital proximity
- How it sorts and filters results

### 2. Redirect to KG Retrieval
Replace thought retrieval with KG retrieval:

```rust
// OLD: Query thoughts directly
let thoughts = db.query("SELECT * FROM thoughts").await?;

// NEW: Query KG entities
let entities = db.query("SELECT * FROM kg_entities WHERE entity_type != 'private'").await?;
```

### 3. Update Memory Injection Logic
Modify how memories are injected into context:

```rust
// Instead of injecting thought content directly
// Inject KG entity descriptions with their relationships

struct InjectedMemory {
    entity: KGEntity,
    related_entities: Vec<(String, f32)>,  // (entity_id, edge_weight)
    source_thoughts: Vec<String>,          // thought_ids that mention this
}
```

### 4. Optimize Submode-Specific Retrieval
Each submode should have different KG traversal patterns:

```rust
match submode {
    "plan" => {
        // Traverse deeper in KG (2-3 hops)
        // Focus on "depends_on", "blocks" relationships
    },
    "build" => {
        // Shallow traversal (1 hop)
        // Focus on "implements", "uses" relationships
    },
    "debug" => {
        // Follow "causes", "fixes" relationships
        // Recent entities weighted higher
    }
}
```

### 5. Performance Optimizations
- Cache KG entity embeddings in memory (they change less than thoughts)
- Pre-compute common relationship paths
- Use relationship strength to prune search space

## Integration Points
Look for these functions to refactor:
- `retrieve_memories_with_injection()`
- `search_thoughts_handler()`
- `convo_think()` and `tech_think()` memory injection
- Anywhere that queries `thoughts` table directly

## Key Principles
1. **Never query thoughts table for retrieval** - only KG
2. **Thoughts are write-only** - they feed the KG but aren't read directly
3. **KG is the memory** - all retrieval goes through it
4. **Maintain backward compatibility** - existing APIs should still work

## Success Metrics
1. Scale 3 injection completes in <5 seconds
2. Memory quality improves (more relevant, less noise)
3. All existing tests pass
4. Clean separation between thought storage and memory retrieval

## Coordination with Warp
- Wait for Warp's base KG retrieval implementation
- Review Warp's `retrieve_from_kg()` function signature
- Ensure your refactoring matches Warp's interface
- Test integration between both changes