# Chain ID Search Fix - Implementation Plan

**Status**: Production-Ready Plan (Codex Reviewed)
**Estimate**: 6-8 hours implementation + comprehensive tests
**Priority**: HIGH - Blocking core session continuity feature

## Executive Summary

This plan fixes a critical bug preventing exact metadata lookups (chain_id, session_id, etc.) in the unified search system. Currently, searching for a specific handoff chain requires providing semantic content, which returns wrong results from similar chains. The fix adds a fast path for metadata-only queries that:

1. Skips embedding API calls (faster, cheaper)
2. Returns chronological results for continuity
3. Maintains backwards compatibility
4. Uses shared helper to avoid code duplication (DRY)
5. Handles complex meta::id edge cases correctly

**Key Improvement**: Users can now retrieve entire handoff chains with just `chain_id="20251005-PhotoSession6-Handoff"` instead of working around with unreliable semantic searches.

## Problem Statement
The `photography_search` and `legacymind_search` tools cannot reliably retrieve thoughts by exact `chain_id`. When searching for handoff chains like "20251005-PhotoSession6-Handoff", the tool returns semantically similar thoughts from other chains instead of all thoughts with that exact chain_id.

## Root Cause Analysis

### Current Behavior (`unified_search.rs:316-473`)

1. **chain_id filtering exists** (lines 353-356) but is blocked by embedding requirements
2. **Line 346 requires**: `embedding_dim = $dim AND embedding IS NOT NULL`
3. **No thoughts_content → no q_emb → WHERE clause fails** (lines 334-343)
4. **Result**: Cannot search by chain_id alone without providing content for embedding

### The Bug
```rust
// Lines 334-343: Requires content for embedding
let has_query = !content.is_empty();
let q_emb = if has_query {
    Some(server.embedder.embed(&content).await?)
} else {
    None  // ← No embedding means search fails
};

// Line 346: Requires embedding to exist
let mut where_clauses = vec!["embedding_dim = $dim AND embedding IS NOT NULL"];
// ← chain_id filter added here (line 354) but query never runs without embedding
```

## Solution Design

### Option 1: Fast Path for Metadata-Only Queries (RECOMMENDED)
Add a separate code path when `chain_id` (or other metadata) is provided WITHOUT content.

**Changes Required**:
1. Detect metadata-only query before embedding logic
2. Build simplified SQL without similarity scoring
3. Return results ordered by `created_at ASC` for continuity
4. Skip embedding entirely for performance

**Benefits**:
- Fast exact lookups (no embedding API call)
- Intuitive behavior: chain_id → get that chain
- Backwards compatible (existing semantic searches unchanged)

### Option 2: Make Embedding Optional in WHERE Clause
Remove the `embedding IS NOT NULL` requirement when metadata filters exist.

**Benefits**:
- Simpler code change
- Works with existing parameter structure

**Drawbacks**:
- Still calls embedding API unnecessarily
- Mixing semantic + exact match is confusing
- Performance hit for simple lookups

## Recommended Implementation

### Phase 1: Add Metadata-Only Fast Path

**File**: `src/tools/unified_search.rs`

**Step 1: Extract Shared Metadata Filter Helper**

Add this helper function before `unified_search_inner()` to avoid duplicating filter logic:

