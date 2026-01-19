# Implementation Plan for Grok - CCR Remaining Fixes
**Date**: 2025-09-08 20:30 CDT  
**For**: Grok  
**Priority**: High - Production stability issues

## Completed Work ✅
Great job on these:
- LRU cache size fix with `SURR_CACHE_MAX` env var
- Database reconnection logic with `SURR_DB_RECONNECT`
- Embedding dimension hygiene check (caught real issue!)
- All tests passing, clippy clean

## Critical Fixes Needed 🔴

### 1. Fix Embedding Dimension Mismatch (BLOCKING)
**Issue**: Server won't start - dimension check failing  
**Location**: Database has mixed dimensions  
**Fix**:
```bash
# First, check what dimensions exist
surreal sql --conn http://localhost:8000 --user root --pass root --ns surreal_mind --db surreal_mind
> SELECT embedding_dim, count() FROM thoughts GROUP BY embedding_dim;
```

**Options**:
- A) Add `--skip-dimension-check` flag for startup
- B) Auto-fix on startup with re-embedding
- C) Add manual fix command: `cargo run --bin fix_dimensions`

**Recommendation**: Option A first (unblock), then B

### 2. Remove ALL unwrap() Calls
**Count**: 21 instances across 9 files  
**Critical locations**:

```rust
// src/server/mod.rs line 562
NonZeroUsize::new(cache_max).unwrap_or_else(|| NonZeroUsize::new(1).unwrap())
// This one YOU added! Fix to:
NonZeroUsize::new(cache_max).unwrap_or(NonZeroUsize::MIN)

// src/embeddings.rs lines 115, 147
.unwrap_or_default() // These hide errors!
// Fix to:
.context("Failed to get response text")?

// src/frameworks/convo.rs - 5 instances
// src/config.rs - 3 instances
```

**Strategy**:
1. Replace `.unwrap()` with `?` operator
2. Add `.context("descriptive error")` for debugging
3. For Options, use `.ok_or_else(|| anyhow!("error"))?`
4. In tests/bins, unwrap() is OK

### 3. Implement Rate Limiting
**Location**: `src/embeddings.rs` OpenAI calls  
**Requirements**:
- Max 3000 embeddings/minute (OpenAI limit)
- Token bucket or sliding window

**Implementation**:
```toml
# Cargo.toml
governor = "0.6"
```

```rust
// src/embeddings.rs
use governor::{Quota, RateLimiter};
use std::sync::Arc;
use nonzero_ext::*;

pub struct OpenAIEmbedder {
    // ... existing fields
    rate_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
}

impl OpenAIEmbedder {
    pub fn new(...) -> Result<Self> {
        let quota = Quota::per_minute(nonzero!(3000u32));
        let rate_limiter = Arc::new(RateLimiter::direct(quota));
        // ...
    }
}

async fn embed(&self, text: &str) -> Result<Vec<f32>> {
    // Wait for rate limit
    self.rate_limiter.until_ready().await;
    // ... existing code
}
```

## Implementation Order

### Phase 1: Unblock Server (30 min)
1. Add `--skip-dimension-check` flag or env var `SURR_SKIP_DIM_CHECK`
2. Test server starts successfully
3. Document the flag in README

### Phase 2: Fix Panics (1 hour)
1. Fix YOUR unwrap in server/mod.rs:562
2. Fix embeddings.rs unwraps (critical path)
3. Fix config.rs unwraps 
4. Fix frameworks/convo.rs unwraps
5. Leave test/bin unwraps alone

### Phase 3: Rate Limiting (1 hour)
1. Add governor dependency
2. Create rate limiter in OpenAIEmbedder::new()
3. Add `.until_ready().await` before API calls
4. Add config: `SURR_EMBED_RATE_LIMIT` (default 3000/min)
5. Test with bulk operation

### Phase 4: Dimension Fix Tool (optional, 30 min)
1. Create `src/bin/fix_dimensions.rs`
2. Query all thoughts with wrong dimensions
3. Re-embed with correct model
4. Update records

## Testing Checklist
- [ ] Server starts with `SURR_SKIP_DIM_CHECK=true`
- [ ] No panics on missing config values
- [ ] No panics on API errors
- [ ] Rate limiting prevents 429 errors
- [ ] `cargo test` still passes
- [ ] `cargo clippy` still clean

## Quick Test Commands
```bash
# Test server startup
SURR_SKIP_DIM_CHECK=true cargo run

# Test rate limiting
for i in {1..100}; do
  curl -X POST localhost:8000/think -d '{"content":"test"}'
done

# Verify no unwraps in critical path
grep -n "unwrap()" src/embeddings.rs src/server/mod.rs src/config.rs
```

## Notes
- Your cache fix and reconnection logic are solid
- The dimension check is valuable - just need escape hatch
- Focus on removing unwraps in critical paths only
- Rate limiting is crucial for production stability

**Target**: All fixes complete in 3 hours max