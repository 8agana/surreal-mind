# Think Tool Refactor: Sequential Simplification (Enhanced)

## Date: 2025-09-06 (Revised 2025-09-07)
## Authors: CC (Claude Code) + Warp (Opus 4.1 convergence)
## Status: Design Phase - Enhanced with Sequential Thinking

## Overview

Major simplification of the thinking tool system from 5 specialized tools to 2 domain-focused tools with built-in sequential thinking, meta-cognitive awareness, and intelligent mode detection.

**Key Enhancement**: Integration of sequential thinking's meta-cognitive features with CC's session management architecture.

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
- No meta-cognitive awareness (confidence, uncertainty)
- No hypothesis verification loop

## Proposed State (2 Tools)

```
legacymind_think   → Technical work (incorporates all modes + meta-cognition)
photography_think  → Photography domain work
```

## Core Features

### 1. Enhanced Sequential Thinking

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
    
    // Meta-cognitive features (NEW)
    confidence: Option<f32>,          // 0.0-1.0 certainty level
    hypothesis: Option<String>,        // For verification loop
    needs_verification: Option<bool>,  // Request memory verification
    
    // Control flow (ENHANCED)
    needs_more_thoughts: Option<bool>,
    estimated_remaining: Option<i32>,
    thought_depth: Option<ThoughtDepth>, // Surface/Deep/Exhaustive
    
    // Memory injection
    injection_scale: i32,            // 1-3 orbital mechanics
}

enum ThoughtMode {
    Continue,    // Default - continue current flow
    Revise,      // Explicitly revising earlier thought
    Branch,      // Exploring alternative path
    Question,    // Meta-questioning current approach (NEW)
    Stuck,       // Explicitly stuck, need lateral thinking (NEW)
    Hypothesis,  // Testing a specific hypothesis (NEW)
    Conclude,    // Wrapping up session
    Auto,        // Let framework detect
}

enum ThoughtDepth {
    Surface,     // Quick, high-level thinking
    Standard,    // Default depth
    Deep,        // Thorough exploration
    Exhaustive,  // Complete analysis with all branches
}
```

### 2. Enhanced Intelligent Mode Detection

Pattern-based detection with confidence scoring:

```rust
struct ThinkingAnalysis {
    pattern: ThinkingPattern,
    confidence: f32,           // How confident in the detection
    suggested_depth: ThoughtDepth,
    needs_hypothesis: bool,
}

fn analyze_thinking_needs(content: &str, context: &SessionContext) -> ThinkingAnalysis {
    let lower = content.to_lowercase();
    let mut confidence = 0.8; // Base confidence
    
    // Adjust based on question marks, uncertainty words
    if lower.contains("maybe") || lower.contains("might") || 
       lower.contains("possibly") || content.matches('?').count() > 2 {
        confidence *= 0.7;
    }
    
    // Detect if stuck or needs lateral thinking
    if lower.contains("stuck") || lower.contains("not sure") || 
       lower.contains("confused") || lower.contains("lost") {
        return ThinkingAnalysis {
            pattern: ThinkingPattern::Lateral,
            confidence: confidence * 0.9,
            suggested_depth: ThoughtDepth::Deep,
            needs_hypothesis: false,
        };
    }
    
    // Detect debugging with hypothesis generation
    if lower.contains("error") || lower.contains("failed") || 
       lower.contains("broken") || lower.contains("bug") {
        return ThinkingAnalysis {
            pattern: ThinkingPattern::Debugging,
            confidence,
            suggested_depth: ThoughtDepth::Deep,
            needs_hypothesis: true, // Debugging benefits from hypotheses
        };
    }
    
    // Planning detection
    if lower.contains("how should") || lower.contains("architecture") ||
       lower.contains("design") || lower.contains("approach") {
        return ThinkingAnalysis {
            pattern: ThinkingPattern::Planning,
            confidence,
            suggested_depth: ThoughtDepth::Standard,
            needs_hypothesis: false,
        };
    }
    
    // Implementation/building
    if lower.contains("implement") || lower.contains("create") ||
       lower.contains("add") || lower.contains("build") {
        return ThinkingAnalysis {
            pattern: ThinkingPattern::Building,
            confidence,
            suggested_depth: ThoughtDepth::Standard,
            needs_hypothesis: false,
        };
    }
    
    // Default to exploring with lower confidence
    ThinkingAnalysis {
        pattern: ThinkingPattern::Exploring,
        confidence: confidence * 0.6,
        suggested_depth: ThoughtDepth::Surface,
        needs_hypothesis: false,
    }
}
```

### 3. Hypothesis Verification Loop (NEW)

```rust
struct HypothesisVerification {
    hypothesis: String,
    supporting_memories: Vec<Memory>,
    contradicting_memories: Vec<Memory>,
    confidence_score: f32,
    suggested_revision: Option<String>,
}