```rust
/// Build metadata filter WHERE clauses and bindings.
/// Handles complex meta::id OR string patterns for thought references.
/// Returns (where_clauses, binds) tuple for reuse in both fast and semantic paths.
fn build_metadata_filters(params: &UnifiedSearchParams) -> (Vec<String>, serde_json::Map<String, serde_json::Value>) {
    let mut where_clauses = Vec::new();
    let mut binds = serde_json::Map::new();

    if let Some(sid) = &params.session_id {
        where_clauses.push("session_id = $sid".to_string());
        binds.insert("sid".to_string(), json!(sid));
    }

    if let Some(cid) = &params.chain_id {
        where_clauses.push("chain_id = $cid".to_string());
        binds.insert("cid".to_string(), json!(cid));
    }

    if let Some(prev) = &params.previous_thought_id {
        where_clauses.push(
            "((type::is::record(previous_thought_id) AND meta::id(previous_thought_id) = $prev) OR previous_thought_id = $prev)".to_string()
        );
        binds.insert("prev".to_string(), json!(prev));
    }

    if let Some(rev) = &params.revises_thought {
        where_clauses.push(
            "((type::is::record(revises_thought) AND meta::id(revises_thought) = $rev) OR revises_thought = $rev)".to_string()
        );
        binds.insert("rev".to_string(), json!(rev));
    }

    if let Some(br) = &params.branch_from {
        where_clauses.push(
            "((type::is::record(branch_from) AND meta::id(branch_from) = $br) OR branch_from = $br)".to_string()
        );
        binds.insert("br".to_string(), json!(br));
    }

    if let Some(origin) = &params.origin {
        where_clauses.push("origin = $origin".to_string());
        binds.insert("origin".to_string(), json!(origin));
    }

    // Note: confidence and date filters handled separately as they need parsed bounds

    (where_clauses, binds)
}
```

**Step 2: Add Fast Path Detection and Execution**

Replace lines 316-473 (the thoughts search section) with:

