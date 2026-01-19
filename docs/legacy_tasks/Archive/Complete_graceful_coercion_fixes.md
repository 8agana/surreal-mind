# Graceful Input Coercion Fixes for surreal-mind

## Philosophy
Instead of validation errors, gracefully coerce all inputs to valid ranges. This provides:
- **Better UX**: Tools always work, no need to remember valid ranges
- **Perfect Security**: Any injection attempt becomes a valid value
- **Frictionless Experience**: CC can use tools without thinking about constraints

## Fix 1: Input Coercion for injection_scale

### Current Behavior (BAD)
```rust
// Probably something like:
if injection_scale < 0 || injection_scale > 3 {
    return Err("injection_scale must be 0-3");
}
```

### New Behavior (GOOD)
```rust
// In all think functions (convo_think, tech_think, inner_voice)
let scale = injection_scale
    .unwrap_or(0)           // Default to 0 if None
    .max(0)                 // Clamp minimum to 0
    .min(3);                // Clamp maximum to 3

// Now scale is ALWAYS 0-3, no matter what was passed in:
// -5 → 0
// 0 → 0
// 2 → 2
// 3 → 3
// 999 → 3
// "DROP TABLE" → 0 (after parse error)
```

### Alternative Using Match (More Explicit)
```rust
let scale = match injection_scale.unwrap_or(0) {
    s if s <= 0 => 0,
    s if s >= 3 => 3,
    s => s,
};
```

## Fix 2: mention_count Field Issue

### The Problem
The KG query at scales 2+ uses `ORDER BY mention_count DESC` but this field doesn't exist in the entity schema.

### Solution Options

#### Option A: Remove mention_count from ORDER BY
```rust
// Find the query that looks like:
let query = format!(
    "SELECT id, name, type, name_embedding, created_at, last_seen_at, last_accessed_at 
     FROM entity 
     WHERE type != 'test_entity' 
     ORDER BY mention_count DESC 
     LIMIT {}", 
    limit
);

// Change to use a field that exists:
let query = format!(
    "SELECT id, name, type, name_embedding, created_at, last_seen_at, last_accessed_at 
     FROM entity 
     WHERE type != 'test_entity' 
     ORDER BY last_accessed_at DESC NULLS LAST
     LIMIT {}", 
    limit
);
```

#### Option B: Add mention_count to Entity Schema
```rust
// In the entity creation/update code:
#[derive(Serialize, Deserialize)]
struct Entity {
    id: String,
    name: String,
    entity_type: String,
    name_embedding: Vec<f32>,
    created_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    last_accessed_at: Option<DateTime<Utc>>,
    mention_count: i32,  // Add this field
}

// Initialize mention_count to 0 for new entities
// Increment when entity is mentioned in thoughts
```

#### Option C: Use COALESCE for Backwards Compatibility
```rust
// This works even if field doesn't exist yet:
let query = format!(
    "SELECT id, name, type, name_embedding, created_at, last_seen_at, last_accessed_at 
     FROM entity 
     WHERE type != 'test_entity' 
     ORDER BY COALESCE(mention_count, 0) DESC, last_accessed_at DESC NULLS LAST
     LIMIT {}", 
    limit
);
```

## Fix 3: Significance Parameter Coercion

Apply same pattern to significance parameter:

```rust
// Convert any significance input to valid 0.0-1.0 range
let sig = significance
    .unwrap_or(0.5)         // Default to medium
    .max(0.0)               // Clamp minimum
    .min(1.0);              // Clamp maximum
```

## Implementation Locations

These changes need to be made in `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/src/main.rs`:

1. **convo_think function** (~line 1050-1100)
   - Add input coercion at start of function
   - Fix KG query around line 1500

2. **tech_think function** (~line 1150-1200)  
   - Add input coercion at start of function
   - Uses same KG retrieval function

3. **inner_voice function** (if it exists)
   - Add input coercion at start of function
   - Uses same KG retrieval function

4. **retrieve_from_kg function** (~line 1490-1530)
   - Fix the actual query with mention_count issue
   - This is where the ORDER BY needs updating

## Testing After Implementation

```bash
# Build
cargo build --release

# Test edge cases
# These should all work without errors:
injection_scale: -5  → uses 0
injection_scale: 0   → uses 0
injection_scale: 3   → uses 3
injection_scale: 999 → uses 3
injection_scale: null → uses 0

# Verify mention_count fix
# Scale 2-3 should work without SQL errors
```

## Benefits of This Approach

1. **Unbreakable**: No possible input can cause an error
2. **Secure**: Perfect input sanitization built-in
3. **User-Friendly**: CC never needs to remember valid ranges
4. **Future-Proof**: If we change ranges, just update the clamp values
5. **Graceful**: Everything "just works"

Good night Sam! These fixes should make surreal-mind bulletproof. 🚀