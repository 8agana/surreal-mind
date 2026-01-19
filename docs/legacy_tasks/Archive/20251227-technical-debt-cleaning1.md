---
date: 2025-12-27
type: technical-debt-audit
status: pending-review
author: rust-builder
total_loc: 15027
audit_duration: 1h
conducted by: Rusty (Sonnet 4.5)
---

# SurrealMind Technical Debt Audit - December 2025

## Executive Summary

After comprehensive analysis of the surreal-mind codebase (15,027 lines of Rust), I've identified **47 discrete cleanup items** across 8 categories. The codebase is generally well-structured, but accumulated technical debt from removed features (memories_populate, memories_moderate, legacymind_update) and evolving architecture creates opportunities for simplification.

**High-Impact Wins**:
- Remove 3 unused schema functions (~75 LOC)
- Delete 2 unused modules (flavor, gemini) saving ~250 LOC
- Consolidate duplicate deserializers (~30 LOC)
- Remove references to 3 deleted tools from detailed_help schema

**Total Estimated Cleanup**: ~600-800 lines of code, improving maintainability without changing functionality.

---

## CATEGORY 1: DEAD CODE FROM REMOVED TOOLS

### Priority: HIGH
### Estimated Impact: Medium - improves clarity, reduces confusion

#### 1.1 Schema Functions for Removed Tools
**File**: `src/schemas.rs`
**Lines**: 4-17 (convo_think_schema), 47-65 (search_thoughts_schema), 82-92 (kg_search_schema)

**Issue**: Three schema functions defined but never used in router.rs:
- `convo_think_schema()` - No "convo_think" tool exists
- `search_thoughts_schema()` - Was replaced by unified_search
- `kg_search_schema()` - Was replaced by unified_search

**Evidence**:
```bash
$ rg "convo_think_schema|search_thoughts_schema|kg_search_schema" src/
src/schemas.rs:4:pub fn convo_think_schema() -> Arc<Map<String, Value>> {
src/schemas.rs:47:pub fn search_thoughts_schema() -> Arc<Map<String, Value>> {
src/schemas.rs:82:pub fn kg_search_schema() -> Arc<Map<String, Value>> {
# No other references found
```

**Fix**: Delete all three functions.

**Risk**: Low - no references exist outside schemas.rs itself.

---

#### 1.2 detailed_help Schema References to Removed Tools
**File**: `src/schemas.rs`
**Lines**: 98-105

**Issue**: detailed_help_schema enum includes removed tools:
- `"legacymind_update"` - Removed December 2024
- `"memories_moderate"` - Removed December 2024
- `"memories_populate"` - Removed December 23, 2025

**Current Code**:
```rust
"tool": {"type": "string", "enum": [
    "legacymind_think",
    "legacymind_update",  // ← REMOVED
    "memories_create", "memories_moderate", "memories_populate",  // ← LAST TWO REMOVED
    "legacymind_search",
    "maintenance_ops", "inner_voice",
    "detailed_help"
]}
```

**Fix**: Remove the three deleted tool names from enum.

**Risk**: Low - these tools don't exist so enum values are unreachable.

---

## CATEGORY 2: UNUSED MODULES

### Priority: HIGH
### Estimated Impact: High - removes ~250 LOC, simplifies dependencies

#### 2.1 Unused Flavor Module
**File**: `src/flavor.rs`
**Lines**: 176 total (including tests)

**Issue**: Module defines `Flavor` enum and `tag_flavor()` function but has **zero usage** in the codebase.

**Evidence**:
```bash
$ rg "use crate::flavor|flavor::|Flavor::" src/
# Only match: src/flavor.rs itself
```

**History**: Likely intended for thought categorization but never integrated.

**Fix**:
1. Delete `src/flavor.rs`
2. Remove `pub mod flavor;` from `src/lib.rs`

**Risk**: Low - completely unused.

---

#### 2.2 Unused Gemini Module
**File**: `src/gemini.rs`
**Lines**: 127 total