impl ThinkingService {
    async fn verify_hypothesis(
        &self,
        hypothesis: &str,
        session_id: &str,
        embedding: &[f32],
    ) -> Result<HypothesisVerification> {
        // Generate hypothesis embedding
        let hyp_embedding = self.embed_text(hypothesis).await?;
        
        // Search for relevant memories
        let memories = self.search_memories_for_verification(
            &hyp_embedding,
            100, // Larger candidate pool for verification
        ).await?;
        
        // Classify memories as supporting or contradicting
        let mut supporting = vec![];
        let mut contradicting = vec![];
        
        for memory in memories {
            let similarity = cosine_similarity(&hyp_embedding, &memory.embedding);
            
            // Analyze semantic relationship
            if memory.content.contains("not") || memory.content.contains("false") ||
               memory.content.contains("incorrect") {
                if similarity > 0.7 {
                    contradicting.push(memory);
                }
            } else if similarity > 0.75 {
                supporting.push(memory);
            }
        }
        
        // Calculate confidence
        let total_evidence = supporting.len() + contradicting.len();
        let confidence = if total_evidence > 0 {
            supporting.len() as f32 / total_evidence as f32
        } else {
            0.5 // No evidence = uncertain
        };
        
        // Generate revision suggestion if confidence is low
        let suggested_revision = if confidence < 0.4 {
            Some(format!("Consider revising hypothesis based on {} contradicting memories", 
                        contradicting.len()))
        } else {
            None
        };
        
        Ok(HypothesisVerification {
            hypothesis: hypothesis.to_string(),
            supporting_memories: supporting,
            contradicting_memories: contradicting,
            confidence_score: confidence,
            suggested_revision,
        })
    }
}
```

### 4. Meta-Cognitive Next-Thought Suggestions (ENHANCED)

Context-aware suggestions with confidence tracking:

```rust
struct NextThoughtSuggestion {
    suggestion: String,
    rationale: String,
    suggested_mode: ThoughtMode,
    suggested_depth: ThoughtDepth,
    confidence_threshold: f32, // Don't proceed if confidence below this
}

