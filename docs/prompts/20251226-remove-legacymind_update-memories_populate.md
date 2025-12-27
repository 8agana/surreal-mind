# Removal Plan: memories_populate, memories_moderate, legacymind_update

**Date**: 2025-12-26
**Prompt Type**: Implementation Plan (tool removal)
**Justification**: The tools being removed are not designed with future iterations in mind. 
**Status**: Completed
**Implementation Date**: 2025-12-26
**Previous Connected Prompt**: docs/prompts/20251221-memories_populate-implementation.md
**Previous Connected Prompt**: docs/prompts/20251226-legacymind_update-implementation.md

___

This document outlines the steps to completely remove the `memories_populate`, `memories_moderate`, and `legacymind_update` tools from the `surreal-mind` codebase.

## 1. Files to Delete

These files are dedicated entirely to the tools being removed:
*   `src/tools/memories_populate.rs`
*   `src/tools/legacymind_update.rs`

## 2. Code Removal

Edit the following files to remove logic, registrations, and references.

### `src/server/router.rs`
*   **Remove Imports:** `use crate::tools::memories_populate::{ExtractedMemory, MemoriesPopulateRequest};`
*   **Remove Tool Registration (`list_tools` method):**
    *   Delete the schema bindings:
        *   `legacymind_update_schema_map`
        *   `kg_moderate_schema_map`
        *   `memories_populate_schema_map`
    *   Delete the output schema bindings:
        *   `legacymind_update_output`
        *   `memories_moderate_output`
        *   `memories_populate_output`
    *   Remove the `Tool` struct definitions for:
        *   `legacymind_update`
        *   `memories_moderate`
        *   `memories_populate`
*   **Remove Dispatch Logic (`call_tool` method):**
    *   Remove the `match` arms for:
        *   `"legacymind_update"`
        *   `"memories_moderate"`
        *   `"memories_populate"`
*   **Remove Implementation Methods (`impl SurrealMindServer`):**
    *   `handle_memories_populate` (The main handler)
    *   `fetch_thoughts_for_extraction` (Helper for population)
    *   `create_memory` (Helper for population)
    *   `stage_memory_for_review` (Helper for population)

### `src/tools/knowledge_graph.rs`
*   **Remove Handler:** `handle_knowledgegraph_moderate`.
*   **Remove Legacy Handlers (Dead Code):** `handle_knowledgegraph_review` and `handle_knowledgegraph_decide` (unused/legacy versions of moderate).

### `src/tools/mod.rs`
*   Remove `pub mod legacymind_update;`
*   Remove `pub mod memories_populate;`

### `src/schemas.rs`
*   **Remove Input Schemas:**
    *   `legacymind_update_schema`
    *   `kg_moderate_schema`
    *   `memories_populate_schema`
    *   `kg_review_schema` (if present)
    *   `kg_decide_schema` (if present)
*   **Remove Output Schemas:**
    *   `legacymind_update_output_schema`
    *   `memories_moderate_output_schema`
    *   `memories_populate_output_schema`

### `src/tools/detailed_help.rs`
*   Remove the JSON help entries for:
    *   `memories_moderate`
    *   `legacymind_update`
    *   `memories_populate`
    *   Remove them from the `Tool` enum list in the schema definition.

### `src/main.rs`
*   Update the startup log string: `"🛠️ Loaded ... MCP tools"` to reflect the new count.

### `tests/tool_schemas.rs`
*   Update the test assertions to remove the expectation of finding these tools in the schema list.

## 3. Verification

*   Run `cargo check` to ensure no broken references.
*   Run `cargo test --test tool_schemas` to verify schema cleanliness.

---

## 20251216 - Reviewed by CC