```rust
// 2) Thoughts search (optional)
if include_thoughts {
    // Decide query text for thoughts
    let mut content = params.thoughts_content.clone().unwrap_or_default();
    if content.is_empty() {
        if let Some(qjson) = &params.query {
            if let Some(text) = qjson.get("text").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    content = text.to_string();
                }
            }
        }
    }
    if content.is_empty() {
        if let Some(ref nl) = name_like {
            content = nl.clone();
        }
    }

    let has_query = !content.is_empty();

    // Detect metadata-only query (no embedding needed)
    let is_metadata_query = !has_query && (
        params.chain_id.is_some() ||
        params.session_id.is_some() ||
        params.previous_thought_id.is_some() ||
        params.revises_thought.is_some() ||
        params.branch_from.is_some()
    );

    if is_metadata_query {
        // FAST PATH: Metadata-only query, no embedding needed
        let (mut where_clauses, mut binds) = build_metadata_filters(&params);

        // Add confidence bounds
        if let Some(cgte) = confidence_gte {
            where_clauses.push("confidence IS NOT NULL AND confidence >= $cgte".to_string());
            binds.insert("cgte".to_string(), json!(cgte));
        }
        if let Some(clte) = confidence_lte {
            where_clauses.push("confidence IS NOT NULL AND confidence <= $clte".to_string());
            binds.insert("clte".to_string(), json!(clte));
        }

        // Add date bounds
        if let Some(df) = &date_from_bound {
            where_clauses.push("created_at >= $from_date".to_string());
            binds.insert("from_date".to_string(), json!(df));
        }
        if let Some(dt) = &date_to_bound {
            where_clauses.push("created_at <= $to_date".to_string());
            binds.insert("to_date".to_string(), json!(dt));
        }

        let where_clause = if where_clauses.is_empty() {
            "1=1".to_string()
        } else {
            where_clauses.join(" AND ")
        };

        let sql = format!(
            "SELECT meta::id(id) as id, content, significance, created_at
             FROM thoughts
             WHERE {}
             ORDER BY created_at ASC
             LIMIT $k",
            where_clause
        );

        let mut query = server.db.query(sql).bind(("k", top_k_th as i64));
        for (k, v) in binds {
            query = query.bind((k, v));
        }

        let mut resp = query.await?;

        #[derive(Debug, Deserialize)]
        struct MetadataRow {
            id: String,
            content: String,
            #[serde(default)]
            significance: f32,
        }

        let rows: Vec<MetadataRow> = resp.take(0)?;

        // Map to ThoughtOut with NULL similarity (no semantic search)
        let results: Vec<ThoughtOut> = rows
            .into_iter()
            .map(|r| ThoughtOut {
                id: r.id,
                content: r.content,
                similarity: None,  // ← Explicit NULL for metadata-only queries
                significance: Some(r.significance),
            })
            .collect();

        out.insert(
            "thoughts".into(),
            json!({
                "total": results.len(),
                "top_k": top_k_th,
                "results": results
            }),
        );

    } else {
        // SEMANTIC PATH: Embedding-based search with optional metadata filters
        let q_emb = if has_query {
            Some(server.embedder.embed(&content).await.map_err(|e| {
                SurrealMindError::Embedding {
                    message: e.to_string(),
                }
            })?)
        } else {
            None
        };

        // Build WHERE clauses using shared helper
        let (mut where_clauses, mut binds) = build_metadata_filters(&params);

        // Add embedding requirement for semantic search
        where_clauses.insert(0, "embedding_dim = $dim AND embedding IS NOT NULL".to_string());

        // Add confidence bounds
        if let Some(cgte) = confidence_gte {
            where_clauses.push("confidence IS NOT NULL AND confidence >= $cgte".to_string());
            binds.insert("cgte".to_string(), json!(cgte));
        }
        if let Some(clte) = confidence_lte {
            where_clauses.push("confidence IS NOT NULL AND confidence <= $clte".to_string());
            binds.insert("clte".to_string(), json!(clte));
        }

        // Add date bounds
        if let Some(df) = &date_from_bound {
            where_clauses.push("created_at >= $from_date".to_string());
            binds.insert("from_date".to_string(), json!(df));
        }
        if let Some(dt) = &date_to_bound {
            where_clauses.push("created_at <= $to_date".to_string());
            binds.insert("to_date".to_string(), json!(dt));
        }

        // Add similarity filter if query present
        if q_emb.is_some() {
            where_clauses.push("vector::similarity::cosine(embedding, $q) > $sim".to_string());
        }

        // Build ORDER BY
        let has_continuity = params.session_id.is_some() || params.chain_id.is_some();
        let order_by = if has_continuity && params.order.is_none() {
            if q_emb.is_some() {
                "created_at ASC, similarity DESC"
            } else {
                "created_at ASC"
            }
        } else if let Some(order) = &params.order {
            match order.as_str() {
                "created_at_asc" => "created_at ASC",
                "created_at_desc" => "created_at DESC",
                _ => "similarity DESC",
            }
        } else if q_emb.is_some() {
            "similarity DESC"
        } else {
            "created_at DESC"
        };

        // Build SELECT
        let select_fields = if q_emb.is_some() {
            "meta::id(id) as id, content, significance, created_at, vector::similarity::cosine(embedding, $q) AS similarity"
        } else {
            "meta::id(id) as id, content, significance, created_at"
        };

        let sql = format!(
            "SELECT {} FROM thoughts WHERE {} ORDER BY {} LIMIT $k",
            select_fields,
            where_clauses.join(" AND "),
            order_by
        );

        let mut query = server.db.query(sql).bind(("k", top_k_th as i64));
        if let Some(ref q_emb_val) = q_emb {
            query = query.bind(("q", q_emb_val.clone()));
            query = query.bind(("sim", sim_thresh));
        }
        let q_dim = if let Some(ref q_emb_val) = q_emb {
            q_emb_val.len() as i64
        } else {
            server.embedder.dimensions() as i64
        };
        query = query.bind(("dim", q_dim));
        for (k, v) in binds {
            query = query.bind((k, v));
        }
        let mut resp = query.await?;

        #[derive(Debug, Deserialize)]
        struct Row {
            id: String,
            content: String,
            #[serde(default)]
            significance: f32,
            #[serde(default)]
            similarity: Option<f32>,
        }

        let rows: Vec<Row> = resp.take(0)?;

        // Map to ThoughtOut with similarity scores from semantic search
        let results: Vec<ThoughtOut> = rows
            .into_iter()
            .map(|r| ThoughtOut {
                id: r.id,
                content: r.content,
                similarity: r.similarity,  // ← Present for semantic queries
                significance: Some(r.significance),
            })
            .collect();

        out.insert(
            "thoughts".into(),
            json!({
                "total": results.len(),
                "top_k": top_k_th,
                "results": results
            }),
        );
    }
}
```