fn suggest_next_thought(
    pattern: ThinkingPattern, 
    content: &str,
    confidence: f32,
    session_context: &SessionContext,
) -> Option<NextThoughtSuggestion> {
    // Meta-cognitive check: Are we too uncertain to continue?
    if confidence < 0.3 {
        return Some(NextThoughtSuggestion {
            suggestion: "Step back and reconsider the approach - confidence is very low",
            rationale: "Current thinking shows high uncertainty",
            suggested_mode: ThoughtMode::Question,
            suggested_depth: ThoughtDepth::Deep,
            confidence_threshold: 0.5,
        });
    }
    
    // Check if we're in a revision loop
    if session_context.revision_count > 3 {
        return Some(NextThoughtSuggestion {
            suggestion: "Consider branching to explore an alternative approach",
            rationale: "Multiple revisions suggest current path may be problematic",
            suggested_mode: ThoughtMode::Branch,
            suggested_depth: ThoughtDepth::Standard,
            confidence_threshold: 0.4,
        });
    }
    
    match pattern {
        ThinkingPattern::Debugging => {
            if content.contains("hypothesis") && confidence > 0.6 {
                Some(NextThoughtSuggestion {
                    suggestion: "Test this hypothesis by checking specific evidence",
                    rationale: "Hypothesis formed with reasonable confidence",
                    suggested_mode: ThoughtMode::Hypothesis,
                    suggested_depth: ThoughtDepth::Deep,
                    confidence_threshold: 0.5,
                })
            } else if content.contains("found") {
                Some(NextThoughtSuggestion {
                    suggestion: "Verify this is the root cause and not a symptom",
                    rationale: "Potential cause identified, needs verification",
                    suggested_mode: ThoughtMode::Continue,
                    suggested_depth: ThoughtDepth::Standard,
                    confidence_threshold: 0.6,
                })
            } else {
                Some(NextThoughtSuggestion {
                    suggestion: "Gather more diagnostic information",
                    rationale: "Insufficient data for hypothesis formation",
                    suggested_mode: ThoughtMode::Continue,
                    suggested_depth: ThoughtDepth::Deep,
                    confidence_threshold: 0.4,
                })
            }
        },
        ThinkingPattern::Planning => {
            if confidence < 0.5 {
                Some(NextThoughtSuggestion {
                    suggestion: "Identify and resolve key uncertainties before proceeding",
                    rationale: "Planning requires higher confidence to be effective",
                    suggested_mode: ThoughtMode::Question,
                    suggested_depth: ThoughtDepth::Deep,
                    confidence_threshold: 0.6,
                })
            } else {
                Some(NextThoughtSuggestion {
                    suggestion: "Evaluate trade-offs and implementation feasibility",
                    rationale: "Plan formed, needs practical validation",
                    suggested_mode: ThoughtMode::Continue,
                    suggested_depth: ThoughtDepth::Standard,
                    confidence_threshold: 0.5,
                })
            }
        },
        ThinkingPattern::Lateral => {
            Some(NextThoughtSuggestion {
                suggestion: "Try inverting the problem or approaching from a different domain",
                rationale: "Lateral thinking requires perspective shift",
                suggested_mode: ThoughtMode::Branch,
                suggested_depth: ThoughtDepth::Deep,
                confidence_threshold: 0.3, // Lower threshold for creative exploration
            })
        },
        _ => None
    }
}
```

### 5. Enhanced Memory Injection with Meta-Context

Prioritized injection considering confidence and session state:

```rust
fn inject_memories_with_meta(
    &self, 
    embedding: &[f32], 
    scale: i32, 
    session_id: Option<&str>,
    confidence: f32,
    thought_mode: ThoughtMode,
) -> Vec<Memory> {
    let mut memories = vec![];
    let mut weights = HashMap::new();
    
    // Priority 1: Current session thoughts (highest weight)
    if let Some(sid) = session_id {
        let session_thoughts = self.fetch_session_thoughts(sid, 5);
        for thought in session_thoughts {
            weights.insert(thought.id.clone(), 1.0);
            memories.push(thought);
        }
    }
    
    // Priority 2: Revision history if in revision mode
    if matches!(thought_mode, ThoughtMode::Revise) {
        let revision_thoughts = self.fetch_revision_chain(session_id, 3);
        for thought in revision_thoughts {
            weights.insert(thought.id.clone(), 0.9);
            memories.push(thought);
        }
    }
    
    // Priority 3: Low confidence = inject more diverse memories
    if confidence < 0.5 {
        let diverse_memories = self.fetch_diverse_memories(embedding, 10);
        for memory in diverse_memories {
            weights.insert(memory.id.clone(), 0.7);
            memories.push(memory);
        }
    }
    
    // Priority 4: Chain thoughts (thematic continuity)
    let chain_thoughts = self.fetch_chain_thoughts(&self.chain_id, 3);
    for thought in chain_thoughts {
        weights.insert(thought.id.clone(), 0.6);
        memories.push(thought);
    }
    
    // Priority 5: Standard orbital mechanics
    let orbital_memories = self.standard_injection(embedding, scale);
    for memory in orbital_memories {
        weights.entry(memory.id.clone()).or_insert(0.5);
        memories.push(memory);
    }
    
    // Sort by weighted relevance
    memories.sort_by(|a, b| {
        let weight_a = weights.get(&a.id).unwrap_or(&0.5);
        let weight_b = weights.get(&b.id).unwrap_or(&0.5);
        weight_b.partial_cmp(weight_a).unwrap()
    });
    
    // Return top memories based on scale
    let limit = match scale {
        1 => 5,
        2 => 10,
        3 => 20,
        _ => 10,
    };
    
    memories.truncate(limit);
    memories
}
```

### 6. Database Schema Updates (ENHANCED)

```sql
-- Session tracking with meta-cognitive state
DEFINE TABLE thought_session SCHEMAFULL;
DEFINE FIELD session_id ON thought_session TYPE string;
DEFINE FIELD domain ON thought_session TYPE string; -- 'legacymind' or 'photography'
DEFINE FIELD started_at ON thought_session TYPE datetime;
DEFINE FIELD thought_count ON thought_session TYPE int;
DEFINE FIELD revision_count ON thought_session TYPE int DEFAULT 0;
DEFINE FIELD branch_count ON thought_session TYPE int DEFAULT 0;
DEFINE FIELD active_branch ON thought_session TYPE option<string>;
DEFINE FIELD hypothesis ON thought_session TYPE option<string>;
DEFINE FIELD avg_confidence ON thought_session TYPE float DEFAULT 0.5;
DEFINE FIELD concluded ON thought_session TYPE bool DEFAULT false;
DEFINE FIELD conclusion_summary ON thought_session TYPE option<string>;