### Reasoning and Justification for Tool Removal:

  1. Fundamental Persistence Failures (Session 20251226-Session1-CC)

  legacymind_update (created today):
  - Test 1: Update extracted_to_kg + extraction_batch_id → updated: false, no persistence
  - Test 2: Update confidence field → updated: false, no persistence
  - Test 3: Fresh thought (seconds old) → updated: false, no persistence
  - Pattern: Tool reports fields_updated but updated: false, database shows zero changes

  memories_populate (existing tool):
  - Five test runs with different fixes (angle brackets, type::thing(), consciousness typo fix, rebuild)
  - All reported success, generated batch IDs, extracted entities
  - Database verification: extracted_to_kg: false, extraction_batch_id: NONE every time
  - Manual CLI UPDATEs with identical syntax: persist immediately

  Root Cause: UPDATE queries execute without error but affect 0 records. Same pattern across:
  - memories_populate UPDATE (marking thoughts processed)
  - legacymind_update UPDATE (any field modification)
  - store_tool_session UPSERT (session persistence)

  2. Architectural Mismatch: Hardcoded Orchestration vs Delegation

  Current broken model:
  router.rs orchestrates:
    → Query thoughts
    → Call Gemini CLI
    → Parse response
    → INSERT entities
    → UPDATE thoughts ← FAILS SILENTLY

  Proven working model (tested this session):
  Gemini orchestrates using MCP tools:
    → Uses legacymind_search to find thoughts
    → Uses memories_create to insert entities
    → Uses bash/CLI to UPDATE thoughts ← WORKS

  Test Results:
  - Scalpel (Opus via Task tool): 10 entities, 7 relationships, 6 observations extracted, 5 thoughts marked → reported success but didn't persist
  - Gemini (via CLI delegation): 4 entities, 1 relationship, 1 observation extracted, 5 thoughts marked → actually persisted (batch ID 9b1deb4d confirmed in database)

  3. Why Current Architecture Can't Be Fixed

  Five different fix attempts failed identically:
  1. Original angle bracket syntax
  2. Gemini's type::thing() parameter binding
  3. Consciousness database typo fix
  4. Binary rebuild + server restart
  5. Fresh thought test (eliminate legacy data issues)

  Pattern: The UPDATE syntax is valid (manual CLI proves it), but execution context in router.rs causes silent failures. This isn't a syntax bug - it's an architectural issue with how the Rust code handles SurrealDB transactions or response parsing.

  Fix attempts would require:
  - Deep debugging of surrealdb Rust crate transaction semantics
  - Response parsing investigation across multiple tools
  - Potential upstream dependency issues
  - Endless whack-a-mole with persistence bugs

  4. Replacement Architecture: inner_voice Delegation

  Design:
  inner_voice(
    prompt: String,           // Natural language request
    session_id?: String,      // Optional session resumption
    context_files?: Vec<String>
  ) -> String

  Capabilities (proven via Gemini CLI test):
  - Extract KG from thoughts: inner_voice("Process 10 oldest unprocessed thoughts")
  - Troubleshoot issues: inner_voice("Debug why UPDATE isn't persisting")
  - Summarize chains: inner_voice("Summarize chain_id 20251226-Session1-CC")
  - Search memory: inner_voice("Have we tried type::thing() before?")

  Why this works:
  - Gemini has direct SurrealMind MCP access (legacymind_search, memories_create, etc.)
  - Gemini can execute bash commands (surreal CLI for UPDATEs)
  - Natural language workflows documented in GEMINI.md, not hardcoded in Rust
  - Proven to work (batch ID 9b1deb4d persisted when hardcoded tools failed)

  5. No Functional Loss

  What we're removing:
  - memories_populate: broken, never successfully marked thoughts
  - legacymind_update: created today, broken immediately
  - memories_moderate: not tested this session, but uses same architecture

  What replaces them:
  - inner_voice delegation to Gemini for all multi-step cognitive workflows
  - Proven via successful 4-wave extraction test
  - Session management already implemented (tool_sessions table exists)
  - Gemini maintains context across waves (1M token window)

  6. Migration Path for 577 Pending Thoughts

  Current state: 577 thoughts with extracted_to_kg: false

  Processing via inner_voice:
  inner_voice("Process all unprocessed thoughts in batches of 10,
               extract entities/relationships/observations,
               mark each batch as processed")

  Gemini handles:
  - Batch retrieval via legacymind_search
  - Extraction via memories_create
  - Marking via CLI UPDATE (proven to work)
  - Progress tracking across batches (session continuity)

  Summary

  These tools represent a failed architectural approach - hardcoded orchestration that can't reliably persist updates. Five debugging sessions, five different fixes, zero successes. Meanwhile, Gemini delegation via CLI worked on first try.

  Remove the broken tools, build inner_voice as the universal delegation endpoint, and let Gemini orchestrate workflows using the MCP tools that actually work (legacymind_search, memories_create, bash).

  Status: Not removing working code - removing code that never worked and has a proven replacement architecture.

---

## Implementation Results - 2025-12-26

The removal of the specified tools and their connected references has been successfully completed.

### Changes Performed:
1.  **File Deletion:**
    -   `src/tools/memories_populate.rs` (deleted)
    -   `src/tools/legacymind_update.rs` (deleted)
2.  **Code Cleanup:**
    -   **`src/server/router.rs`**: Removed all logic for tool registration (`list_tools`), dispatching (`call_tool`), and the implementation methods for `memories_populate`.
    -   **`src/tools/knowledge_graph.rs`**: Removed `handle_knowledgegraph_moderate` and associated dead code handlers (`review`, `decide`).
    -   **`src/tools/mod.rs`**: Removed module declarations for the deleted files.
    -   **`src/schemas.rs`**: Removed input and output JSON schemas for all three tools.
    -   **`src/tools/detailed_help.rs`**: Removed tool entries from the roster and the `Tool` enum list.
    -   **`src/main.rs`**: Updated the startup log message to reflect the new tool count (6 tools).
    -   **`tests/tool_schemas.rs`**: Updated assertions to match the new tool roster.
3.  **Verification:**
    -   `cargo check`: Passed.
    -   `cargo test --test tool_schemas`: Passed (6 tests successful).

### Final Status:
The cognitive kernel has been simplified by removing high-orchestration logic that failed to persist reliably, deferring these workflows to Gemini via the `inner_voice` delegation model.