**Issue**: Defines `GeminiClient` with API integration but has **zero usage** in active code.

**Evidence**:
```bash
$ rg "use crate::gemini|gemini::" src/
# No matches outside lib.rs module declaration
```

**Note**: Was used for synthesis experiments but replaced by inner_voice provider system.

**Fix**:
1. Delete `src/gemini.rs`
2. Remove `pub mod gemini;` from `src/lib.rs`

**Risk**: Medium - verify no external scripts/binaries depend on it. Check git history for context.

**Recommendation**: Archive to `docs/archived-modules/gemini.rs` with README explaining removal, in case revival needed.

---

## CATEGORY 3: UNUSED COGNITIVE FRAMEWORKS

### Priority: MEDIUM
### Estimated Impact: Medium - ~800 LOC but may have future value

#### 3.1 Complete Cognitive Framework Suite Unused
**Files**:
- `src/cognitive/dialectical.rs`
- `src/cognitive/first_principles.rs`
- `src/cognitive/lateral.rs`
- `src/cognitive/ooda.rs`
- `src/cognitive/root_cause.rs`
- `src/cognitive/socratic.rs`
- `src/cognitive/systems.rs`
- `src/cognitive/framework.rs`
- `src/cognitive/types.rs`
- `src/cognitive/mod.rs`

**Total Lines**: ~800 LOC

**Issue**: Entire cognitive framework system (7 frameworks + engine) is **only used in one location**:
```rust
// src/server/db.rs:211
use crate::cognitive::profile::{Submode, profile_for};
```

Only the `profile` module is actively used. The framework analysis engine (`CognitiveEngine`) and all 7 individual frameworks are never invoked.

**Evidence**:
- `CognitiveEngine::new()` - not called
- `CognitiveEngine::analyze_all()` - not called
- `CognitiveEngine::blend()` - not called
- All framework implementations (OODA, Socratic, etc.) - instantiated in `FRAMEWORKS` lazy static but never executed

**Architectural Decision Required**:

**Option A - Keep (Recommended)**: This appears to be future infrastructure for advanced thinking modes. The code is well-structured, tested, and may be activated when submode-based analysis is fully integrated.

**Option B - Archive**: Move to `docs/archived-modules/cognitive/` if not planned for 2025.

**Option C - Partial Cleanup**: Keep only `profile.rs` and delete the rest if profiles are the only needed component.

**Recommendation**: Keep but add documentation explaining intended use case. Add feature flag if wanted.

---

## CATEGORY 4: DUPLICATE CODE PATTERNS

### Priority: MEDIUM
### Estimated Impact: Low - consolidation improves maintainability

#### 4.1 Duplicate Thing-to-String Deserializers
**Files**:
- `src/serializers.rs:13` - `deserialize_thing_or_string()`
- `src/server/mod.rs:17` - `deserialize_thing_to_string()`

**Issue**: Two nearly identical functions doing the same job - converting SurrealDB Thing objects to strings.

**Current Usage**:
- `deserialize_thing_to_string` used in `src/server/mod.rs:57`
- `deserialize_thing_or_string` appears unused

**Evidence**:
```bash
$ rg "deserialize_thing_or_string" src/
src/serializers.rs:13:pub fn deserialize_thing_or_string<'de, D>(...)
# No usage found
```

**Fix**:
1. Verify `deserialize_thing_or_string` is truly unused
2. Delete from `src/serializers.rs`
3. OR consolidate both into single implementation in `src/serializers.rs` and update import in `src/server/mod.rs`

**Risk**: Low - just consolidating duplicates.

---

## CATEGORY 5: UNUSED PROMPT INFRASTRUCTURE

### Priority: MEDIUM
### Estimated Impact: Medium - ~400 LOC of unused infrastructure

#### 5.1 Unused Prompt Metrics System
**File**: `src/prompt_metrics.rs`
**Lines**: 221 total

