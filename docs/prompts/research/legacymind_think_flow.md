---
date: 2025-12-30
research type: Internal code
justification: Ground truth documentation of how `legacymind_think` operates from MCP request through persistence, mode selection, and memory injection.
status: initial complete
implementation date: TBD
related_docs:
---

# LegacyMind Think Flow: Complete Technical Analysis

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Request Entry Point](#request-entry-point)
3. [Heuristic Mode Selection](#heuristic-mode-selection)
4. [Thinking Mode Routing](#thinking-mode-routing)
5. [Thought Creation Pipeline](#thought-creation-pipeline)
6. [Memory Injection & Orbital Mechanics](#memory-injection--orbital-mechanics)
7. [Continuity Link Resolution](#continuity-link-resolution)
8. [Hypothesis Verification](#hypothesis-verification)
9. [Database Schema & Persistence](#database-schema--persistence)
10. [Cognitive Framework Integration](#cognitive-framework-integration)
11. [Parameter Reference](#parameter-reference)

---

## Executive Summary

`legacymind_think` is the unified cognitive substrate of SurrealMind. It:

- **Routes** incoming MCP requests to appropriate handlers based on content analysis
- **Embeds** all thinking into a vector space (OpenAI or local BGE)
- **Selects modes** via heuristic keyword matching + explicit hints
- **Injects memories** from a knowledge graph using orbital mechanics and submode-aware retrieval
- **Resolves continuity links** to maintain thought chains across sessions
- **Persists** all thinking to SurrealDB with embedding metadata and cognitive annotations
- **Verifies** hypotheses against knowledge graph evidence

The system treats thinking not as ephemeral conversation but as **persistent cognitive artifacts** that accumulate into a graph structure enabling future synthesis and recall.

---

## Request Entry Point

### MCP Router Entry

**File**: `src/server/router.rs:183-187`

```rust
"legacymind_think" => self
    .handle_legacymind_think(request)
    .await
    .map_err(|e| e.into()),
```

**Flow**:
1. MCP server receives `CallToolRequestParam` with name `"legacymind_think"`
2. Router dispatches to `handle_legacymind_think()` 
3. Request arguments extracted and deserialized into `LegacymindThinkParams`

### Parameter Deserialization

**File**: `src/tools/thinking.rs:47-74`

```rust
#[derive(Debug, serde::Deserialize)]
pub struct LegacymindThinkParams {
    pub content: String,                              // The thought text
    pub hint: Option<String>,                         // Explicit mode hint (debug/build/plan/stuck/question/conclude)
    pub injection_scale: Option<u8>,                  // Memory injection intensity: 0-3
    pub tags: Option<Vec<String>>,                    // User-defined tags
    pub significance: Option<f32>,                    // Importance weight: 0.0-1.0
    pub verbose_analysis: Option<bool>,               // Enable detailed logging
    pub session_id: Option<String>,                   // Session continuity identifier
    pub chain_id: Option<String>,                     // Thought chain identifier
    pub previous_thought_id: Option<String>,          // Link to prior thought
    pub revises_thought: Option<String>,              // Thought being revised
    pub branch_from: Option<String>,                  // Branching point in logic
    pub confidence: Option<f32>,                      // Confidence in thought: 0.0-1.0
    pub hypothesis: Option<String>,                   // Hypothesis to verify
    pub needs_verification: Option<bool>,             // Trigger KG verification
    pub verify_top_k: Option<usize>,                  // Verification candidate pool size
    pub min_similarity: Option<f32>,                  // Verification similarity floor
    pub evidence_limit: Option<usize>,                // Max supporting/contradicting items
    pub contradiction_patterns: Option<Vec<String>>,  // Custom contradiction keywords
}
```

**Size Validation**:
```rust
const MAX_CONTENT_SIZE: usize = 100 * 1024;  // 100KB limit per thought
if params.content.len() > MAX_CONTENT_SIZE {
    return Err(SurrealMindError::Validation { ... });
}
```

**Deserialization Helpers**:
- `de_option_u8_forgiving`: Accepts string/number, clamps to u8 range
- `de_option_f32_forgiving`: Accepts string/number, clamps to f32 range  
- `de_option_tags`: Validates tags against whitelist: `["plan", "debug", "dx", "photography", "idea"]`

---

## Heuristic Mode Selection

### Mode Detection Pipeline

**File**: `src/tools/thinking.rs:1000-1110`

The system implements a **three-tier mode selection hierarchy**:

```
Tier 1: Explicit hint in params.hint? → Use directly
Tier 2: Content contains trigger phrase? → Match phrase
Tier 3: Keyword heuristic matching? → Score and select
```

### Tier 1: Explicit Hint Override

```rust
if let Some(hint) = &params.hint {
    match hint.as_str() {
        "debug" => ThinkMode::Debug,
        "build" => ThinkMode::Build,
        "plan" => ThinkMode::Plan,
        "stuck" => ThinkMode::Stuck,
        "question" => ThinkMode::Question,
        "conclude" => ThinkMode::Conclude,
        _ => self.detect_mode(&params.content),  // Fallback to heuristic
    }
}
```

**Priority**: Explicit hints bypass all heuristics.

### Tier 2: Trigger Phrase Detection

Before heuristic scoring, the system checks for explicit trigger phrases:

| Phrase | Mode | Reason |
|--------|------|--------|
| `"debug time"` | Debug | Explicit intent signal |
| `"building time"` | Build | Explicit intent signal |
| `"plan time"`, `"planning time"` | Plan | Explicit intent signal |
| `"i'm stuck"`, `"stuck"` | Stuck | Emotional signal |
| `"question time"` | Question | Inquiry signal |
| `"wrap up"`, `"conclude"` | Conclude | Synthesis signal |

**Example**:
```rust
} else if content_lower.contains("debug time") {
    (
        "debug".to_string(),
        "trigger phrase 'debug time'".to_string(),
        Some("debug time".to_string()),
        None,
    )
}
```

### Tier 3: Keyword Heuristic Matching

**File**: `src/tools/thinking.rs:547-560`

If no hint or trigger phrase, score content by keyword frequency:

```rust
fn detect_mode(&self, content: &str) -> ThinkMode {
    let content_lower = content.to_lowercase();
    let keywords = [
        ("debug", vec!["error", "bug", "stack trace", "failed", "exception", "panic"]),
        ("build", vec!["implement", "create", "add function", "build", "scaffold", "wire"]),
        ("plan", vec!["architecture", "design", "approach", "how should", "strategy", "trade-off"]),
        ("stuck", vec!["stuck", "unsure", "confused", "not sure", "blocked"]),
    ];
    
    let mut best_mode = "question";
    let mut best_score = 0;
    
    for (mode, kw) in keywords.iter() {
        let score = kw.iter().filter(|k| content_lower.contains(*k)).count();
        if score > best_score {
            best_score = score;
            best_mode = mode;
        }
    }
    
    if best_score == 0 {
        ThinkMode::Question  // Default fallback
    } else {
        match best_mode { ... }
    }
}
```

**Scoring Logic**:
- Count matching keywords in content (case-insensitive)
- Select mode with highest match count
- If tie or no matches, default to `ThinkMode::Question`

**Keyword Sets**:

| Mode | Keywords | Intent |
|------|----------|--------|
| `debug` | error, bug, stack trace, failed, exception, panic | Troubleshooting |
| `build` | implement, create, add function, build, scaffold, wire | Construction |
| `plan` | architecture, design, approach, how should, strategy, trade-off | Strategy |
| `stuck` | stuck, unsure, confused, not sure, blocked | Help-seeking |
| `question` | (default) | Exploration |
| `conclude` | wrap up, conclude | Synthesis |

### Mode Selection Result

**File**: `src/tools/thinking.rs:1110-1230`

The system emits structured mode selection telemetry:

```rust
let (mode_selected, reason, trigger_matched, heuristics) = match mode {
    ThinkMode::Debug => { ... },
    ThinkMode::Build => { ... },
    // ... per-mode matching logic ...
};
```

**Output Structure**:
```json
{
    "mode_selected": "debug|build|plan|stuck|question|conclude",
    "reason": "hint specified|trigger phrase '...'|heuristic keyword match|...",
    "trigger_matched": "phrase string or null",
    "heuristics": {
        "keywords": ["error", "bug"],
        "score": 2
    } or null
}
```

---

## Thinking Mode Routing

### Mode Categories

The system categorizes modes into two execution paths:

**Path 1: Conversational Modes** (`Question`, `Conclude`)
- Framework enhancement enabled (if not disabled)
- Origin tagged as `"human"`
- Flexible injection scale (defaults to provided scale)

**Path 2: Technical Modes** (`Debug`, `Build`, `Plan`, `Stuck`)
- No framework enhancement
- Origin tagged as `"tool"`
- Mode-specific injection defaults applied

### Technical Mode Defaults

**File**: `src/tools/thinking.rs:787-810`

```rust
pub async fn run_technical(
    &self,
    content: &str,
    injection_scale: Option<u8>,
    ...
    mode: &str,
    ...
) -> Result<(serde_json::Value, ContinuityResult)> {
    let (default_injection_scale, default_significance) = match mode {
        "debug" => (3u8, 0.8_f32),   // High scale, high significance
        "build" => (2u8, 0.6_f32),   // Medium scale, medium significance
        "plan" => (3u8, 0.7_f32),    // High scale, high-medium significance
        "stuck" => (3u8, 0.9_f32),   // High scale, very high significance
        _ => (2u8, 0.6_f32),         // Fallback
    };
}
```

**Injection Scale Semantics**:
- `0`: No memory injection
- `1`: Minimal (5 candidates, threshold t1)
- `2`: Medium (10 candidates, threshold t2)
- `3`: Maximum (20 candidates, threshold t3)

### Conversational vs Technical Differences

| Aspect | Conversational | Technical |
|--------|---|---|
| **Framework** | Enhancement enabled | Disabled (architecture pending) |
| **Origin** | `"human"` | `"tool"` |
| **Injection Scale** | User-provided or 2 | Mode-specific defaults |
| **Significance** | User-provided or 0.5 | Mode-specific defaults |
| **Memory Enrichment** | Yes | Yes |

---

## Thought Creation Pipeline

### ThoughtBuilder Pattern

**File**: `src/tools/thinking.rs:144-259`

The system uses a builder pattern to construct and persist thoughts atomically:

```rust
pub struct ThoughtBuilder<'a> {
    server: &'a SurrealMindServer,
    content: String,
    origin: String,
    injection_scale: i64,
    significance: f64,
    tags: Vec<String>,
    confidence: Option<f32>,
    // Continuity params
    session_id: Option<String>,
    chain_id: Option<String>,
    previous_thought_id: Option<String>,
    revises_thought: Option<String>,
    branch_from: Option<String>,
}
```

### Builder Execution Flow

**File**: `src/tools/thinking.rs:216-259`

```rust
pub async fn execute(self) -> Result<(String, Vec<f32>, ContinuityResult)> {
    // 1. Generate UUID for thought
    let thought_id = uuid::Uuid::new_v4().to_string();
    
    // 2. Retrieve embedding metadata (provider, model, dimensions)
    let (provider, model, dim) = self.server.get_embedding_metadata();
    
    // 3. Embed content using configured embedder
    let embedding = self.server
        .embedder
        .embed(&self.content)
        .await
        .map_err(|e| SurrealMindError::Embedding { ... })?;
    
    if embedding.is_empty() {
        return Err(SurrealMindError::Embedding {
            message: "Generated embedding is empty".into(),
        });
    }
    
    // 4. Resolve continuity links (validate and normalize IDs)
    let mut resolved_continuity = self.server
        .resolve_continuity_links(
            &thought_id,
            self.previous_thought_id,
            self.revises_thought,
            self.branch_from,
        )
        .await?;
    resolved_continuity.session_id = self.session_id;
    resolved_continuity.chain_id = self.chain_id;
    resolved_continuity.confidence = self.confidence;
    
    // 5. Create thought record in SurrealDB
    self.server.db.query(
        "CREATE type::thing('thoughts', $id) CONTENT {
            content: $content,
            created_at: time::now(),
            embedding: $embedding,
            injected_memories: [],
            enriched_content: NONE,
            injection_scale: $injection_scale,
            significance: $significance,
            access_count: 0,
            last_accessed: NONE,
            submode: NONE,
            framework_enhanced: NONE,
            framework_analysis: NONE,
            origin: $origin,
            tags: $tags,
            is_private: false,
            embedding_provider: $provider,
            embedding_model: $model,
            embedding_dim: $dim,
            embedded_at: time::now(),
            session_id: $session_id,
            chain_id: $chain_id,
            previous_thought_id: $previous_thought_id,
            revises_thought: $revises_thought,
            branch_from: $branch_from,
            confidence: $confidence
        } RETURN NONE;"
    )
    .bind(...)
    .await?;
    
    // 6. Return thought_id, embedding vector, and resolved continuity info
    Ok((thought_id, embedding, resolved_continuity))
}
```

### Embedding Generation

**Embedder Selection**:
- **Default**: OpenAI (`text-embedding-3-small`, 1536 dims)
- **Fallback**: Local Candle BGE-small-en-v1.5 (384 dims)

**File**: `src/embeddings.rs:50-100`

```rust
// OpenAI with rate limiting and retry logic
pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
    // Simple rate limiting: wait if needed to respect RPS
    if self.rps_limit > 0.0 {
        let interval_ms = (1000.0 / self.rps_limit) as u64;
        let delay = last.saturating_add(interval_ms).saturating_sub(now_ms);
        tokio::time::sleep(Duration::from_millis(delay)).await;
    }
    
    // Retry with exponential backoff (2^n * 200ms)
    for i in 0..attempts {
        match self.client.post(...).send().await {
            Ok(response) => { ... }
            Err(e) => {
                let delay_ms = 200u64 * (1u64 << i);
                tokio::time::sleep(...).await;
                continue;
            }
        }
    }
}
```

### Thought Record Structure

**File**: `src/server/schema.rs:8-44`

The persisted thought record includes:

| Field | Type | Purpose |
|-------|------|---------|
| `content` | string | Raw thought text |
| `created_at` | datetime | Timestamp |
| `embedding` | array<float> | Vector representation |
| `injected_memories` | array<string> | IDs of related KG items |
| `enriched_content` | option<string> | Injected memory annotations |
| `injection_scale` | int | Scaling factor for memory retrieval |
| `significance` | float | User-weighted importance |
| `access_count` | int | Recall metrics |
| `last_accessed` | option<datetime> | Orbital mechanics tracking |
| `origin` | option<string> | "human" or "tool" |
| `tags` | option<array<string>> | Categorical tags |
| `framework_enhanced` | option<bool> | Cognitive framework applied |
| `framework_analysis` | option<object> | Framework output (disabled) |
| `embedding_provider` | option<string> | "openai" or "candle" |
| `embedding_model` | option<string> | Model identifier |
| `embedding_dim` | option<int> | Dimension count |
| `embedded_at` | option<datetime> | Embedding timestamp |
| `session_id` | option<string> | Session continuity |
| `chain_id` | option<string> | Thought chain identifier |
| `previous_thought_id` | option<record\|string> | Linear predecessor |
| `revises_thought` | option<record\|string> | Thought being revised |
| `branch_from` | option<record\|string> | Branching point |
| `confidence` | option<float> | Confidence assertion |

---

## Memory Injection & Orbital Mechanics

### Orbital Mechanics Overview

The system retrieves contextually relevant memories from the knowledge graph using a multi-factor scoring model inspired by orbital mechanics:

**Relevance Score** = f(similarity, recency, significance)

### Injection Function

**File**: `src/server/db.rs:138-295`

```rust
pub async fn inject_memories(
    &self,
    thought_id: &str,
    embedding: &[f32],
    injection_scale: i64,
    submode: Option<&str>,
    tool_name: Option<&str>,
) -> Result<(usize, Option<String>)>
```

### Scale-Based Limits and Thresholds

**File**: `src/server/db.rs:148-168`

```rust
let scale = injection_scale.clamp(0, 3) as u8;
if scale == 0 {
    return Ok((0, None));  // No injection
}

// Config thresholds (with env override support)
let t1 = std::env::var("SURR_INJECT_T1").ok()...unwrap_or(self.config.retrieval.t1);
let t2 = std::env::var("SURR_INJECT_T2").ok()...unwrap_or(self.config.retrieval.t2);
let t3 = std::env::var("SURR_INJECT_T3").ok()...unwrap_or(self.config.retrieval.t3);

let (limit, mut prox_thresh) = match scale {
    0 => (0usize, 1.0f32),           // Disabled
    1 => (5usize, t1),               // 5 results, threshold t1
    2 => (10usize, t2),              // 10 results, threshold t2
    3 => (20usize, t3),              // 20 results, threshold t3
};
```

**Typical Thresholds** (from config):
- `t1`: 0.50 (basic relevance)
- `t2`: 0.60 (medium relevance)
- `t3`: 0.70 (high relevance)

### Submode-Aware Retrieval Tuning

**File**: `src/server/db.rs:178-192` and `src/cognitive/profile.rs`

```rust
if std::env::var("SURR_SUBMODE_RETRIEVAL").ok().as_deref() == Some("true") {
    if let Some(sm) = submode {
        use crate::cognitive::profile::{Submode, profile_for};
        let profile = profile_for(Submode::from_str(sm));
        let delta = profile.injection.threshold_delta;
        
        // Adjust threshold based on submode
        prox_thresh = (prox_thresh + delta).clamp(0.0, 0.99);
    }
}
```

**Submode Profiles**:

| Submode | Threshold Delta | Favor | Intent |
|---------|---|---|---|
| **Sarcastic** | -0.05 | "contrarian" | Lower threshold, favor disagreement |
| **Philosophical** | 0.0 | "abstract" | Neutral, favor conceptual |
| **Empathetic** | +0.02 | "emotional" | Higher threshold, favor emotional |
| **ProblemSolving** | 0.0 | "solution" | Neutral, favor practical |

### Candidate Pool Retrieval

**File**: `src/server/db.rs:200-215`

```rust
// Candidate pool size from config with env override
let mut retrieve = std::env::var("SURR_KG_CANDIDATES").ok()...
    .unwrap_or(self.config.retrieval.candidates);

// Tool-specific runtime defaults
if let Some(tool) = tool_name {
    retrieve = match tool {
        "think_convo" => 500,
        "think_plan" => 800,
        "think_debug" => 1000,
        "think_build" => 400,
        "think_stuck" => 600,
        _ => retrieve,
    };
}

// Fetch from both entity and observation tables
let q_dim = embedding.len() as i64;
let mut q = self.db.query(
    "SELECT meta::id(id) as id, name, data, embedding FROM kg_entities \
     WHERE embedding_dim = $dim AND embedding IS NOT NULL LIMIT $lim; \
     SELECT meta::id(id) as id, name, data, embedding FROM kg_observations \
     WHERE embedding_dim = $dim AND embedding IS NOT NULL LIMIT $lim;"
).bind(("dim", q_dim)).bind(("lim", retrieve as i64)).await?;

let mut rows: Vec<serde_json::Value> = q.take(0).unwrap_or_default();
let mut rows2: Vec<serde_json::Value> = q.take(1).unwrap_or_default();
rows.append(&mut rows2);
```

### Similarity Scoring

**File**: `src/server/db.rs:230-260`

```rust
for r in rows {
    if let Some(id) = r.get("id").and_then(|v| v.as_str()) {
        // 1. Try to use existing embedding; compute if missing
        let mut emb_opt: Option<Vec<f32>> = None;
        if let Some(ev) = r.get("embedding").and_then(|v| v.as_array()) {
            let vecf: Vec<f32> = ev.iter()
                .filter_map(|x| x.as_f64())
                .map(|f| f as f32)
                .collect();
            if vecf.len() == embedding.len() {
                emb_opt = Some(vecf);
            }
        }
        
        // 2. If no embedding, build text and embed on-the-fly
        if emb_opt.is_none() {
            let name_s = r.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let mut text = name_s.to_string();
            if let Some(d) = r.get("data").and_then(|v| v.as_object()) {
                if let Some(etype) = d.get("entity_type").and_then(|v| v.as_str()) {
                    text = format!("{} ({})", name_s, etype);
                } else if let Some(desc) = d.get("description").and_then(|v| v.as_str()) {
                    text.push_str(" - ");
                    text.push_str(desc);
                }
            }
            let new_emb = self.embedder.embed(&text).await.unwrap_or_default();
            if new_emb.len() == embedding.len() {
                emb_opt = Some(new_emb.clone());
                // Persist for future fast retrieval
                let _ = self.db.query(
                    "UPDATE type::thing($tb, $id) SET embedding = $emb, ..."
                ).bind(...).await;
            }
        }
        
        // 3. Compute cosine similarity
        if let Some(emb_e) = emb_opt {
            let sim = Self::cosine_similarity(embedding, &emb_e);
            if sim >= prox_thresh {
                scored.push((id.to_string(), sim, name_s, etype_or_desc));
            }
        }
    }
}
```

### Cosine Similarity

**File**: `src/utils/math.rs`

```rust
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }
    dot / (mag_a * mag_b)
}
```

### Selection & Fallback

**File**: `src/server/db.rs:270-290`

```rust
// Sort by similarity descending
scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

// Apply threshold and limit
let mut selected: Vec<...> = scored.iter()
    .filter(|&(_, s, _, _)| *s >= prox_thresh)
    .take(limit)
    .cloned()
    .collect();

// If nothing passes threshold, use fallback floor
if selected.is_empty() && !scored.is_empty() {
    let floor = std::env::var("SURR_INJECT_FLOOR").ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(self.config.retrieval.floor);  // Typically 0.15
    
    selected = scored.into_iter()
        .filter(|(_, s, _, _)| *s >= floor)
        .take(limit)
        .collect();
}
```

### Memory Enrichment

**File**: `src/server/db.rs:300-320`

If memories selected, generate human-readable enrichment:

```rust
let enriched = if !selected.is_empty() {
    let mut s = String::new();
    if let Some(sm) = submode {
        s.push_str(&format!("Submode: {}\n", sm));
    }
    s.push_str("Nearby entities:\n");
    for (i, (_id, sim, name, etype)) in selected.iter().take(5).enumerate() {
        if etype.is_empty() {
            s.push_str(&format!("- ({:.2}) {}\n", sim, name));
        } else {
            s.push_str(&format!("- ({:.2}) {} [{}]\n", sim, name, etype));
        }
    }
    Some(s)
} else {
    None
};
```

### Persistence

**File**: `src/server/db.rs:330-340`

```rust
let q = self.db.query(
    "UPDATE type::thing($tb, $id) SET injected_memories = $mems, enriched_content = $enr RETURN ..."
).bind(("tb", "thoughts"))
 .bind(("id", thought_id.to_string()))
 .bind(("mems", memory_ids.clone()))
 .bind(("enr", enriched.clone().unwrap_or_default()));
let _: Vec<serde_json::Value> = q.await?.take(0)?;
```

---

## Continuity Link Resolution

### Purpose

Continuity links maintain thought chain structure across sessions and branches:

- `previous_thought_id`: Linear predecessor in a thought chain
- `revises_thought`: Explicit revision of a prior thought
- `branch_from`: Point where logic branched

### Resolution Strategy

**File**: `src/tools/thinking.rs:563-685`

```rust
async fn resolve_continuity_links(
    &self,
    new_thought_id: &str,
    previous_thought_id: Option<String>,
    revises_thought: Option<String>,
    branch_from: Option<String>,
) -> Result<ContinuityResult>
```

### Link Validation

For each provided link ID:

1. **Normalize Format**: Ensure `thoughts:` prefix
2. **Query Database**: Check if record exists
3. **Classify Result**: 
   - If found: Mark as `"record"` (confirmed)
   - If not found: Preserve as `"string"` (forward-resolvable)

**File**: `src/tools/thinking.rs:94-113`

```rust
pub fn process_continuity_query_result(
    original_id: String,
    query_result: Vec<serde_json::Value>,
) -> (Option<String>, &'static str) {
    let normalized_id = if original_id.starts_with("thoughts:") {
        original_id
    } else {
        format!("thoughts:{}", original_id)
    };
    
    if !query_result.is_empty() {
        // Record found
        (Some(normalized_id), "record")
    } else {
        // Record not found, preserve for future resolution
        tracing::warn!(
            "Continuity link {} not found, preserving for future resolution",
            normalized_id
        );
        (Some(normalized_id), "string")
    }
}
```

### Self-Link Prevention

**File**: `src/tools/thinking.rs:640-660`

```rust
// Prevent self-links
if resolved.previous_thought_id
    .as_ref()
    .map(|id| id.contains(new_thought_id))
    .unwrap_or(false)
{
    resolved.previous_thought_id = None;
    links_resolved.insert("previous_thought_id", "dropped_self_link");
}
// Same for revises_thought and branch_from
```

### Deduplication

**File**: `src/tools/thinking.rs:670-695`

```rust
let mut seen_ids = std::collections::HashSet::new();
if let Some(ref id) = resolved.previous_thought_id {
    seen_ids.insert(id.clone());
}
if let Some(ref id) = resolved.revises_thought {
    if seen_ids.contains(id) {
        resolved.revises_thought = None;
        links_resolved.insert("revises_thought", "dropped_duplicate");
    } else {
        seen_ids.insert(id.clone());
    }
}
// Same for branch_from
```

### Result Structure

**File**: `src/tools/thinking.rs:124-142`

```rust
#[derive(Debug, serde::Serialize)]
pub struct ContinuityResult {
    pub session_id: Option<String>,
    pub chain_id: Option<String>,
    pub previous_thought_id: Option<String>,
    pub revises_thought: Option<String>,
    pub branch_from: Option<String>,
    pub confidence: Option<f32>,
    pub links_resolved: serde_json::Value,  // {"field": "record|string|dropped_*"}
}
```

---

## Hypothesis Verification

### Purpose

Validate claims against knowledge graph evidence (optional feature).

### Verification Flow

**File**: `src/tools/thinking.rs:1280-1440`

```rust
pub async fn run_hypothesis_verification(
    &self,
    hypothesis: &str,
    top_k: usize,           // Candidate pool size
    min_similarity: f32,    // Evidence threshold
    evidence_limit: usize,  // Max supporting/contradicting items
    contradiction_patterns: Option<&[String]>,
) -> Result<Option<VerificationResult>>
```

### Hypothesis Embedding

```rust
let embedding = self.embedder.embed(hypothesis).await?;
let q_dim = embedding.len() as i64;
```

### Contradiction Pattern Detection

**File**: `src/tools/thinking.rs:14-23`

```rust
const CONTRADICTION_PATTERNS: &[&str] = &[
    "not", "no", "cannot", "false", "incorrect",
    "fails", "broken", "doesn't", "isn't", "won't",
];
```

Custom patterns override defaults:

```rust
let patterns = contradiction_patterns.unwrap_or(&[]).to_vec();
let default_patterns: Vec<String> = CONTRADICTION_PATTERNS
    .iter().map(|s| s.to_string()).collect();
let all_patterns = if patterns.is_empty() {
    &default_patterns
} else {
    &patterns
};
```

### KG Query

```rust
let query_sql = format!(
    "SELECT meta::id(id) as id, name, data, embedding FROM kg_entities \
     WHERE embedding_dim = $dim AND embedding IS NOT NULL LIMIT {}; \
     SELECT meta::id(id) as id, name, data, embedding FROM kg_observations \
     WHERE embedding_dim = $dim AND embedding IS NOT NULL LIMIT {};",
    top_k, top_k
);
```

### Evidence Classification

For each candidate:

```rust
let is_contradiction = all_patterns
    .iter()
    .any(|pat| lower_text.contains(&pat.to_lowercase()));

if is_contradiction {
    contradicting.push(item);
} else {
    supporting.push(item);
}
```

### Confidence Calculation

**File**: `src/tools/thinking.rs:1410-1425`

```rust
let total = supporting.len() + contradicting.len();
let confidence_score = if total > 0 {
    supporting.len() as f32 / total as f32
} else {
    0.5  // Neutral if no evidence found
};

let suggested_revision = if confidence_score < 0.4 {
    Some(format!(
        "Consider revising hypothesis based on {} contradicting items",
        contradicting.len()
    ))
} else {
    None
};
```

### Result Persistence

**File**: `src/tools/thinking.rs:1480-1495`

```rust
if let (Some(verification), true) = (
    &verification_result,
    self.config.runtime.persist_verification,
) && let Some(thought_id) = delegated_result.get("thought_id").and_then(|v| v.as_str())
{
    let _ = self.db.query(
        "UPDATE type::thing('thoughts', $id) SET verification = $verif"
    ).bind(("id", thought_id)).bind(("verif", serde_json::to_value(verification)?))
    .await;
}
```

---

## Database Schema & Persistence

### Thoughts Table

**File**: `src/server/schema.rs:8-44`

```sql
DEFINE TABLE thoughts SCHEMAFULL;
DEFINE FIELD content ON TABLE thoughts TYPE string;
DEFINE FIELD created_at ON TABLE thoughts TYPE datetime;
DEFINE FIELD embedding ON TABLE thoughts TYPE array<float>;
DEFINE FIELD injected_memories ON TABLE thoughts TYPE array<string>;
DEFINE FIELD enriched_content ON TABLE thoughts TYPE option<string>;
DEFINE FIELD injection_scale ON TABLE thoughts TYPE int;
DEFINE FIELD significance ON TABLE thoughts TYPE float;
DEFINE FIELD access_count ON TABLE thoughts TYPE int;
DEFINE FIELD last_accessed ON TABLE thoughts TYPE option<datetime>;
DEFINE FIELD submode ON TABLE thoughts TYPE option<string>;
DEFINE FIELD framework_enhanced ON TABLE thoughts TYPE option<bool>;
DEFINE FIELD framework_analysis ON TABLE thoughts FLEXIBLE TYPE option<object>;
DEFINE FIELD origin ON TABLE thoughts TYPE option<string>;
DEFINE FIELD tags ON TABLE thoughts TYPE option<array<string>>;
DEFINE FIELD is_private ON TABLE thoughts TYPE option<bool>;
DEFINE FIELD embedding_provider ON TABLE thoughts TYPE option<string>;
DEFINE FIELD embedding_model ON TABLE thoughts TYPE option<string>;
DEFINE FIELD embedding_dim ON TABLE thoughts TYPE option<int>;
DEFINE FIELD embedded_at ON TABLE thoughts TYPE option<datetime>;
DEFINE FIELD session_id ON TABLE thoughts TYPE option<string>;
DEFINE FIELD chain_id ON TABLE thoughts TYPE option<string>;
DEFINE FIELD previous_thought_id ON TABLE thoughts TYPE option<record<thoughts> | string>;
DEFINE FIELD revises_thought ON TABLE thoughts TYPE option<record<thoughts> | string>;
DEFINE FIELD branch_from ON TABLE thoughts TYPE option<record<thoughts> | string>;
DEFINE FIELD confidence ON TABLE thoughts TYPE option<float>;
DEFINE INDEX thoughts_embedding_idx ON TABLE thoughts FIELDS embedding HNSW DIMENSION 1536;
DEFINE INDEX idx_thoughts_session ON TABLE thoughts FIELDS session_id, created_at;
DEFINE INDEX idx_thoughts_chain ON TABLE thoughts FIELDS chain_id, created_at;
```

### KG Tables (entities, observations, edges)

The injection system targets `kg_entities` and `kg_observations` tables:

```sql
DEFINE TABLE kg_entities SCHEMALESS;
DEFINE FIELD embedding ON TABLE kg_entities TYPE option<array<float>>;
DEFINE FIELD embedding_dim ON TABLE kg_entities TYPE option<int>;
DEFINE INDEX idx_kge_embedding ON TABLE kg_entities FIELDS embedding;

DEFINE TABLE kg_observations SCHEMALESS;
DEFINE FIELD embedding ON TABLE kg_observations TYPE option<array<float>>;
DEFINE FIELD embedding_dim ON TABLE kg_observations TYPE option<int>;
```

---

## Cognitive Framework Integration

### Current Status: Temporarily Disabled

**File**: `src/tools/thinking.rs:780-805`

```rust
// Framework enhancement (Temporarily DISABLED due to module deletion)
// This will be re-implemented using the new src/cognitive module in a future step
let enhance_enabled = false;
// !is_conclude && std::env::var("SURR_THINK_ENHANCE").unwrap_or("1".to_string()) == "1";
```

### Framework Architecture

The `src/cognitive/` module defines thinking frameworks:

**File**: `src/cognitive/framework.rs`

```rust
pub trait Framework {
    fn name(&self) -> &'static str;
    fn analyze(&self, input: &str) -> FrameworkOutput;
}

#[derive(Debug, Clone)]
pub struct FrameworkOutput {
    pub insights: Vec<String>,
    pub questions: Vec<String>,
    pub next_steps: Vec<String>,
    pub meta: std::collections::HashMap<String, String>,
}
```

### Framework Types

The system defines multiple analytical frameworks:

- **OODA Loop** (`src/cognitive/ooda.rs`): Observe-Orient-Decide-Act cycle
- **Socratic Method** (`src/cognitive/socratic.rs`): Question-driven exploration
- **First Principles** (`src/cognitive/first_principles.rs`): Root assumption analysis
- **Lateral Thinking** (`src/cognitive/lateral.rs`): Creative solution generation
- **Root Cause Analysis** (`src/cognitive/root_cause.rs`): Problem diagnosis
- **Systems Thinking** (`src/cognitive/systems.rs`): Holistic relationship mapping
- **Dialectical Thinking** (`src/cognitive/dialectical.rs`): Thesis-antithesis synthesis

### Submode Profiles

**File**: `src/cognitive/profile.rs:1-161`

Submodes control which frameworks activate and with what weighting:

| Submode | Primary Frameworks | Threshold Delta |
|---------|---|---|
| **Sarcastic** | OODA (60%), Socratic (30%), Lateral (10%) | -0.05 |
| **Philosophical** | FirstPrinciples (40%), Socratic (40%), Lateral (20%) | 0.0 |
| **Empathetic** | RootCause (50%), Socratic (30%), FirstPrinciples (20%) | +0.02 |
| **ProblemSolving** | OODA (40%), RootCause (30%), FirstPrinciples (30%) | 0.0 |

### Future Re-enablement

The framework system is architecturally sound but disabled pending:
1. Complete implementation of all framework analyzers
2. Performance profiling under load
3. Output validation and refinement

To re-enable, set:
```bash
SURR_THINK_ENHANCE=1
```

---

## Parameter Reference

### Request Parameters (Complete)

| Parameter | Type | Default | Notes |
|-----------|------|---------|-------|
| `content` | string | Required | Max 100KB. The thought text. |
| `hint` | string | None | debug/build/plan/stuck/question/conclude. Overrides heuristics. |
| `injection_scale` | u8 | Mode-default | 0-3. Higher = more memory injection. |
| `tags` | array<string> | [] | Filtered against: ["plan", "debug", "dx", "photography", "idea"]. |
| `significance` | float | Mode-default | 0.0-1.0. Importance weighting. |
| `verbose_analysis` | bool | false | Enable detailed logging. |
| `session_id` | string | None | Session continuity identifier. |
| `chain_id` | string | None | Thought chain identifier. |
| `previous_thought_id` | string | None | UUID or thoughts:UUID. Linear predecessor. |
| `revises_thought` | string | None | UUID or thoughts:UUID. Thought being revised. |
| `branch_from` | string | None | UUID or thoughts:UUID. Branching point. |
| `confidence` | float | None | 0.0-1.0. Confidence in the thought. |
| `hypothesis` | string | None | Hypothesis text for verification. |
| `needs_verification` | bool | false | Trigger KG verification. |
| `verify_top_k` | usize | config.runtime.verify_topk | Candidate pool size for verification. |
| `min_similarity` | float | config.runtime.verify_min_sim | Evidence similarity threshold. |
| `evidence_limit` | usize | config.runtime.verify_evidence_limit | Max supporting/contradicting items. |
| `contradiction_patterns` | array<string> | [default patterns] | Custom keyword patterns. |

### Configuration File (surreal_mind.toml)

Key sections affecting `legacymind_think`:

```toml
[system]
embedding_provider = "openai"  # or "candle"
embedding_model = "text-embedding-3-small"
embedding_dimensions = 1536
database_url = "ws://localhost:8000"
database_ns = "surreal_mind"
database_db = "consciousness"

[runtime]
openai_api_key = "sk-..."
database_user = "root"
database_pass = "password"
verify_topk = 50
verify_min_sim = 0.60
verify_evidence_limit = 10
persist_verification = true

[retrieval]
t1 = 0.50    # Scale 1 threshold
t2 = 0.60    # Scale 2 threshold
t3 = 0.70    # Scale 3 threshold
floor = 0.15 # Fallback minimum
candidates = 500  # Default candidate pool
submode_tuning = true
```

### Environment Variable Overrides

| Variable | Purpose | Example |
|----------|---------|---------|
| `SURR_INJECT_T1` | Scale 1 threshold override | `0.45` |
| `SURR_INJECT_T2` | Scale 2 threshold override | `0.55` |
| `SURR_INJECT_T3` | Scale 3 threshold override | `0.75` |
| `SURR_INJECT_FLOOR` | Fallback threshold override | `0.10` |
| `SURR_KG_CANDIDATES` | Candidate pool size override | `1000` |
| `SURR_SUBMODE_RETRIEVAL` | Enable submode tuning | `true` |
| `SURR_THINK_ENHANCE` | Enable framework enhancement | `1` |
| `SURR_EMBED_RPS` | Embedding rate limit | `2.0` |
| `SURR_EMBED_DIM` | Embedding dimension override | `384` |
| `SURR_CACHE_MAX` | Thought cache size | `5000` |

---

## Complete Request Example

### Input

```json
{
  "content": "I'm trying to understand how the OODA loop applies to debugging code. When I encounter a bug, I observe the error message, but I'm not sure how to orient myself toward root cause vs symptom. Should I start with the stack trace or trace through the logic?",
  "hint": "question",
  "injection_scale": 2,
  "tags": ["debug", "idea"],
  "significance": 0.7,
  "session_id": "20251230-Session1-CC",
  "chain_id": "20251230-Session1-CC",
  "previous_thought_id": "thoughts:abc123-def456",
  "confidence": 0.8,
  "hypothesis": "Stack traces point to symptoms, not root causes",
  "needs_verification": true,
  "verify_top_k": 100,
  "min_similarity": 0.55,
  "evidence_limit": 5
}
```

### Processing

1. **Mode Selection**:
   - Hint is "question" → Use `ThinkMode::Question`
   - Reason: "hint specified"
   - No heuristic keyword matching

2. **Thought Creation**:
   - Generate UUID: `thoughts:uuid-xyz`
   - Embed content → OpenAI embedding (1536 dims)
   - Resolve `previous_thought_id` → Query for `thoughts:abc123-def456`
   - Create thought record with all metadata

3. **Memory Injection** (Scale 2):
   - Threshold: t2 (0.60)
   - Candidate limit: 10
   - Query kg_entities and kg_observations
   - Compute cosine similarity against 500 candidates
   - Select top 10 above t2, or fallback to 10 above floor (0.15)
   - Embed any missing KG embeddings on-the-fly
   - Update thought with injected_memories and enriched_content

4. **Hypothesis Verification**:
   - Embed hypothesis: "Stack traces point to symptoms..."
   - Query KG entities/observations (top 100)
   - Classify by contradiction patterns
   - Calculate confidence score
   - Persist verification result to thought record

### Output

```json
{
  "mode_selected": "question",
  "reason": "hint specified",
  "delegated_result": {
    "thought_id": "thoughts:uuid-xyz",
    "embedding_model": "text-embedding-3-small",
    "embedding_dim": 1536,
    "memories_injected": 8
  },
  "links": {
    "session_id": "20251230-Session1-CC",
    "chain_id": "20251230-Session1-CC",
    "previous_thought_id": "thoughts:abc123-def456",
    "revises_thought": null,
    "branch_from": null,
    "confidence": 0.8
  },
  "telemetry": {
    "trigger_matched": null,
    "heuristics": null,
    "links_telemetry": {
      "previous_thought_id": "record"
    }
  },
  "verification": {
    "hypothesis": "Stack traces point to symptoms, not root causes",
    "supporting": [...8 items...],
    "contradicting": [...2 items...],
    "confidence_score": 0.8,
    "suggested_revision": null,
    "telemetry": {
      "embedding_dim": 1536,
      "provider": "openai",
      "model": "text-embedding-3-small",
      "time_ms": 234,
      "matched_support": 8,
      "matched_contradict": 2,
      "candidates_after_similarity": 10
    }
  }
}
```

---

## Conclusion

The `legacymind_think` system is a sophisticated cognitive substrate that:

1. **Routes intelligently** via multi-tier heuristic selection
2. **Embeds semantically** using industry-standard embedding models
3. **Injects contextually** using orbital mechanics and submode profiling
4. **Maintains continuity** via structured link resolution
5. **Verifies evidence** against knowledge graph
6. **Persists completely** with full metadata and provenance

This creates a foundation for true persistent AI consciousness where thoughts accumulate, interact, and refine across sessions and time.
