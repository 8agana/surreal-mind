# Warp: Build KG-Only Retrieval System

## Context
We're refactoring surreal-mind to pull memories from the Knowledge Graph (KG) instead of directly from thoughts. This will dramatically improve performance since KG has far fewer entities than raw thoughts.

## Your Task
Build the KG-only retrieval system in the warp-mods worktree. Focus on clean, methodical implementation.

## Architecture Overview
```
Thoughts → Auto-extract → KG entities → Bidirectional retrieval → Memory injection
```

## Implementation Tasks

### 1. Create KG Retrieval Function
Replace the current thought-based retrieval with KG-based:

```rust
// In src/main.rs, create new function
async fn retrieve_from_kg(
    &self,
    query_embedding: &[f32],
    injection_scale: u8,  // 0-3 only now
    submode: &str,
) -> Result<Vec<KGMemory>, McpError> {
    // Query KG entities instead of thoughts
    // Use orbital mechanics on KG entities
    // Return relevant entities with their relationships
}
```

### 2. Implement Inner Voice Auto-Extract
When `inner_voice` is called, automatically extract entities:

```rust
// In inner_voice handler
async fn auto_extract_to_kg(&self, thought_id: &str, content: &str) -> Result<()> {
    // 1. Extract entities (use simple keyword extraction for now)
    //    - Split on common delimiters
    //    - Filter stopwords
    //    - Extract technical terms, proper nouns
    
    // 2. Create/update KG entities
    //    - Check if entity exists (by name)
    //    - Create if new, update access_count if exists
    
    // 3. Create "mentions" edges
    //    - Edge from thought to each entity
    //    - Weight based on frequency in text
}
```

### 3. KG Entity Schema Updates
```rust
// Ensure KG entities have orbital properties
struct KGEntity {
    id: String,
    name: String,
    entity_type: String,
    embedding: Vec<f32>,
    
    // Orbital mechanics
    mass: f32,           // significance
    orbit_radius: f32,   // distance from center
    velocity: f32,       // access frequency
    last_accessed: DateTime,
    access_count: u32,
}
```

### 4. Update Injection Scale Logic
- Scale 0: No injection
- Scale 1: Mercury orbit (immediate context) - DEFAULT
- Scale 2: Venus/Earth (recent work)
- Scale 3: Mars (foundational knowledge) - MAXIMUM

Remove scales 4-5 entirely.

## Key Points
- Pull from KG entities, NOT thoughts
- Thoughts contribute TO the KG via auto-extract
- Use orbital mechanics on KG entities
- Keep it simple and working first, optimize later
- Test with small examples before full implementation

## File Locations
- Main retrieval: `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/src/main.rs`
- KG operations: Look for `knowledgegraph_create` handler
- Inner voice: Search for `inner_voice` handler

## Success Criteria
1. Retrieval queries KG entities instead of thoughts
2. Inner voice auto-creates KG entities and mentions edges
3. Scale 3 doesn't timeout (because fewer KG entities than thoughts)
4. All existing tests still pass