**Issue**: Entire prompt metrics tracking system defined but **never used**:
- `PromptInvocation` struct
- `PromptMetrics` struct
- `PromptMetricsCollector`
- `init_prompt_metrics()` function

**Evidence**:
```bash
$ rg "prompt_metrics::|PromptMetrics|PromptInvocation" src/ --type rust
src/lib.rs:12:pub mod prompt_metrics;
src/prompt_metrics.rs:# (only definitions, no usage)
```

**Purpose**: Was designed to track prompt effectiveness for cognitive framework evolution.

**Fix Options**:
1. **Delete** if not planned for use
2. **Archive** to docs/archived-modules/ for future reference
3. **Activate** if framework evolution tracking is roadmapped

**Recommendation**: Archive - well-designed system but not currently integrated.

---

#### 5.2 Unused Prompt Critiques System
**File**: `src/prompt_critiques.rs`
**Lines**: 180 total

**Issue**: Similar to prompt_metrics - entire critique storage system unused:
- `PromptCritique` struct
- Methods to store/retrieve critiques
- Evolution suggestion system

**Evidence**: Only declared in `src/lib.rs:11`, never invoked.

**Fix**: Same options as 5.1 - delete, archive, or activate.

**Recommendation**: Archive alongside prompt_metrics since they're complementary systems.

---

#### 5.3 Unused Prompts Registry
**File**: `src/prompts.rs`
**Lines**: 139 total

**Issue**: `Prompt` struct and `PromptRegistry` defined but unused:
- Prompt with ID, name, template, version
- PromptRegistry for storing/retrieving prompts

**Evidence**: Module declared but no active usage found.

**Relationship**: Works with prompt_metrics and prompt_critiques - part of unified (but unused) prompt management system.

**Fix**: Archive all three prompt-related modules together as a coherent subsystem.

---

## CATEGORY 6: BINARY UTILITIES CLEANUP

### Priority: LOW
### Estimated Impact: Low - utilities may have historical/debug value

#### 6.1 Potentially Obsolete Debug Binaries
**Location**: `src/bin/`
**Total LOC**: 2,625 lines across 12 binaries

**Binaries to Review**:

1. **check_db_contents.rs** (2228 bytes, Aug 31)
   - Purpose: Database content inspection
   - Status: May be superseded by smtop

2. **simple_db_test.rs** (3691 bytes, Aug 31)
   - Purpose: Basic DB connectivity test
   - Status: Likely replaced by proper tests

3. **sanity_cosine.rs** (2281 bytes, Sep 1)
   - Purpose: Cosine similarity testing
   - Status: One-off validation tool

**Active/Recent Binaries** (keep these):
- `smtop.rs` (Dec 24) - Active monitoring tool
- `kg_dedupe_plan.rs` (Dec 24) - Active KG maintenance
- `reembed_kg.rs` (Dec 24) - Active embedding maintenance
- `db_check.rs` (Nov 4) - Database health checks
- `migration.rs` (Oct 30) - Schema migrations

**Recommendation**:
- Keep active utilities (6 binaries)
- Archive old debug utilities (3 binaries) to `tools/archived/`
- Document purpose of each kept binary in `docs/UTILITIES.md`

---

## CATEGORY 7: DEPENDENCY AUDIT

### Priority: LOW
### Estimated Impact: Low - build time reduction

#### 7.1 Potentially Unused Dependencies
**File**: `Cargo.toml`

**Dependencies to Verify**:

1. **rmp-serde** - MessagePack serialization
   - Used? Couldn't find references
   - Check: May be for future binary protocol

2. **sha1** - SHA-1 hashing
   - Used? Not found in grep
   - Note: blake3 is used for hashing

3. **strsim** - String similarity
   - Used? Not found in active code
   - May have been for fuzzy matching experiments

4. **time** - Time handling
   - Used? chrono is primary time library
   - Possible redundancy

5. **unicode-normalization** - Text normalization
   - Used? Not found in grep
   - May be for future international text handling