-- Enhanced thoughts table with meta-cognitive fields
ALTER TABLE thoughts 
  ADD FIELD previous_thought_id TYPE option<record<thoughts>>
  ADD FIELD session_id TYPE option<record<thought_session>>
  ADD FIELD thinking_pattern TYPE option<string>
  ADD FIELD confidence TYPE float DEFAULT 0.7
  ADD FIELD thought_mode TYPE option<string>
  ADD FIELD thought_depth TYPE option<string>
  ADD FIELD suggested_next TYPE option<string>
  ADD FIELD is_revision TYPE bool DEFAULT false
  ADD FIELD revises_thought_id TYPE option<record<thoughts>>
  ADD FIELD branch_id TYPE option<string>
  ADD FIELD hypothesis_verification TYPE option<object>;

-- Hypothesis tracking
DEFINE TABLE hypothesis_verification SCHEMAFULL;
DEFINE FIELD hypothesis ON hypothesis_verification TYPE string;
DEFINE FIELD thought_id ON hypothesis_verification TYPE record<thoughts>;
DEFINE FIELD supporting_count ON hypothesis_verification TYPE int;
DEFINE FIELD contradicting_count ON hypothesis_verification TYPE int;
DEFINE FIELD confidence_score ON hypothesis_verification TYPE float;
DEFINE FIELD verified_at ON hypothesis_verification TYPE datetime;
DEFINE FIELD outcome ON hypothesis_verification TYPE option<string>;