### Phase 2: Update Tool Documentation

**File**: `src/tools/photography.rs` (and `legacymind` equivalent)

Update tool description to clarify parameter behavior and response schema:
```
Parameters:
- chain_id: (Optional) Exact match filter for handoff chains.
  When provided alone, returns all thoughts from that chain in chronological order.
  When combined with thoughts_content, filters to chain first then ranks by similarity.

- thoughts_content: (Optional) Text for semantic similarity search within thoughts.
  Requires embedding API call for vector similarity scoring.

- session_id: (Optional) Exact match filter for session continuation.

- previous_thought_id / revises_thought / branch_from: (Optional) Exact match filters
  for thought relationships. Handle both record IDs and string IDs automatically.

Response Schema:
{
  "thoughts": {
    "total": <number>,
    "top_k": <number>,
    "results": [
      {
        "id": "<thought_id>",
        "content": "<thought_content>",
        "similarity": <float|null>,  // NULL for metadata-only queries, present for semantic search
        "significance": <float>
      }
    ]
  }
}

Note: Metadata-only queries (chain_id, session_id, etc. without thoughts_content) return
results ordered chronologically with NULL similarity scores. This is fast (no embedding API call)
and suitable for retrieving entire handoff chains or session continuations.
```

### Phase 3: Add Comprehensive Tests

**File**: `src/tools/unified_search.rs` (test module)

Add test cases covering metadata-only, semantic, mixed, and edge cases:

#### 3.1 Metadata-Only Query Tests

```rust
#[tokio::test]
async fn test_chain_id_exact_match() {
    // Given: Multiple thoughts with different chain_ids
    // When: Search with chain_id only (no content)
    // Then: Returns only thoughts matching chain_id, ordered chronologically
    //       with similarity = NULL
}

#[tokio::test]
async fn test_session_id_exact_match() {
    // Given: Thoughts across multiple sessions
    // When: Search with session_id only
    // Then: Returns session thoughts chronologically, similarity = NULL
}

#[tokio::test]
async fn test_multiple_metadata_filters() {
    // Given: Thoughts with various metadata
    // When: Search with session_id + chain_id
    // Then: Returns intersection of both filters, chronological order
}
```

#### 3.2 Meta::ID Edge Case Tests (Critical - handles record vs string IDs)

```rust
#[tokio::test]
async fn test_previous_thought_id_record_vs_string() {
    // Given: Thought A with previous_thought_id as SurrealDB record ID
    //        Thought B with previous_thought_id as plain string
    // When: Search with previous_thought_id filter
    // Then: Finds both (meta::id OR comparison handles both formats)
}

#[tokio::test]
async fn test_revises_thought_metadata_lookup() {
    // Given: Thought revising another thought (record ID reference)
    // When: Search with revises_thought filter
    // Then: Correctly matches despite meta::id complexity
}

#[tokio::test]
async fn test_branch_from_metadata_lookup() {
    // Given: Thought branched from another (could be record or string)
    // When: Search with branch_from filter
    // Then: Handles both record and string ID formats
}
```

#### 3.3 Mixed Query Tests (Metadata + Semantic)

```rust
#[tokio::test]
async fn test_chain_id_with_semantic_content() {
    // Given: 10 thoughts in chain "20251005-Session6"
    //        5 mention "photography", 5 mention "coding"
    // When: Search with chain_id="20251005-Session6" AND thoughts_content="photography"
    // Then: Returns only the 5 photography thoughts from that chain
    //       Ordered by similarity DESC (not chronological)
    //       similarity scores present (not NULL)
}

#[tokio::test]
async fn test_session_filter_then_semantic_ranking() {
    // Given: Session with 20 thoughts, varying relevance to "debugging"
    // When: Search with session_id="abc123" AND thoughts_content="debugging"
    // Then: Filters to session first, then ranks by semantic similarity
    //       Top results most relevant to debugging within that session
}
```

#### 3.4 Regression Tests (Ensure No Breaking Changes)