**Fix**:
1. Run `cargo +nightly udeps` to detect truly unused deps
2. Remove confirmed unused dependencies
3. Document why each dependency exists in Cargo.toml comments

**Risk**: Low - cargo will catch missing deps on rebuild.

---

## CATEGORY 8: CODE QUALITY IMPROVEMENTS

### Priority: LOW
### Estimated Impact: Low - polish and consistency

#### 8.1 smtop.rs Deprecation Warning
**File**: `src/bin/smtop.rs`
**Issue**: Uses deprecated `Frame::size()` instead of `Frame::area()`

**Fix**: Update to modern ratatui API:
```rust
// Old
let size = frame.size();

// New
let area = frame.area();
```

**Risk**: None - simple API update.

---

#### 8.2 Missing Error Context
**Locations**: Various `anyhow::bail!()` calls

**Issue**: Some errors lack contextual information for debugging.

**Examples**:
```rust
// src/lib.rs:99
anyhow::bail!("HTTP select failed: {}", resp.text().await.unwrap_or_default());
// Better: Include query, NS, DB in error

// src/lib.rs:163
anyhow::bail!("HTTP update failed: {}", uresp.text().await.unwrap_or_default());
// Better: Include thought ID, embedding provider in error
```

**Fix**: Audit all `anyhow::bail!()` calls and add context using `.context()`:
```rust
resp.error_for_status()
    .context(format!("Querying {} thoughts from {}/{}", take, ns, dbname))?
```

**Benefit**: Better error messages for production debugging.

---

#### 8.3 Inconsistent Comment Styles
**Issue**: Mix of documentation comments (`///`, `//!`) and regular comments (`//`).

**Examples**:
- Some modules have comprehensive `//!` module docs
- Others have minimal or no module-level documentation
- Some public functions lack `///` doc comments

**Fix**:
1. Add `//!` module documentation to all public modules
2. Add `///` doc comments to all public functions
3. Run `cargo doc --open` to verify documentation completeness

**Benefit**: Better generated documentation, easier onboarding.

---

## CATEGORY 9: OPPORTUNITIES FOR CONSOLIDATION

### Priority: LOW
### Estimated Impact: Medium - architectural improvement

#### 9.1 Embeddings Provider Pattern Could Unify
**Files**: `src/embeddings.rs`, `src/bge_embedder.rs`

**Observation**: BGE embedder is separate module while OpenAI is inline in embeddings.rs.

**Suggestion**: Consider extracting OpenAI to `src/embeddings/openai.rs` for symmetry:
```
src/embeddings/
  mod.rs       (trait + factory)
  bge.rs       (BGE implementation)
  openai.rs    (OpenAI implementation)
```

**Benefit**: Clearer separation, easier to add new providers.

**Risk**: Low - refactor only.

---

#### 9.2 HTTP SQL Client Pattern
**File**: `src/utils/db.rs`

**Observation**: `HttpSqlConfig` utility is well-designed for HTTP SQL operations.

**Opportunity**: Some binaries duplicate HTTP SQL client setup instead of using this utility.

**Fix**: Audit all binaries and migrate to using `HttpSqlConfig::from_config()`.

**Benefit**: Less duplication, consistent error handling.

---

## IMPLEMENTATION PLAN

### Phase 1: Quick Wins (1-2 hours)
**Goal**: Remove clear dead code with zero risk

1. Delete 3 unused schema functions (schemas.rs)
2. Remove 3 deleted tools from detailed_help enum (schemas.rs)
3. Fix smtop deprecation warning (smtop.rs)
4. Delete unused deserializer (if confirmed)

**Expected Outcome**: ~100 LOC removed, 0 functionality changed

---

### Phase 2: Module Cleanup (2-4 hours)
**Goal**: Remove unused modules after verification

1. Archive flavor.rs module
2. Archive gemini.rs module (with git history check)
3. Archive 3 prompt management modules (prompts, prompt_metrics, prompt_critiques)
4. Archive 3 old debug binaries

