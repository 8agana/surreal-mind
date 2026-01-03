# Think Tool Refactor: Sequential Simplification

## Date: 2025-09-06
## Author: CC (Claude Code) with Sam
## Status: Design Phase

## Overview

Major simplification of the thinking tool system from 5 specialized tools to 2 domain-focused tools with built-in sequential thinking and intelligent mode detection.

## Current State (5 Tools)

```
think_convo   → General conversation
think_plan    → Architecture/strategy  
think_debug   → Root cause analysis
think_build   → Implementation
think_stuck   → Lateral thinking
```

**Problems:**
- Artificial boundaries between thinking modes
- No continuity between thoughts
- User must choose the right tool
- Can't flow naturally from debugging → planning → building
- No revision or backtracking capability

## Proposed State (2 Tools)

```
legacymind_think   → Technical work (incorporates all modes)
photography_think  → Photography domain work
```

## Core Features

### 1. Sequential Thinking Built-In

```rust
pub struct SequentialThinkArgs {
    // Core content
    content: String,
    
    // Session management
    session_id: Option<String>,      // Continue or start new
    chain_id: String,                 // Thematic grouping
    
    // Sequential features
    thought_mode: Option<ThoughtMode>, // Auto-detected if not specified
    previous_thought_id: Option<String>,
    revises_thought: Option<String>,
    branch_from: Option<String>,
    hypothesis: Option<String>,
    
    // Control flow
    needs_more_thoughts: Option<bool>,
    estimated_remaining: Option<i32>,
    
    // Memory injection
    injection_scale: i32,            // 1-3 orbital mechanics
}

enum ThoughtMode {
    Continue,    // Default - continue current flow
    Revise,      // Explicitly revising earlier thought
    Branch,      // Exploring alternative path
    Conclude,    // Wrapping up session
    Auto,        // Let framework detect
}
```

### 2. Intelligent Mode Detection

Simple pattern-based detection in `detect_thinking_mode()`:

```rust
fn detect_thinking_mode(content: &str) -> ThinkingPattern {
    let lower = content.to_lowercase();
    
    if lower.contains("error") || lower.contains("failed") || 
       lower.contains("broken") || lower.contains("bug") {
        ThinkingPattern::Debugging
    } else if lower.contains("how should") || lower.contains("architecture") ||
              lower.contains("design") || lower.contains("approach") {
        ThinkingPattern::Planning
    } else if lower.contains("implement") || lower.contains("create") ||
              lower.contains("add") || lower.contains("build") {
        ThinkingPattern::Building
    } else if lower.contains("stuck") || lower.contains("not working") ||
              lower.contains("confused") {
        ThinkingPattern::Lateral
    } else {
        ThinkingPattern::Exploring
    }
}
```

### 3. Next-Thought Suggestions

Context-aware suggestions based on current thought:

```rust
fn suggest_next_thought(pattern: ThinkingPattern, content: &str) -> Option<String> {
    match pattern {
        ThinkingPattern::Debugging => {
            if content.contains("hypothesis") {
                Some("Test this hypothesis by checking...")
            } else if content.contains("found") {
                Some("Verify this is the root cause by...")
            } else {
                Some("Check the logs/error details for...")
            }
        },
        ThinkingPattern::Planning => {
            if content.contains("option") {
                Some("Evaluate trade-offs between options...")
            } else {
                Some("Consider the implementation steps...")
            }
        },
        ThinkingPattern::Building => {
            if content.contains("completed") {
                Some("Test the implementation with...")
            } else {
                Some("Next, implement the...")
            }
        },
        ThinkingPattern::Lateral => {
            Some("Try approaching from a different angle...")
        },
        _ => None
    }
}
```

### 4. Database Schema Updates

```sql
-- Session tracking
DEFINE TABLE thought_session SCHEMAFULL;
DEFINE FIELD session_id ON thought_session TYPE string;
DEFINE FIELD domain ON thought_session TYPE string; -- 'legacymind' or 'photography'
DEFINE FIELD started_at ON thought_session TYPE datetime;
DEFINE FIELD thought_count ON thought_session TYPE int;
DEFINE FIELD active_branch ON thought_session TYPE option<string>;
DEFINE FIELD hypothesis ON thought_session TYPE option<string>;
DEFINE FIELD concluded ON thought_session TYPE bool DEFAULT false;

-- Enhanced thoughts table
ALTER TABLE thoughts 
  ADD FIELD previous_thought_id TYPE option<record<thoughts>>
  ADD FIELD session_id TYPE option<record<thought_session>>
  ADD FIELD thinking_pattern TYPE option<string>
  ADD FIELD suggested_next TYPE option<string>
  ADD FIELD is_revision TYPE bool DEFAULT false
  ADD FIELD revises_thought_id TYPE option<record<thoughts>>
  ADD FIELD branch_id TYPE option<string>;

-- Revision tracking
DEFINE TABLE thought_revision SCHEMAFULL;
DEFINE FIELD original ON thought_revision TYPE record<thoughts>;
DEFINE FIELD revision ON thought_revision TYPE record<thoughts>;
DEFINE FIELD reason ON thought_revision TYPE string;
DEFINE FIELD created_at ON thought_revision TYPE datetime;

-- Branch tracking
DEFINE TABLE thought_branch SCHEMAFULL;
DEFINE FIELD branch_id ON thought_branch TYPE string;
DEFINE FIELD branched_from ON thought_branch TYPE record<thoughts>;
DEFINE FIELD branch_reason ON thought_branch TYPE string;
DEFINE FIELD selected ON thought_branch TYPE bool DEFAULT false;
```