-- Meta-cognitive tracking
DEFINE TABLE thinking_metrics SCHEMAFULL;
DEFINE FIELD session_id ON thinking_metrics TYPE record<thought_session>;
DEFINE FIELD avg_confidence ON thinking_metrics TYPE float;
DEFINE FIELD revision_ratio ON thinking_metrics TYPE float;
DEFINE FIELD hypothesis_success_rate ON thinking_metrics TYPE float;
DEFINE FIELD most_effective_pattern ON thinking_metrics TYPE string;
DEFINE FIELD computed_at ON thinking_metrics TYPE datetime;
```

## Implementation Approach

### Phase 1: Core Infrastructure (Days 1-3)

1. **Session Management**
   - Implement `ThoughtSession` struct and database operations
   - Add session tracking to existing think tools for backward compatibility
   - Create session continuation logic

2. **Meta-Cognitive Layer**
   - Add confidence tracking to all thoughts
   - Implement `ThinkingAnalysis` for pattern detection
   - Create hypothesis verification service

3. **Enhanced Memory Injection**
   - Implement weighted memory retrieval
   - Add session-aware and confidence-aware injection
   - Test with existing think tools

### Phase 2: New Tools (Days 4-6)

1. **legacymind_think Implementation**
   ```rust
   pub async fn legacymind_think(
       args: SequentialThinkArgs,
       ctx: &Context,
   ) -> Result<ThinkResponse> {
       // Analyze thinking needs
       let analysis = analyze_thinking_needs(&args.content, &ctx.session);
       
       // Get or create session
       let session = get_or_create_session(
           args.session_id,
           "legacymind",
           &ctx.db,
       ).await?;
       
       // Verify hypothesis if provided
       let verification = if let Some(hyp) = &args.hypothesis {
           Some(verify_hypothesis(hyp, &session.id, &embedding).await?)
       } else {
           None
       };
       
       // Enhanced memory injection
       let memories = inject_memories_with_meta(
           &embedding,
           args.injection_scale,
           Some(&session.id),
           args.confidence.unwrap_or(analysis.confidence),
           args.thought_mode.unwrap_or(ThoughtMode::Auto),
       );
       
       // Store thought with all metadata
       let thought = store_sequential_thought(
           args,
           analysis,
           verification,
           memories,
           session,
       ).await?;
       
       // Generate next suggestion
       let next_suggestion = suggest_next_thought(
           analysis.pattern,
           &args.content,
           analysis.confidence,
           &session.context,
       );
       
       Ok(ThinkResponse {
           thought_id: thought.id,
           session_id: session.id,
           confidence: analysis.confidence,
           pattern_detected: analysis.pattern,
           memories_injected: memories.len(),
           hypothesis_verification: verification,
           suggested_next: next_suggestion,
           needs_more_thoughts: args.needs_more_thoughts.unwrap_or(
               next_suggestion.is_some()
           ),
       })
   }
   ```

2. **photography_think Implementation**
   - Simpler variant focused on creative work
   - Less emphasis on debugging/hypothesis
   - More weight on aesthetic and technical memories

### Phase 3: Migration & Testing (Days 7-10)

1. **Backward Compatibility Layer**
   ```rust
   // Wrapper for old tools during migration
   pub async fn think_debug_compat(args: OldThinkArgs) -> Result<OldResponse> {
       let new_args = SequentialThinkArgs {
           content: args.content,
           session_id: None,
           chain_id: generate_chain_id("debug"),
           thought_mode: Some(ThoughtMode::Auto),
           injection_scale: args.injection_scale,
           confidence: None,
           hypothesis: None,
           // ... map other fields
       };
       
       let response = legacymind_think(new_args).await?;
       
       // Convert to old response format
       Ok(OldResponse {
           thought_id: response.thought_id,
           memories_injected: response.memories_injected,
           // ... map fields
       })
   }
   ```

2. **Testing Strategy**
   - Unit tests for each new component
   - Integration tests for session continuity
   - Hypothesis verification accuracy tests
   - Performance tests for weighted injection

### Phase 4: Deprecation (Week 2)

1. Add deprecation warnings to old tools
2. Update documentation with migration guide
3. Monitor usage patterns
4. Assist users in transitioning

## Configuration (env-first)

```bash
# Meta-cognitive features
SURR_ENABLE_META_COGNITION=true
SURR_MIN_CONFIDENCE=0.3
SURR_MAX_REVISION_LOOPS=5
SURR_HYPOTHESIS_CANDIDATES=100

# Session management
SURR_SESSION_TIMEOUT_HOURS=24
SURR_MAX_SESSION_THOUGHTS=1000
SURR_SESSION_MEMORY_PRIORITY=true

# Thinking patterns
SURR_AUTO_DETECT_PATTERNS=true
SURR_PATTERN_CONFIDENCE_THRESHOLD=0.5
SURR_SUGGEST_NEXT_THOUGHT=true

