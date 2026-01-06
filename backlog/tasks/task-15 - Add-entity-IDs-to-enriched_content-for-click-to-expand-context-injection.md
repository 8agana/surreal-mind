---
id: task-15
title: Add entity IDs to enriched_content for click-to-expand context injection
status: To Do
assignee: []
created_date: '2026-01-05 22:15'
updated_date: '2026-01-05 22:15'
labels:
  - enhancement
  - legacymind_think
  - context-injection
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Enable users to do direct entity lookups from legacymind_think results by including entity IDs in enriched_content. Currently shows truncated names with similarity scores but no way to drill down into interesting entities.

## Current State
- `enriched_content` built in `inject_memories()` (src/server/db.rs:405-424)
- Format: `- (0.85) Entity Name [type]` (no ID)
- Entity IDs available in the `selected` vector at that point as `(id, sim, name, etype)`

## Desired State
- Format: `- [entity:abc123] (0.85) Entity Name [type]`
- Users can copy ID and use existing `memories_create` tool to get full entity details
- Alternatively: create dedicated `get_entity_by_id` tool for cleaner UX

## Implementation Notes
- IDs are already available in the selection loop (line 411)
- Simple format change: `format!("- [{}] ({:.2}) {} [{}]\n", id, sim, name, etype)`
- Need to handle empty etype case too
- Consider truncating very long IDs for readability (e.g., show first 8 chars)
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 enriched_content includes entity IDs in clickable format
- [ ] #2 Users can copy ID from think result and lookup full entity
- [ ] #3 Format is clean and readable (truncated IDs if needed)
- [ ] #4 Works for both entities with and without entity_type
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan

### Step 1: Modify enriched_content format in inject_memories()
**File**: `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/src/server/db.rs`
**Lines**: 405-424 (the enrichment block)

**Current code** (line 411-417):
```rust
for (i, (_id, sim, name, etype)) in selected.iter().take(5).enumerate() {
    if etype.is_empty() {
        s.push_str(&format!("- ({:.2}) {}\n", sim, name));
    } else {
        s.push_str(&format!("- ({:.2}) {} [{}]\n", sim, name, etype));
    }
```

**New code**:
```rust
for (i, (id, sim, name, etype)) in selected.iter().take(5).enumerate() {
    // Truncate ID to first 8 chars after table prefix for readability
    let short_id = id.split(':').nth(1).unwrap_or(id).chars().take(8).collect::<String>();
    
    if etype.is_empty() {
        s.push_str(&format!("- [{}] ({:.2}) {}\n", short_id, sim, name));
    } else {
        s.push_str(&format!("- [{}] ({:.2}) {} [{}]\n", short_id, sim, name, etype));
    }
```

**Changes**:
- Remove underscore prefix from `_id` to use the ID value
- Add `short_id` extraction (strips table prefix, takes first 8 chars)
- Insert `[short_id]` prefix in both format strings

### Step 2: Consider adding get_entity_by_id tool (optional enhancement)
**File**: `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/src/tools/knowledge_graph.rs`

**Why**: Currently users would need to use `legacymind_search` with exact ID match, which is clunky. A dedicated tool would be cleaner.

**Implementation**:
```rust
pub async fn handle_get_entity(
    &self,
    request: CallToolRequestParam,
) -> Result<CallToolResult> {
    let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
        message: "Missing parameters".into(),
    })?;
    
    let entity_id = args
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SurrealMindError::Validation {
            message: "Missing entity ID".into(),
        })?;
    
    // Try both kg_entities and kg_observations
    let sql = "SELECT meta::id(id) as id, name, data, created_at FROM kg_entities WHERE meta::id(id) = $id LIMIT 1; 
               SELECT meta::id(id) as id, name, data, created_at FROM kg_observations WHERE meta::id(id) = $id LIMIT 1;";
    
    let mut q = self.db.query(sql).bind(("id", entity_id)).await?;
    let entities: Vec<serde_json::Value> = q.take(0)?;
    let observations: Vec<serde_json::Value> = q.take(1)?;
    
    let result = if !entities.is_empty() {
        entities[0].clone()
    } else if !observations.is_empty() {
        observations[0].clone()
    } else {
        return Err(SurrealMindError::Validation {
            message: format!("Entity not found: {}", entity_id),
        });
    };
    
    Ok(CallToolResult::structured(result))
}
```

**Wire up in router.rs** (add to list_tools):
```rust
Tool {
    name: "get_entity_by_id".into(),
    title: Some("Get Entity By ID".into()),
    description: Some("Retrieve full details for a specific entity or observation by ID".into()),
    input_schema: get_entity_schema(),
    // ... rest of tool definition
}
```

**Add schema in schemas.rs**:
```rust
pub fn get_entity_schema() -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), json!("object"));
    
    let mut props = serde_json::Map::new();
    props.insert("id".to_string(), json!({
        "type": "string",
        "description": "Full entity ID (e.g., 'kg_entities:abc123' or just 'abc123')"
    }));
    
    map.insert("properties".to_string(), json!(props));
    map.insert("required".to_string(), json!(["id"]));
    map
}
```

### Step 3: Update call_tool router
**File**: `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/src/server/router.rs`

Add handler case (if implementing optional tool):
```rust
"get_entity_by_id" => self.handle_get_entity(request).await,
```

### Step 4: Testing
1. Run `legacymind_think` with injection_scale > 0
2. Verify enriched_content shows format: `- [abc12345] (0.85) Entity Name [type]`
3. Copy ID from output
4. Use `get_entity_by_id` (if implemented) or `legacymind_search` to retrieve full entity
5. Verify full entity details are returned

### Files to Modify
- **Required**: `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/src/server/db.rs` (inject_memories enrichment)
- **Optional**: `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/src/tools/knowledge_graph.rs` (new tool)
- **Optional**: `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/src/server/router.rs` (wire up tool)
- **Optional**: `/Users/samuelatagana/Projects/LegacyMind/surreal-mind/src/schemas.rs` (add schema)
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- IDs are already available in the selection loop (line 411)
- Simple format change: `format!("- [{}] ({:.2}) {} [{}]\n", id, sim, name, etype)`
- Need to handle empty etype case too
- Consider truncating very long IDs for readability (e.g., show first 8 chars)
<!-- SECTION:DESCRIPTION:END -->
<!-- SECTION:NOTES:END -->