### 5. Enhanced Memory Injection

Orbital mechanics now considers session context:

```rust
fn inject_memories(&self, embedding: &[f32], scale: i32, session_id: Option<&str>) -> Vec<Memory> {
    let mut memories = vec![];
    
    // First priority: thoughts from same session
    if let Some(sid) = session_id {
        let session_thoughts = self.fetch_session_thoughts(sid, 5);
        memories.extend(session_thoughts);
    }
    
    // Second priority: recent thoughts in same chain_id
    let chain_thoughts = self.fetch_chain_thoughts(&self.chain_id, 3);
    memories.extend(chain_thoughts);
    
    // Third priority: standard orbital mechanics
    let orbital_memories = self.standard_injection(embedding, scale);
    memories.extend(orbital_memories);
    
    memories
}
```

## Migration Plan

### Phase 1: Build New Tools (Week 1)
1. Create `legacymind_think` with all features
2. Create `photography_think` as simpler variant
3. Keep existing tools operational

### Phase 2: Parallel Running (Week 2)
1. Both old and new tools available
2. Encourage use of new tools
3. Monitor for issues

### Phase 3: Deprecate Old Tools (Week 3)
1. Mark old tools as deprecated
2. Add warnings when used
3. Provide migration messages

### Phase 4: Remove Old Tools (Week 4)
1. Remove think_convo, think_plan, think_debug, think_build, think_stuck
2. Clean up unused code
3. Update documentation

## Implementation Priority

1. **Core Sequential Logic** (Day 1-2)
   - Session management
   - Thought chaining
   - Previous thought injection

2. **Pattern Detection** (Day 3)
   - Mode detection function
   - Next-thought suggestions
   - Framework selection

3. **Database Updates** (Day 4)
   - Schema changes
   - Migration scripts
   - Relationship definitions

4. **Tool Implementation** (Day 5-6)
   - legacymind_think tool
   - photography_think tool
   - Testing

5. **Enhanced Injection** (Day 7)
   - Session-aware injection
   - Branch-aware retrieval
   - Testing

## Benefits

1. **Simplicity**: 2 tools instead of 5
2. **Natural Flow**: Thinking modes flow into each other
3. **Transparency**: See entire reasoning path
4. **Learning**: Revisions and branches are preserved
5. **Guidance**: Gentle suggestions keep thoughts flowing
6. **Domain Focus**: Technical vs creative separation

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Pattern detection too simple | Start simple, enhance based on usage |
| Lost specialization | Modes still exist, just auto-detected |
| Migration confusion | Keep old tools during transition |
| Session state complexity | Use SurrealDB's native graph features |

## Success Metrics

- Reduction in "wrong tool" moments
- Increased thought continuity (measurable via previous_thought_id usage)
- More natural conversation flow
- Ability to trace reasoning paths
- Successful revision tracking

## Example Usage

```rust
// Starting a debugging session
legacymind_think(
    content: "The HNSW index creation is failing with dimension mismatch",
    session_id: None,  // Starts new session
    chain_id: "hnsw-debug-20250906",
)
// Returns: thought_id: "abc123", suggested_next: "Check the embedding dimensions in config"

// Continuing with suggestion
legacymind_think(
    content: "Config shows 768 but embeddings are 1536 dimensions",
    session_id: Some("session_xyz"),
    previous_thought_id: Some("abc123"),
)
// Framework detects: Found root cause, shifting to solution
// Returns: suggested_next: "Update the config to match embedding dimensions"

// Revising earlier hypothesis
legacymind_think(
    content: "Actually, the config is correct - the embedder is using wrong model",
    session_id: Some("session_xyz"),
    thought_mode: Some(ThoughtMode::Revise),
    revises_thought: Some("abc123"),
)
```

## Next Steps

1. Review and approve design
2. Create implementation issues
3. Begin Phase 1 development
4. Set up testing framework
5. Document migration guide

---

*"Simplicity is the ultimate sophistication" - But with intelligence baked in*