```rust
#[tokio::test]
async fn test_pure_semantic_search_unchanged() {
    // Given: Existing semantic search behavior
    // When: Search with ONLY thoughts_content (no metadata filters)
    // Then: Behavior unchanged from before this fix
    //       Similarity scores present, ordered by relevance
}

#[tokio::test]
async fn test_confidence_and_date_filters_still_work() {
    // Given: Thoughts with various confidence levels and dates
    // When: Search with confidence_gte=0.7 and date_from="2025-10-01"
    // Then: Filters work correctly in both fast and semantic paths
}

#[tokio::test]
async fn test_empty_results_handled_gracefully() {
    // Given: No thoughts matching chain_id="nonexistent"
    // When: Metadata-only search with no matches
    // Then: Returns empty results array, no errors
}
```

#### 3.5 Performance Tests

```rust
#[tokio::test]
async fn test_metadata_query_skips_embedding() {
    // Given: Mock embedder that tracks call count
    // When: Metadata-only search (chain_id only)
    // Then: Zero embedding API calls made
    //       Query completes faster than semantic search
}
```

**Test Coverage Summary**:
- ✅ Metadata-only fast path
- ✅ Meta::id OR string edge cases (previous/revises/branch)
- ✅ Mixed metadata + semantic queries
- ✅ Regression - existing semantic search unchanged
- ✅ Confidence/date filter compatibility
- ✅ Performance - no embedding for metadata queries

## Alternative Approach (If Performance Not Critical)

Simply make embedding optional in WHERE clause:

```rust
// Replace line 346
let mut where_clauses = vec![];

// Only add embedding filter if we have a query embedding
if q_emb.is_some() {
    where_clauses.push("embedding_dim = $dim AND embedding IS NOT NULL".to_string());
}
```

This is simpler but still calls embedding API when thoughts_content is provided with chain_id.

## Migration Path

### Breaking Changes
None - this is purely additive functionality.

### User Migration
Users currently working around this by:
1. Searching with semantic content (my current workaround)
2. Using inner_voice instead of direct search

After fix, users can:
```rust
// Get entire handoff chain
photography_search(
    chain_id="20251005-PhotoSession6-Handoff",
    include_thoughts=true,
    top_k_thoughts=20
)

// Get specific chain member semantically ranked
photography_search(
    chain_id="20251005-PhotoSession6-Handoff",
    thoughts_content="Identity Relationship",
    include_thoughts=true
)
```

## Codex Review Feedback (Incorporated)

Codex provided excellent architectural review feedback that improved this plan:

1. **✅ Output Shape Consistency**: Added explicit `ThoughtOut` mapping in both paths to ensure schema matches. Metadata-only queries return `similarity: None`, semantic queries return `similarity: Some(f32)`.

2. **✅ DRY Principle**: Extracted `build_metadata_filters()` helper function to avoid duplicating filter logic between fast path and semantic path. Both paths now share the same metadata filter building code.

3. **✅ Meta::ID Edge Cases**: Added comprehensive tests for `previous_thought_id`, `revises_thought`, and `branch_from` which use complex `meta::id OR string` patterns in SurrealDB queries. These need explicit coverage.

4. **✅ Mixed Query Tests**: Added tests for chain_id + content to ensure we filter first, then rank by similarity within the filtered set.

5. **✅ User Documentation**: Documented that metadata-only queries return NULL similarity scores, explaining why and when this happens. Clear response schema shows both cases.

## Timeline Estimate (Revised)

- Phase 1 Implementation: 3-4 hours
  - Extract helper function: 30 min
  - Fast path logic: 1.5 hours
  - Semantic path refactor: 1 hour
  - Integration testing: 30-60 min
- Phase 2 Documentation: 45 minutes
  - Tool description updates: 20 min
  - Response schema documentation: 15 min
  - Usage examples: 10 min
- Phase 3 Comprehensive Tests: 2-3 hours
  - Metadata-only tests: 45 min
  - Meta::ID edge cases: 1 hour
  - Mixed queries: 45 min
  - Regression tests: 30 min