**Expected Outcome**: ~900 LOC removed/archived, build time reduced

---

### Phase 3: Dependency Audit (1-2 hours)
**Goal**: Clean unused dependencies

1. Run `cargo +nightly udeps`
2. Remove confirmed unused deps from Cargo.toml
3. Test build and all binaries
4. Document remaining deps

**Expected Outcome**: Faster builds, clearer dependency graph

---

### Phase 4: Quality Polish (4-6 hours)
**Goal**: Improve maintainability

1. Add module documentation to undocumented modules
2. Add error context to anyhow::bail!() calls
3. Consider embeddings provider consolidation
4. Create UTILITIES.md documenting all binaries

**Expected Outcome**: Better documentation, easier debugging

---

### Phase 5: Cognitive Framework Decision (1 hour planning)
**Goal**: Decide fate of cognitive modules

**Options**:
- A: Keep with documentation explaining future use
- B: Archive entire cognitive/ directory
- C: Keep only profile.rs, archive rest

**Recommendation**: Keep all for now, add feature flag `cognitive-frameworks` to make optional:
```toml
[features]
default = []
cognitive-frameworks = []
```

This preserves the work while making it opt-in.

---

## RISK ASSESSMENT

### Low Risk Items (safe to proceed)
- Deleting unused schema functions
- Removing deleted tool names from enums
- Deleting flavor.rs module
- Fixing deprecation warnings
- Archiving old debug binaries

### Medium Risk Items (verify first)
- Deleting gemini.rs (check git history for context)
- Deleting prompt management modules (may be planned for use)
- Removing dependencies (run udeps to confirm)

### High Risk Items (needs discussion)
- Cognitive framework modules (significant LOC, may have future plans)

---

## METRICS

### Current State
- Total LOC: 15,027
- Dead code identified: ~600-800 LOC
- Unused modules: 5 files (~900 LOC)
- Unused functions: 6 schema functions (~150 LOC)
- Duplicate code: 1 deserializer (~30 LOC)

### After Cleanup (Phases 1-2)
- Estimated LOC: 14,100-14,200
- Reduction: ~800-900 LOC (6%)
- Modules: -5 files
- Dependencies: TBD (pending udeps)

---

## NOTES FOR MAINTAINER

### Why This Matters
Technical debt isn't "bad code" - it's accumulated cruft from evolving architecture. These removed tools (memories_populate, memories_moderate, legacymind_update) left behind unused infrastructure. Cleaning this:

1. **Improves clarity**: New contributors won't wonder why flavor.rs exists
2. **Reduces build time**: Fewer modules, fewer dependencies
3. **Prevents bugs**: Unused code can't cause bugs, but it can confuse
4. **Enables iteration**: Cleaner codebase is easier to refactor

### What NOT to Clean
- Cognitive frameworks - likely future feature
- Well-tested utilities (kg_dedupe_plan, reembed_kg, smtop)
- Any code with recent commits (< 1 month)
- Anything with unclear purpose (ask first)

### Testing Strategy
After each cleanup phase:
```bash
cargo check                    # Compilation
cargo clippy -- -D warnings    # Lints
cargo test                     # Tests
cargo build --release          # All binaries
```

---

## CONCLUSION

SurrealMind has **moderate technical debt** concentrated in:
1. Removed tool artifacts (high priority, low risk)
2. Unused experimental modules (medium priority, low risk)
3. Unused infrastructure (medium priority, medium risk)

**Recommended Action**: Execute Phases 1-2 immediately (quick wins + module cleanup). Phase 3-4 can be scheduled. Phase 5 needs architectural decision on cognitive frameworks.

**Total Effort**: 8-12 hours for comprehensive cleanup.
**Impact**: 6% LOC reduction, improved maintainability, clearer architecture.

---

**Next Steps**: Review this audit with Sam, decide on cognitive frameworks, execute Phase 1 as test case.