# Branching
SURR_MAX_BRANCHES_PER_SESSION=10
SURR_AUTO_PRUNE_BRANCHES=true
```

## Benefits Over Current System

1. **Continuity**: Thoughts build on each other naturally
2. **Meta-awareness**: System knows when it's uncertain or stuck
3. **Learning**: Tracks what works via hypothesis verification
4. **Transparency**: Full reasoning paths are traceable
5. **Adaptability**: Auto-adjusts depth and approach based on confidence
6. **Simplicity**: 2 tools instead of 5, with intelligent defaults

## Success Metrics

- **Session continuity**: 80%+ thoughts reference previous in session
- **Hypothesis accuracy**: 70%+ verification confidence
- **Revision effectiveness**: <3 revisions average per session
- **Pattern detection accuracy**: 85%+ match with user intent
- **Memory relevance**: 90%+ injected memories rated relevant

## Example Usage Scenarios

### Debugging with Hypothesis Loop
```rust
// Initial problem statement
let r1 = legacymind_think(SequentialThinkArgs {
    content: "HNSW index creation fails with dimension mismatch",
    chain_id: "hnsw-debug-20250907",
    ..Default::default()
}).await?;
// Returns: pattern=Debugging, suggests gathering more info

// Following suggestion
let r2 = legacymind_think(SequentialThinkArgs {
    content: "Logs show expected 768 dims but receiving 1536",
    session_id: Some(r1.session_id),
    previous_thought_id: Some(r1.thought_id),
    ..Default::default()
}).await?;
// Returns: suggests hypothesis formation

// Hypothesis formation
let r3 = legacymind_think(SequentialThinkArgs {
    content: "Hypothesis: The embedder is using wrong model (3-small instead of BGE)",
    session_id: Some(r1.session_id),
    hypothesis: Some("Embedder using text-embedding-3-small instead of BGE"),
    needs_verification: Some(true),
    ..Default::default()
}).await?;
// Returns: hypothesis_verification with supporting memories

// Confident solution
let r4 = legacymind_think(SequentialThinkArgs {
    content: "Confirmed: OPENAI_API_KEY was set, triggering wrong embedder",
    session_id: Some(r1.session_id),
    confidence: Some(0.95),
    thought_mode: Some(ThoughtMode::Conclude),
    ..Default::default()
}).await?;
// Returns: session concluded with solution
```

### Planning with Low Confidence Branching
```rust
// Uncertain architecture decision
let r1 = legacymind_think(SequentialThinkArgs {
    content: "Should we use PostgreSQL or SurrealDB for the new service?",
    chain_id: "db-selection-20250907",
    confidence: Some(0.4), // Explicitly uncertain
    ..Default::default()
}).await?;
// Returns: pattern=Planning, suggests exploring both options

// Branch 1: PostgreSQL
let r2a = legacymind_think(SequentialThinkArgs {
    content: "PostgreSQL path: Mature, battle-tested, great for relational data",
    session_id: Some(r1.session_id),
    branch_from: Some(r1.thought_id),
    thought_mode: Some(ThoughtMode::Branch),
    ..Default::default()
}).await?;

// Branch 2: SurrealDB
let r2b = legacymind_think(SequentialThinkArgs {
    content: "SurrealDB path: Graph capabilities, embedded KG, better for our use case",
    session_id: Some(r1.session_id),
    branch_from: Some(r1.thought_id),
    thought_mode: Some(ThoughtMode::Branch),
    ..Default::default()
}).await?;

// Converge with higher confidence
let r3 = legacymind_think(SequentialThinkArgs {
    content: "After exploring both: SurrealDB fits our graph-heavy needs better",
    session_id: Some(r1.session_id),
    confidence: Some(0.85),
    thought_mode: Some(ThoughtMode::Conclude),
    ..Default::default()
}).await?;
```

## Next Steps

1. ✅ Design review and refinement (this document)
2. ⏳ Implement Phase 1 core infrastructure
3. ⏳ Create comprehensive test suite
4. ⏳ Build new tools with full features
5. ⏳ Migration guide and tooling
6. ⏳ Gradual rollout with monitoring

---

*"Two Opus minds, one optimal solution - the convergence validates the approach"*