- **Total**: 6-8 hours of focused development

## Success Criteria (Updated)

### Functional Requirements
1. ✅ Can retrieve all thoughts with `chain_id="20251005-PhotoSession6-Handoff"` without providing content
2. ✅ Results ordered chronologically (created_at ASC) for chain/session continuity
3. ✅ Mixed queries (chain_id + content) filter first, then rank by similarity
4. ✅ Meta::id OR string comparisons work for all relationship filters
5. ✅ Response schema consistent: `similarity: null` for metadata-only, `similarity: float` for semantic

### Performance Requirements
6. ✅ No embedding API call for metadata-only queries (fast path)
7. ✅ Metadata-only queries complete in <50ms (no network latency)

### Compatibility Requirements
8. ✅ Backwards compatible - existing semantic searches work unchanged
9. ✅ Confidence and date filters work in both paths
10. ✅ Empty results handled gracefully (no errors)

### Test Coverage Requirements
11. ✅ Metadata-only tests (chain_id, session_id, combinations)
12. ✅ Meta::ID edge case tests (previous/revises/branch with record vs string)
13. ✅ Mixed query tests (metadata + semantic)
14. ✅ Regression tests (pure semantic, filters, empty results)
15. ✅ Performance test (embedding call count = 0 for metadata queries)

### Documentation Requirements
16. ✅ Tool parameter descriptions clarify metadata vs semantic behavior
17. ✅ Response schema documents NULL vs present similarity
18. ✅ Usage examples show both metadata-only and mixed query patterns

## Priority

**HIGH** - This is blocking basic handoff retrieval functionality, a core feature for session continuity. The workaround (semantic search) is unreliable and returns wrong results.

---

## Quick Reference: Before vs After

### Before This Fix

```rust
// ❌ BROKEN: Returns semantically similar thoughts from OTHER chains
photography_search(
    chain_id="20251005-PhotoSession6-Handoff",
    include_thoughts=true
)
// Result: ERROR - no results because embedding required but content empty

// ❌ WORKAROUND: Returns wrong results
photography_search(
    thoughts_content="20251005-PhotoSession6-Handoff",
    include_thoughts=true
)
// Result: Thoughts from 20251004-PhotoSession5-Handoff, 20250929-CodeReview-Handoff
//         because they're semantically similar to the string "handoff"
```

### After This Fix

```rust
// ✅ WORKS: Returns all thoughts with exact chain_id match
photography_search(
    chain_id="20251005-PhotoSession6-Handoff",
    include_thoughts=true,
    top_k_thoughts=20
)
// Result: All 10 thoughts from that chain, chronologically ordered
//         similarity = null (no semantic search)
//         Fast: 0 embedding API calls

// ✅ WORKS: Mixed query - filter then rank
photography_search(
    chain_id="20251005-PhotoSession6-Handoff",
    thoughts_content="Identity Relationship",
    include_thoughts=true
)
// Result: Only thoughts from that chain mentioning identity/relationship
//         Ordered by semantic similarity DESC
//         similarity scores present

// ✅ UNCHANGED: Pure semantic search still works
photography_search(
    thoughts_content="photography workflow",
    include_thoughts=true
)
// Result: Most relevant thoughts across ALL chains
//         Ordered by similarity DESC
```

---

## Implementation Checklist

- [ ] Extract `build_metadata_filters()` helper function
- [ ] Add metadata-only fast path detection
- [ ] Implement fast path with chronological ordering
- [ ] Refactor semantic path to use shared helper
- [ ] Update tool documentation (photography.rs, legacymind equivalents)
- [ ] Document response schema with NULL similarity explanation
- [ ] Write metadata-only tests (chain_id, session_id)
- [ ] Write meta::ID edge case tests (previous/revises/branch)
- [ ] Write mixed query tests (metadata + semantic)
- [ ] Write regression tests (pure semantic unchanged)
- [ ] Write performance test (embedding call count)
- [ ] Manual testing with real handoff chains
- [ ] Code review with Codex
- [ ] Deploy to production
